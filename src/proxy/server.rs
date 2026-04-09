use std::sync::Arc;

#[allow(unused_imports)]
use serde_json::Value;

use crate::catalog::Catalog;
#[allow(unused_imports)]
use crate::error::McpzipError;
use crate::mcp::protocol::ToolInfo;
use crate::search::Searcher;
use crate::transport::Manager;

/// The core MCP proxy that exposes 3 meta-tools.
pub struct ProxyServer {
    pub(crate) catalog: Arc<Catalog>,
    pub(crate) searcher: Box<dyn Searcher>,
    pub(crate) transport: Arc<Manager>,
}

impl ProxyServer {
    pub fn new(
        catalog: Arc<Catalog>,
        searcher: Box<dyn Searcher>,
        transport: Arc<Manager>,
    ) -> Self {
        Self {
            catalog,
            searcher,
            transport,
        }
    }

    /// Build the MCP tool definitions for the 3 meta-tools.
    pub fn tool_definitions(&self) -> Vec<ToolInfo> {
        vec![
            ToolInfo {
                name: "search_tools".into(),
                description: Some("Search for available tools by keyword query. Returns matching tool names, descriptions, and parameter summaries.".into()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query to find tools (e.g. 'send message', 'list channels')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 5, max: 50)"
                        }
                    },
                    "required": ["query"]
                })),
            },
            ToolInfo {
                name: "describe_tool".into(),
                description: Some("Get the full description and input schema for a specific tool. Use the prefixed name from search_tools results.".into()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The prefixed tool name (e.g. 'slack__send_message')"
                        }
                    },
                    "required": ["name"]
                })),
            },
            ToolInfo {
                name: "execute_tool".into(),
                description: Some("Execute a tool on its upstream MCP server. Use the prefixed name from search_tools results and provide the required arguments.".into()),
                input_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The prefixed tool name (e.g. 'slack__send_message')"
                        },
                        "arguments": {
                            "type": "object",
                            "description": "Arguments to pass to the tool"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in seconds for this tool call (default: uses proxy default)"
                        }
                    },
                    "required": ["name"]
                })),
            },
        ]
    }

    /// Generate instructions for the proxy.
    pub fn instructions(&self) -> String {
        let server_names = self.catalog.server_names();
        if server_names.is_empty() {
            return "mcpzip proxy - use search_tools to discover available tools.".into();
        }

        let mut sb = String::from("mcpzip proxy aggregates tools from the following servers:\n");
        for name in &server_names {
            let tools = self.catalog.server_tools(name);
            sb.push_str(&format!("- {} ({} tools)\n", name, tools.len()));
        }
        sb.push_str(
            "\nUse search_tools to discover tools, describe_tool for details, execute_tool to invoke them.",
        );
        sb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::search;
    use crate::transport::{ConnectFn, Upstream};
    use crate::types::ToolEntry;
    use serde_json::json;
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::time::Duration;

    struct MockUpstream;

    #[async_trait::async_trait]
    impl Upstream for MockUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
            Ok(vec![])
        }
        async fn call_tool(&self, _: &str, _: Value) -> Result<Value, McpzipError> {
            Ok(json!({}))
        }
        async fn close(&self) -> Result<(), McpzipError> {
            Ok(())
        }
        fn alive(&self) -> bool {
            true
        }
    }

    fn make_proxy(with_tools: bool) -> ProxyServer {
        let dir = tempfile::tempdir().unwrap();
        let catalog = Arc::new(Catalog::new(dir.path().join("tools.json")));

        if with_tools {
            let mut server_tools = HashMap::new();
            server_tools.insert(
                "slack".into(),
                vec![ToolEntry {
                    name: "slack__send".into(),
                    server_name: "slack".into(),
                    original_name: "send".into(),
                    description: "Send message".into(),
                    input_schema: json!(null),
                    compact_params: "".into(),
                }],
            );
            catalog.refresh(server_tools).unwrap();
        }

        let catalog_for_search = catalog.clone();
        let searcher =
            search::new_searcher("", "", Arc::new(move || catalog_for_search.all_tools()));

        let connect: ConnectFn = Arc::new(|_name, _cfg| {
            Box::pin(async { Ok(Box::new(MockUpstream) as Box<dyn Upstream>) })
                as Pin<
                    Box<
                        dyn std::future::Future<Output = Result<Box<dyn Upstream>, McpzipError>>
                            + Send,
                    >,
                >
        });

        let transport = Arc::new(Manager::new(
            HashMap::new(),
            Duration::from_secs(300),
            Duration::from_secs(120),
            connect,
        ));

        ProxyServer::new(catalog, searcher, transport)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_instructions_with_servers() {
        let proxy = make_proxy(true);
        let instructions = proxy.instructions();
        assert!(instructions.contains("slack"));
        assert!(instructions.contains("search_tools"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_instructions_empty() {
        let proxy = make_proxy(false);
        let instructions = proxy.instructions();
        assert!(instructions.contains("search_tools"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_tool_definitions() {
        let proxy = make_proxy(false);
        let defs = proxy.tool_definitions();
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, "search_tools");
        assert_eq!(defs[1].name, "describe_tool");
        assert_eq!(defs[2].name, "execute_tool");
    }
}
