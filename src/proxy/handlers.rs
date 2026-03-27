use std::fmt::Write;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use crate::error::McpzipError;
use crate::proxy::server::ProxyServer;

#[derive(Deserialize)]
struct SearchToolsArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct DescribeToolArgs {
    name: String,
}

#[derive(Deserialize)]
struct ExecuteToolArgs {
    name: String,
    #[serde(default)]
    arguments: Option<Value>,
    #[serde(default)]
    timeout: Option<u64>,
}

const DEFAULT_SEARCH_LIMIT: usize = 5;
const MAX_SEARCH_LIMIT: usize = 50;

impl ProxyServer {
    /// Handle the search_tools meta-tool.
    pub async fn handle_search_tools(&self, raw_args: Value) -> Result<String, McpzipError> {
        let args: SearchToolsArgs = serde_json::from_value(raw_args)
            .map_err(|e| McpzipError::Protocol(format!("invalid search_tools arguments: {}", e)))?;

        if args.query.is_empty() {
            return Err(McpzipError::Protocol("query is required".into()));
        }

        let mut limit = args.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
        if limit == 0 {
            limit = DEFAULT_SEARCH_LIMIT;
        } else if limit > MAX_SEARCH_LIMIT {
            limit = MAX_SEARCH_LIMIT;
        }

        let results = self.searcher.search(&args.query, limit).await?;

        if results.is_empty() {
            return Ok("No tools found matching your query.".into());
        }

        let mut sb = String::new();
        for (i, r) in results.iter().enumerate() {
            if i > 0 {
                sb.push_str("\n\n");
            }
            write!(sb, "**{}**", r.name).unwrap();
            if !r.description.is_empty() {
                write!(sb, "\n{}", r.description).unwrap();
            }
            if !r.compact_params.is_empty() {
                write!(sb, "\nParams: {}", r.compact_params).unwrap();
            }
        }
        Ok(sb)
    }

    /// Handle the describe_tool meta-tool.
    pub fn handle_describe_tool(&self, raw_args: Value) -> Result<String, McpzipError> {
        let args: DescribeToolArgs = serde_json::from_value(raw_args)
            .map_err(|e| McpzipError::Protocol(format!("invalid describe_tool arguments: {}", e)))?;

        if args.name.is_empty() {
            return Err(McpzipError::Protocol("name is required".into()));
        }

        let tool = self.catalog.get_tool(&args.name)?;

        let mut sb = String::new();
        writeln!(sb, "**{}**", tool.name).unwrap();
        writeln!(sb, "Server: {}", tool.server_name).unwrap();
        writeln!(sb, "Original name: {}", tool.original_name).unwrap();
        if !tool.description.is_empty() {
            writeln!(sb, "\n{}", tool.description).unwrap();
        }
        if !tool.input_schema.is_null() {
            sb.push_str("\nInput Schema:\n```json\n");
            if let Ok(pretty) = serde_json::to_string_pretty(&tool.input_schema) {
                sb.push_str(&pretty);
            } else {
                sb.push_str(&tool.input_schema.to_string());
            }
            sb.push_str("\n```");
        }
        Ok(sb)
    }

    /// Handle the execute_tool meta-tool.
    pub async fn handle_execute_tool(&self, raw_args: Value) -> Result<Value, McpzipError> {
        let mut args: ExecuteToolArgs = serde_json::from_value(raw_args)
            .map_err(|e| McpzipError::Protocol(format!("invalid execute_tool arguments: {}", e)))?;

        if args.name.is_empty() {
            return Err(McpzipError::Protocol("name is required".into()));
        }

        // LLMs sometimes double-encode arguments as a JSON string.
        // Unwrap one level: "\"...\"" -> {...}
        if let Some(Value::String(s)) = &args.arguments {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                args.arguments = Some(parsed);
            }
        }

        let arguments = args.arguments.unwrap_or(Value::Object(Default::default()));

        // Handle admin tools before catalog lookup.
        match args.name.as_str() {
            "proxy_status" => return self.handle_proxy_status(),
            "proxy_refresh" => return self.handle_proxy_refresh().await,
            _ => {}
        }

        // Verify tool exists in catalog.
        let _ = self.catalog.get_tool(&args.name)?;

        // Parse prefixed name.
        let (server_name, tool_name) = crate::types::parse_prefixed_name(&args.name)?;

        // Apply per-call timeout if specified.
        if let Some(timeout_secs) = args.timeout {
            if timeout_secs > 0 {
                let result: Result<Value, McpzipError> = tokio::time::timeout(
                    Duration::from_secs(timeout_secs),
                    self.transport.call_tool(server_name, tool_name, arguments),
                )
                .await
                .map_err(|_| McpzipError::Timeout(timeout_secs))?;
                return result;
            }
        }

        self.transport
            .call_tool(server_name, tool_name, arguments)
            .await
    }

    fn handle_proxy_status(&self) -> Result<Value, McpzipError> {
        Ok(serde_json::json!({
            "tool_count": self.catalog.tool_count(),
            "server_names": self.catalog.server_names(),
        }))
    }

    async fn handle_proxy_refresh(&self) -> Result<Value, McpzipError> {
        let server_tools = self.transport.list_tools_all().await?;
        self.catalog.refresh(server_tools)?;
        Ok(serde_json::json!({
            "status": "refreshed",
            "tool_count": self.catalog.tool_count(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::search;
    use crate::transport::{ConnectFn, Manager, Upstream};
    use crate::types::{ServerConfig, ToolEntry};
    use serde_json::json;
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::Arc;

    struct MockUpstream {
        name: String,
    }

    #[async_trait::async_trait]
    impl Upstream for MockUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
            Ok(vec![])
        }

        async fn call_tool(
            &self,
            tool_name: &str,
            _args: Value,
        ) -> Result<Value, McpzipError> {
            Ok(json!({
                "server": self.name,
                "tool": tool_name,
                "response": "ok",
            }))
        }

        async fn close(&self) -> Result<(), McpzipError> {
            Ok(())
        }

        fn alive(&self) -> bool {
            true
        }
    }

    fn setup_test_server() -> ProxyServer {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Arc::new(Catalog::new(dir.path().join("tools.json")));

        let mut server_tools = HashMap::new();
        server_tools.insert(
            "slack".into(),
            vec![
                ToolEntry {
                    name: "slack__channels_list".into(),
                    server_name: "slack".into(),
                    original_name: "channels_list".into(),
                    description: "List Slack channels".into(),
                    input_schema: json!({"type":"object","properties":{"limit":{"type":"integer"}}}),
                    compact_params: "limit:integer".into(),
                },
                ToolEntry {
                    name: "slack__send_message".into(),
                    server_name: "slack".into(),
                    original_name: "send_message".into(),
                    description: "Send a Slack message".into(),
                    input_schema: json!({"type":"object","properties":{"channel":{"type":"string"},"text":{"type":"string"}},"required":["channel","text"]}),
                    compact_params: "channel:string*, text:string*".into(),
                },
            ],
        );
        catalog.refresh(server_tools).unwrap();

        let catalog_for_search = catalog.clone();
        let catalog_fn: search::CatalogFn = Arc::new(move || catalog_for_search.all_tools());
        let searcher = search::new_searcher("", "", catalog_fn);

        let mut configs = HashMap::new();
        configs.insert(
            "slack".into(),
            ServerConfig {
                server_type: None,
                command: Some("slack-mcp".into()),
                args: None,
                env: None,
                url: None,
            },
        );

        let connect: ConnectFn = Arc::new(|name: String, _cfg: ServerConfig| {
            Box::pin(async move {
                Ok(Box::new(MockUpstream { name }) as Box<dyn Upstream>)
            }) as Pin<Box<dyn std::future::Future<Output = Result<Box<dyn Upstream>, McpzipError>> + Send>>
        });

        let transport = Arc::new(Manager::new(
            configs,
            Duration::from_secs(300),
            Duration::from_secs(120),
            connect,
        ));

        ProxyServer::new(catalog, searcher, transport)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_search_tools() {
        let s = setup_test_server();
        let result = s
            .handle_search_tools(json!({"query": "slack channels"}))
            .await
            .unwrap();
        assert!(result.contains("slack__channels_list"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_search_tools_no_results() {
        let s = setup_test_server();
        let result = s
            .handle_search_tools(json!({"query": "nonexistent_xyz"}))
            .await
            .unwrap();
        assert!(result.contains("No tools found"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_search_tools_empty_query() {
        let s = setup_test_server();
        let result = s.handle_search_tools(json!({"query": ""})).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_search_tools_with_limit() {
        let s = setup_test_server();
        let result = s
            .handle_search_tools(json!({"query": "slack", "limit": 1}))
            .await
            .unwrap();
        let parts: Vec<&str> = result.split("\n\n").collect();
        assert_eq!(parts.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_search_tools_limit_capped() {
        let s = setup_test_server();
        // Should not panic or OOM with large limit.
        let result = s
            .handle_search_tools(json!({"query": "slack", "limit": 9999}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_describe_tool() {
        let s = setup_test_server();
        let result = s
            .handle_describe_tool(json!({"name": "slack__send_message"}))
            .unwrap();
        assert!(result.contains("slack__send_message"));
        assert!(result.contains("Input Schema"));
        assert!(result.contains("channel"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_describe_tool_unknown() {
        let s = setup_test_server();
        let result = s.handle_describe_tool(json!({"name": "unknown__tool"}));
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_describe_tool_empty_name() {
        let s = setup_test_server();
        let result = s.handle_describe_tool(json!({"name": ""}));
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool() {
        let s = setup_test_server();
        let result = s
            .handle_execute_tool(json!({
                "name": "slack__send_message",
                "arguments": {"channel": "#general", "text": "hello"}
            }))
            .await
            .unwrap();

        let resp: HashMap<String, String> = serde_json::from_value(result).unwrap();
        assert_eq!(resp["server"], "slack");
        assert_eq!(resp["tool"], "send_message");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool_unknown() {
        let s = setup_test_server();
        let result = s
            .handle_execute_tool(json!({"name": "unknown__tool", "arguments": {}}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool_empty_name() {
        let s = setup_test_server();
        let result = s
            .handle_execute_tool(json!({"name": "", "arguments": {}}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool_string_arguments() {
        let s = setup_test_server();
        // LLMs sometimes send arguments as a JSON string.
        let result = s
            .handle_execute_tool(json!({
                "name": "slack__send_message",
                "arguments": "{\"channel\": \"#general\", \"text\": \"hello\"}"
            }))
            .await
            .unwrap();

        let resp: HashMap<String, String> = serde_json::from_value(result).unwrap();
        assert_eq!(resp["server"], "slack");
        assert_eq!(resp["tool"], "send_message");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool_proxy_status() {
        let s = setup_test_server();
        let result = s
            .handle_execute_tool(json!({"name": "proxy_status", "arguments": {}}))
            .await
            .unwrap();

        let resp = result.as_object().unwrap();
        assert_eq!(resp["tool_count"], 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_handle_execute_tool_invalid_args() {
        let s = setup_test_server();
        // Pass a non-object value that can't deserialize to ExecuteToolArgs.
        let result = s.handle_execute_tool(Value::String("invalid".into())).await;
        assert!(result.is_err());
    }
}
