use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use clap::Args;

use crate::catalog::Catalog;
use crate::config;
use crate::error::McpzipError;
use crate::mcp::protocol::*;
use crate::mcp::server::McpServer;
use crate::mcp::transport::NdjsonTransport;
use crate::proxy::ProxyServer;
use crate::search;
use crate::transport::{ConnectFn, Manager, Upstream};

#[derive(Args)]
pub struct ServeArgs {
    /// Path to config file
    #[arg(long, default_value_os_t = config::default_path())]
    pub config: std::path::PathBuf,
}

pub async fn run_serve(args: &ServeArgs) -> Result<(), McpzipError> {
    let cfg = config::load(&args.config)?;

    eprintln!(
        "mcpzip: starting proxy ({} servers)",
        cfg.mcp_servers.len()
    );

    // Resolve Gemini API key: env -> config.
    let api_key = std::env::var("GEMINI_API_KEY")
        .ok()
        .or_else(|| cfg.gemini_api_key.clone())
        .unwrap_or_default();

    // Create transport manager.
    let connect = make_connect_fn();

    let idle_timeout = Duration::from_secs(
        cfg.idle_timeout_minutes.unwrap_or(5) * 60,
    );
    let call_timeout = Duration::from_secs(
        cfg.call_timeout_seconds.unwrap_or(120),
    );
    let tm = Arc::new(Manager::new(
        cfg.mcp_servers.clone(),
        idle_timeout,
        call_timeout,
        connect,
    ));

    // Create catalog.
    let catalog = Arc::new(Catalog::new(config::cache_path()));
    if let Err(e) = catalog.load() {
        eprintln!("mcpzip: warning: failed to load cache: {}", e);
    }

    // Create searcher.
    let model = cfg
        .search
        .model
        .as_deref()
        .unwrap_or("gemini-2.0-flash")
        .to_string();
    let catalog_for_search = catalog.clone();
    let catalog_fn: search::CatalogFn = Arc::new(move || catalog_for_search.all_tools());
    let searcher = search::new_searcher(&api_key, &model, catalog_fn);

    // Create proxy server.
    let proxy = Arc::new(ProxyServer::new(catalog.clone(), searcher, tm.clone()));

    eprintln!(
        "mcpzip: loaded {} tools from cache",
        catalog.tool_count()
    );

    // Background refresh - serve from cache immediately, update as servers connect.
    let refresh_catalog = catalog.clone();
    let refresh_tm = tm.clone();
    tokio::spawn(async move {
        eprintln!("mcpzip: refreshing catalog in background...");
        match refresh_tm.list_tools_all().await {
            Ok(server_tools) => {
                if let Err(e) = refresh_catalog.refresh(server_tools) {
                    eprintln!("mcpzip: background refresh error: {}", e);
                } else {
                    eprintln!(
                        "mcpzip: catalog refreshed ({} tools)",
                        refresh_catalog.tool_count()
                    );
                }
            }
            Err(e) => {
                eprintln!("mcpzip: background refresh error (serving from cache): {}", e);
            }
        }
    });

    // Set up cancellation.
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_for_signal = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        eprintln!("\nmcpzip: shutting down");
        cancel_for_signal.cancel();
    });

    // Set up MCP server over stdio.
    let transport: Arc<dyn crate::mcp::transport::McpTransport> =
        Arc::new(NdjsonTransport::stdio());
    let mut server = McpServer::new(transport);
    server.set_capabilities(ServerCapabilities {
        tools: Some(ToolsCapability {}),
        resources: None,
        prompts: None,
    });

    let instructions = proxy.instructions();
    if !instructions.is_empty() {
        server.set_instructions(instructions);
    }

    // Register tool handlers.
    register_handlers(&mut server, proxy);

    eprintln!("mcpzip: serving MCP over stdio");
    server.run(cancel).await?;

    // Cleanup.
    tm.close().await?;
    Ok(())
}

fn make_connect_fn() -> ConnectFn {
    Arc::new(|name: String, cfg: crate::types::ServerConfig| {
        Box::pin(async move {
            match cfg.effective_type() {
                "stdio" => {
                    let upstream =
                        crate::transport::stdio::StdioUpstream::new(name, &cfg).await?;
                    Ok(Box::new(upstream) as Box<dyn Upstream>)
                }
                "http" => {
                    let store = Arc::new(
                        crate::auth::store::TokenStore::new(crate::config::auth_dir()),
                    );
                    let oauth = crate::auth::oauth::OAuthHandler::new(
                        cfg.url.clone().unwrap_or_default(),
                        store,
                    );
                    let upstream =
                        crate::transport::http::HttpUpstream::new(name, &cfg, Some(oauth)).await?;
                    Ok(Box::new(upstream) as Box<dyn Upstream>)
                }
                "sse" => {
                    let upstream =
                        crate::transport::sse::SseUpstream::new(name, &cfg).await?;
                    Ok(Box::new(upstream) as Box<dyn Upstream>)
                }
                other => Err(McpzipError::Config(format!(
                    "unsupported transport type: {:?}",
                    other
                ))),
            }
        }) as Pin<Box<dyn std::future::Future<Output = Result<Box<dyn Upstream>, McpzipError>> + Send>>
    })
}

fn register_handlers(server: &mut McpServer, proxy: Arc<ProxyServer>) {
    // tools/list
    let proxy_for_list = proxy.clone();
    server.on(
        "tools/list",
        Box::new(move |_method, _params| {
            let proxy = proxy_for_list.clone();
            Box::pin(async move {
                let tools = proxy.tool_definitions();
                let result = ListToolsResult { tools };
                Ok(serde_json::to_value(result)?)
            })
        }),
    );

    // tools/call
    server.on(
        "tools/call",
        Box::new(move |_method, params| {
            let proxy = proxy.clone();
            Box::pin(async move {
                let params = params.ok_or_else(|| {
                    McpzipError::Protocol("tools/call requires params".into())
                })?;
                let call: CallToolParams = serde_json::from_value(params)?;
                let args = call
                    .arguments
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                match call.name.as_str() {
                    "search_tools" => match proxy.handle_search_tools(args).await {
                        Ok(text) => Ok(serde_json::to_value(CallToolResult {
                            content: vec![ContentItem::Text { text }],
                            is_error: None,
                        })?),
                        Err(e) => Ok(serde_json::to_value(CallToolResult {
                            content: vec![ContentItem::Text {
                                text: format!("Error: {}", e),
                            }],
                            is_error: Some(true),
                        })?),
                    },
                    "describe_tool" => match proxy.handle_describe_tool(args) {
                        Ok(text) => Ok(serde_json::to_value(CallToolResult {
                            content: vec![ContentItem::Text { text }],
                            is_error: None,
                        })?),
                        Err(e) => Ok(serde_json::to_value(CallToolResult {
                            content: vec![ContentItem::Text {
                                text: format!("Error: {}", e),
                            }],
                            is_error: Some(true),
                        })?),
                    },
                    "execute_tool" => match proxy.handle_execute_tool(args).await {
                        Ok(result) => {
                            let text = serde_json::to_string(&result)
                                .unwrap_or_else(|_| result.to_string());
                            Ok(serde_json::to_value(CallToolResult {
                                content: vec![ContentItem::Text { text }],
                                is_error: None,
                            })?)
                        }
                        Err(e) => Ok(serde_json::to_value(CallToolResult {
                            content: vec![ContentItem::Text {
                                text: format!("Error: {}", e),
                            }],
                            is_error: Some(true),
                        })?),
                    },
                    other => Err(McpzipError::Protocol(format!(
                        "unknown tool: {:?}",
                        other
                    ))),
                }
            })
        }),
    );
}
