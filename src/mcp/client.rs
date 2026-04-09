use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[allow(unused_imports)]
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex};

use crate::error::McpzipError;
use crate::mcp::protocol::*;
use crate::mcp::transport::McpTransport;

/// MCP client that communicates with an upstream MCP server.
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
}

impl McpClient {
    /// Create a new MCP client with the given transport.
    /// Starts a background reader task that dispatches responses.
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_transport = transport.clone();
        let reader_pending = pending.clone();

        let reader_handle = tokio::spawn(async move {
            while let Ok(msg) = reader_transport.receive().await {
                match JsonRpcMessage::from_value(msg) {
                    Ok(JsonRpcMessage::Response(resp)) => {
                        if let Id::Number(id) = &resp.id {
                            let mut pending = reader_pending.lock().await;
                            if let Some(tx) = pending.remove(id) {
                                let _ = tx.send(resp);
                            }
                        }
                    }
                    Ok(JsonRpcMessage::Request(_) | JsonRpcMessage::Notification(_)) => {
                        // Server-initiated requests/notifications — ignore for now
                    }
                    Err(_) => {
                        // Malformed message — skip
                    }
                }
            }
        });

        Self {
            transport,
            next_id: AtomicU64::new(1),
            pending,
            reader_handle: Some(reader_handle),
        }
    }

    /// Send a request and wait for the matching response.
    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, McpzipError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = make_request(id, method, params);

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        self.transport.send(serde_json::to_value(&req)?).await?;

        let resp = rx.await.map_err(|_| {
            McpzipError::Transport("response channel dropped (transport closed)".into())
        })?;

        if let Some(err) = resp.error {
            return Err(McpzipError::Protocol(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        resp.result
            .ok_or_else(|| McpzipError::Protocol("response has neither result nor error".into()))
    }

    /// Send a notification (no response expected).
    async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpzipError> {
        let notif = make_notification(method, params);
        self.transport.send(serde_json::to_value(&notif)?).await
    }

    /// Perform the MCP initialize handshake.
    pub async fn initialize(&self) -> Result<InitializeResult, McpzipError> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".into(),
            capabilities: ClientCapabilities {},
            client_info: ClientInfo {
                name: "mcpzip".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
        };

        let result = self
            .request("initialize", Some(serde_json::to_value(&params)?))
            .await?;
        let init_result: InitializeResult = serde_json::from_value(result)?;

        // Send initialized notification
        self.notify("notifications/initialized", None).await?;

        Ok(init_result)
    }

    /// List all tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>, McpzipError> {
        let result = self.request("tools/list", None).await?;
        let list: ListToolsResult = serde_json::from_value(result)?;
        Ok(list.tools)
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<CallToolResult, McpzipError> {
        let params = CallToolParams {
            name: name.into(),
            arguments: Some(arguments),
        };
        let result = self
            .request("tools/call", Some(serde_json::to_value(&params)?))
            .await?;
        let call_result: CallToolResult = serde_json::from_value(result)?;
        Ok(call_result)
    }

    /// Shut down the client, aborting the reader task.
    pub fn close(&mut self) {
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::transport::memory_transport_pair;

    /// Helper: run a mock MCP server that responds to requests.
    async fn mock_server(transport: Arc<dyn McpTransport>, tools: Vec<ToolInfo>) {
        loop {
            let msg = match transport.receive().await {
                Ok(m) => m,
                Err(_) => break,
            };
            let parsed = match JsonRpcMessage::from_value(msg) {
                Ok(m) => m,
                Err(_) => continue,
            };

            match parsed {
                JsonRpcMessage::Request(req) => {
                    let result = match req.method.as_str() {
                        "initialize" => json!({
                            "protocolVersion": "2024-11-05",
                            "capabilities": {"tools": {}},
                            "serverInfo": {"name": "mock", "version": "1.0"}
                        }),
                        "tools/list" => json!({"tools": tools}),
                        "tools/call" => {
                            let params: CallToolParams =
                                serde_json::from_value(req.params.unwrap_or_default()).unwrap();
                            json!({
                                "content": [{"type": "text", "text": format!("called {}", params.name)}]
                            })
                        }
                        _ => {
                            let resp = make_error_response(
                                req.id,
                                METHOD_NOT_FOUND,
                                "unknown method".into(),
                            );
                            transport
                                .send(serde_json::to_value(&resp).unwrap())
                                .await
                                .unwrap();
                            continue;
                        }
                    };
                    let resp = make_response(req.id, result);
                    transport
                        .send(serde_json::to_value(&resp).unwrap())
                        .await
                        .unwrap();
                }
                JsonRpcMessage::Notification(_) => {
                    // Ignore notifications
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_initialize_handshake() {
        let (client_transport, server_transport) = memory_transport_pair();
        let client_transport = Arc::new(client_transport);
        let server_transport = Arc::new(server_transport);

        let server = tokio::spawn(mock_server(server_transport, vec![]));
        let client = McpClient::new(client_transport);

        let result = client.initialize().await.unwrap();
        assert_eq!(result.protocol_version, "2024-11-05");
        assert_eq!(result.server_info.name, "mock");

        server.abort();
    }

    #[tokio::test]
    async fn test_list_tools() {
        let (ct, st) = memory_transport_pair();
        let ct = Arc::new(ct);
        let st = Arc::new(st);

        let tools = vec![ToolInfo {
            name: "send_message".into(),
            description: Some("Send a msg".into()),
            input_schema: Some(json!({"type": "object"})),
        }];

        let server = tokio::spawn(mock_server(st, tools));
        let client = McpClient::new(ct);
        client.initialize().await.unwrap();

        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "send_message");

        server.abort();
    }

    #[tokio::test]
    async fn test_call_tool() {
        let (ct, st) = memory_transport_pair();
        let ct = Arc::new(ct);
        let st = Arc::new(st);

        let server = tokio::spawn(mock_server(st, vec![]));
        let client = McpClient::new(ct);
        client.initialize().await.unwrap();

        let result = client
            .call_tool("my_tool", json!({"key": "value"}))
            .await
            .unwrap();
        assert_eq!(result.content.len(), 1);
        if let ContentItem::Text { text } = &result.content[0] {
            assert!(text.contains("my_tool"));
        } else {
            panic!("expected text content");
        }

        server.abort();
    }

    #[tokio::test]
    async fn test_request_id_increments() {
        let (ct, st) = memory_transport_pair();
        let ct = Arc::new(ct);
        let st = Arc::new(st);

        let server = tokio::spawn(mock_server(st, vec![]));
        let client = McpClient::new(ct);
        client.initialize().await.unwrap();

        // After initialize (id=1) and list_tools calls, IDs should increment
        let _tools1 = client.list_tools().await.unwrap();
        let _tools2 = client.list_tools().await.unwrap();
        // If we got here without error, IDs were correctly matched
        assert!(client.next_id.load(Ordering::Relaxed) >= 4);

        server.abort();
    }

    #[tokio::test]
    async fn test_error_response() {
        let (ct, st) = memory_transport_pair();
        let ct = Arc::new(ct);
        let st = Arc::new(st);

        let server = tokio::spawn(mock_server(st, vec![]));
        let client = McpClient::new(ct);
        client.initialize().await.unwrap();

        let err = client.request("unknown/method", None).await.unwrap_err();
        assert!(err.to_string().contains("unknown method"));

        server.abort();
    }
}
