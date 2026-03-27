use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::error::McpzipError;
use crate::mcp::protocol::*;
use crate::mcp::transport::McpTransport;

/// Type for async handler functions.
pub type Handler = Box<
    dyn Fn(String, Option<Value>) -> Pin<Box<dyn Future<Output = Result<Value, McpzipError>> + Send>>
        + Send
        + Sync,
>;

/// MCP server that dispatches incoming JSON-RPC requests to handlers.
pub struct McpServer {
    transport: Arc<dyn McpTransport>,
    handlers: std::collections::HashMap<String, Handler>,
    capabilities: ServerCapabilities,
    instructions: Option<String>,
}

impl McpServer {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self {
            transport,
            handlers: std::collections::HashMap::new(),
            capabilities: ServerCapabilities::default(),
            instructions: None,
        }
    }

    pub fn set_capabilities(&mut self, caps: ServerCapabilities) {
        self.capabilities = caps;
    }

    pub fn set_instructions(&mut self, instructions: String) {
        self.instructions = Some(instructions);
    }

    /// Register a handler for a method. Handler receives (method, params) -> result.
    pub fn on(&mut self, method: &str, handler: Handler) {
        self.handlers.insert(method.into(), handler);
    }

    /// Run the server loop: read requests from transport, dispatch, send responses.
    pub async fn run(&self, cancel: tokio_util::sync::CancellationToken) -> Result<(), McpzipError> {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Ok(());
                }
                msg = self.transport.receive() => {
                    let msg = match msg {
                        Ok(m) => m,
                        Err(McpzipError::Transport(_)) => return Ok(()), // Clean shutdown
                        Err(e) => return Err(e),
                    };

                    match JsonRpcMessage::from_value(msg) {
                        Ok(JsonRpcMessage::Request(req)) => {
                            let resp = self.handle_request(req).await;
                            self.transport.send(serde_json::to_value(&resp)?).await?;
                        }
                        Ok(JsonRpcMessage::Notification(_)) => {
                            // Notifications are fire-and-forget
                        }
                        Ok(JsonRpcMessage::Response(_)) => {
                            // Unexpected — we're a server, ignore responses
                        }
                        Err(_) => {
                            // Malformed message — skip
                        }
                    }
                }
            }
        }
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            "initialize" => {
                let result = InitializeResult {
                    protocol_version: "2024-11-05".into(),
                    capabilities: self.capabilities.clone(),
                    server_info: ServerInfo {
                        name: "mcpzip".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    },
                    instructions: self.instructions.clone(),
                };
                match serde_json::to_value(&result) {
                    Ok(v) => make_response(req.id, v),
                    Err(e) => make_error_response(req.id, INTERNAL_ERROR, e.to_string()),
                }
            }
            method => {
                if let Some(handler) = self.handlers.get(method) {
                    match handler(method.to_string(), req.params).await {
                        Ok(result) => make_response(req.id, result),
                        Err(e) => make_error_response(req.id, INTERNAL_ERROR, e.to_string()),
                    }
                } else {
                    make_error_response(
                        req.id,
                        METHOD_NOT_FOUND,
                        format!("method not found: {}", method),
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::transport::memory_transport_pair;
    use serde_json::json;

    fn test_handler() -> Handler {
        Box::new(|_method, _params| {
            Box::pin(async { Ok(json!({"tools": []})) })
        })
    }

    #[tokio::test]
    async fn test_server_dispatch_tools_list() {
        let (client_t, server_t) = memory_transport_pair();
        let client_t = Arc::new(client_t);
        let server_t = Arc::new(server_t);

        let mut server = McpServer::new(server_t);
        server.on("tools/list", test_handler());

        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel2 = cancel.clone();

        let srv_handle = tokio::spawn(async move { server.run(cancel2).await });

        // Send initialize
        let init_req = make_request(1, "initialize", Some(json!({})));
        client_t.send(serde_json::to_value(&init_req).unwrap()).await.unwrap();
        let resp = client_t.receive().await.unwrap();
        assert!(resp.get("result").is_some());

        // Send tools/list
        let list_req = make_request(2, "tools/list", None);
        client_t.send(serde_json::to_value(&list_req).unwrap()).await.unwrap();
        let resp = client_t.receive().await.unwrap();
        assert!(resp["result"]["tools"].is_array());

        cancel.cancel();
        let _ = srv_handle.await;
    }

    #[tokio::test]
    async fn test_server_unknown_method() {
        let (client_t, server_t) = memory_transport_pair();
        let client_t = Arc::new(client_t);
        let server_t = Arc::new(server_t);

        let server = McpServer::new(server_t);
        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel2 = cancel.clone();

        let srv_handle = tokio::spawn(async move { server.run(cancel2).await });

        let req = make_request(1, "unknown/method", None);
        client_t.send(serde_json::to_value(&req).unwrap()).await.unwrap();
        let resp = client_t.receive().await.unwrap();
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);

        cancel.cancel();
        let _ = srv_handle.await;
    }

    #[tokio::test]
    async fn test_server_initialize_returns_capabilities() {
        let (client_t, server_t) = memory_transport_pair();
        let client_t = Arc::new(client_t);
        let server_t = Arc::new(server_t);

        let mut server = McpServer::new(server_t);
        server.set_capabilities(ServerCapabilities {
            tools: Some(ToolsCapability {}),
            resources: Some(ResourcesCapability {}),
            prompts: None,
        });
        server.set_instructions("Use search_tools first.".into());

        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel2 = cancel.clone();

        let srv_handle = tokio::spawn(async move { server.run(cancel2).await });

        let req = make_request(1, "initialize", Some(json!({})));
        client_t.send(serde_json::to_value(&req).unwrap()).await.unwrap();
        let resp = client_t.receive().await.unwrap();

        let result = &resp["result"];
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
        assert_eq!(result["instructions"], "Use search_tools first.");
        assert_eq!(result["serverInfo"]["name"], "mcpzip");

        cancel.cancel();
        let _ = srv_handle.await;
    }

    #[tokio::test]
    async fn test_server_notification_ignored() {
        let (client_t, server_t) = memory_transport_pair();
        let client_t = Arc::new(client_t);
        let server_t = Arc::new(server_t);

        let server = McpServer::new(server_t);
        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel2 = cancel.clone();

        let srv_handle = tokio::spawn(async move { server.run(cancel2).await });

        // Send notification — should not get a response
        let notif = make_notification("notifications/initialized", None);
        client_t.send(serde_json::to_value(&notif).unwrap()).await.unwrap();

        // Send a request to verify server is still alive
        let req = make_request(1, "initialize", Some(json!({})));
        client_t.send(serde_json::to_value(&req).unwrap()).await.unwrap();
        let resp = client_t.receive().await.unwrap();
        assert!(resp.get("result").is_some());

        cancel.cancel();
        let _ = srv_handle.await;
    }
}
