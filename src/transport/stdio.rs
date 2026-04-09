use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::error::McpzipError;
use crate::mcp::client::McpClient;
use crate::mcp::protocol::{ContentItem, ToolInfo};
use crate::mcp::transport::NdjsonTransport;
use crate::transport::Upstream;
use crate::types::{compact_params_from_schema, prefixed_name, ServerConfig, ToolEntry};

/// Upstream connection via a stdio subprocess.
pub struct StdioUpstream {
    name: String,
    client: McpClient,
    alive: AtomicBool,
    child: tokio::sync::Mutex<Option<tokio::process::Child>>,
}

impl StdioUpstream {
    /// Spawn a subprocess and perform MCP handshake.
    pub async fn new(name: String, cfg: &ServerConfig) -> Result<Self, McpzipError> {
        let command = cfg
            .command
            .as_deref()
            .ok_or_else(|| McpzipError::Config(format!("server {:?}: missing command", name)))?;

        let mut cmd = Command::new(command);
        if let Some(args) = &cfg.args {
            cmd.args(args);
        }
        if let Some(env) = &cfg.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| McpzipError::Transport(format!("failed to spawn {:?}: {}", command, e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpzipError::Transport("failed to capture stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpzipError::Transport("failed to capture stdout".into()))?;

        let transport = Arc::new(NdjsonTransport::new(Box::new(stdout), Box::new(stdin)));
        let client = McpClient::new(transport);

        // Perform MCP handshake
        client.initialize().await.map_err(|e| {
            McpzipError::Transport(format!("handshake failed for {:?}: {}", name, e))
        })?;

        Ok(Self {
            name,
            client,
            alive: AtomicBool::new(true),
            child: tokio::sync::Mutex::new(Some(child)),
        })
    }
}

#[async_trait]
impl Upstream for StdioUpstream {
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
        let tools = self.client.list_tools().await?;
        Ok(tools
            .into_iter()
            .map(|t| tool_info_to_entry(&self.name, t))
            .collect())
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<Value, McpzipError> {
        let result = self.client.call_tool(tool_name, args).await?;

        // Convert CallToolResult to raw JSON value, matching Go behavior:
        // - Single text content that's valid JSON: return the text as raw JSON
        // - Otherwise: return the full result as JSON
        if result.content.len() == 1 {
            if let ContentItem::Text { ref text } = result.content[0] {
                if serde_json::from_str::<Value>(text).is_ok() {
                    return Ok(serde_json::from_str(text)?);
                }
                return Ok(Value::String(text.clone()));
            }
        }
        Ok(serde_json::to_value(&result)?)
    }

    async fn close(&self) -> Result<(), McpzipError> {
        self.alive.store(false, Ordering::Relaxed);
        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

fn tool_info_to_entry(server_name: &str, info: ToolInfo) -> ToolEntry {
    let schema = info.input_schema.unwrap_or(Value::Null);
    let compact = compact_params_from_schema(&schema);
    ToolEntry {
        name: prefixed_name(server_name, &info.name),
        server_name: server_name.into(),
        original_name: info.name,
        description: info.description.unwrap_or_default(),
        input_schema: schema,
        compact_params: compact,
    }
}
