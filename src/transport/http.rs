use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use crate::auth::oauth::OAuthHandler;
use crate::error::McpzipError;
use crate::transport::Upstream;
use crate::types::{ServerConfig, ToolEntry};

/// Upstream connection via Streamable HTTP (MCP 2025-03-26 spec).
/// Handles both JSON and SSE responses per the spec.
pub struct HttpUpstream {
    name: String,
    url: String,
    client: reqwest::Client,
    session_id: tokio::sync::Mutex<Option<String>>,
    oauth: Option<OAuthHandler>,
    alive: AtomicBool,
    request_id: AtomicU64,
}

impl HttpUpstream {
    pub async fn new(
        name: String,
        cfg: &ServerConfig,
        oauth: Option<OAuthHandler>,
    ) -> Result<Self, McpzipError> {
        let url = cfg.url.as_deref().ok_or_else(|| {
            McpzipError::Config(format!("server {:?}: missing url", name))
        })?;

        let client = reqwest::Client::new();

        let upstream = Self {
            name,
            url: url.into(),
            client,
            session_id: tokio::sync::Mutex::new(None),
            oauth,
            alive: AtomicBool::new(true),
            request_id: AtomicU64::new(1),
        };

        // Perform MCP handshake via HTTP POST
        upstream.initialize().await?;

        Ok(upstream)
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn initialize(&self) -> Result<(), McpzipError> {
        use crate::mcp::protocol::*;

        let req = make_request(
            self.next_id(),
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "mcpzip", "version": env!("CARGO_PKG_VERSION")}
            })),
        );

        let _resp = self.post_jsonrpc(&serde_json::to_value(&req)?).await?;

        // Send initialized notification
        let notif = make_notification("notifications/initialized", None);
        // Notifications return 202 with no body - ignore errors
        let _ = self.post_jsonrpc(&serde_json::to_value(&notif)?).await;

        Ok(())
    }

    async fn post_jsonrpc(&self, body: &Value) -> Result<Value, McpzipError> {
        let mut req = self.client.post(&self.url)
            .json(body)
            .header("Accept", "application/json, text/event-stream");

        // Add session ID if we have one
        if let Some(ref sid) = *self.session_id.lock().await {
            req = req.header("Mcp-Session-Id", sid);
        }

        // Add OAuth token if available
        if let Some(ref oauth) = self.oauth {
            match oauth.authorization_header().await {
                Ok(header) => {
                    req = req.header("Authorization", header);
                }
                Err(_) => {
                    // First request may not need auth
                }
            }
        }

        let resp = req.send().await.map_err(|e| {
            McpzipError::Http(format!("POST to {} failed: {}", self.url, e))
        })?;

        // Handle 401 - trigger OAuth flow and retry
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return self.handle_401(body, &resp).await;
        }

        // 202 Accepted = notification acknowledged, no body
        if resp.status() == reqwest::StatusCode::ACCEPTED {
            return Ok(Value::Null);
        }

        if !resp.status().is_success() {
            return Err(McpzipError::Http(format!("HTTP {}", resp.status())));
        }

        // Store session ID from response headers
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(s) = sid.to_str() {
                *self.session_id.lock().await = Some(s.into());
            }
        }

        // Check Content-Type to decide parsing strategy
        let content_type = resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        if content_type.contains("text/event-stream") {
            self.parse_sse_response(resp).await
        } else {
            resp.json()
                .await
                .map_err(|e| McpzipError::Http(format!("error decoding response body: {}", e)))
        }
    }

    /// Parse an SSE response, extracting JSON-RPC messages from `data:` lines.
    /// Returns the first JSON-RPC response found (the one matching our request).
    async fn parse_sse_response(&self, resp: reqwest::Response) -> Result<Value, McpzipError> {
        let text = resp.text().await.map_err(|e| {
            McpzipError::Http(format!("reading SSE body: {}", e))
        })?;

        // SSE format: lines starting with "data: " contain JSON-RPC messages.
        // Events are separated by blank lines.
        for line in text.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }
                // Try to parse as JSON-RPC
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    // Return the first JSON-RPC response (has "result" or "error" or "id")
                    if value.get("result").is_some()
                        || value.get("error").is_some()
                        || value.get("id").is_some()
                    {
                        return Ok(value);
                    }
                }
            }
        }

        Err(McpzipError::Http("no JSON-RPC response found in SSE stream".into()))
    }

    async fn handle_401(&self, body: &Value, resp: &reqwest::Response) -> Result<Value, McpzipError> {
        if let Some(ref oauth) = self.oauth {
            // Extract resource_metadata from WWW-Authenticate header if present
            let www_auth = resp.headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            // Try to get token (cached or via browser flow)
            let _token = oauth.get_token_with_hint(www_auth, &self.url).await?;

            // Retry with the new token
            let header = oauth.authorization_header().await?;
            let mut retry_req = self.client
                .post(&self.url)
                .json(body)
                .header("Accept", "application/json, text/event-stream")
                .header("Authorization", header);

            if let Some(ref sid) = *self.session_id.lock().await {
                retry_req = retry_req.header("Mcp-Session-Id", sid);
            }

            let retry_resp = retry_req.send().await
                .map_err(|e| McpzipError::Http(e.to_string()))?;

            if retry_resp.status() == reqwest::StatusCode::ACCEPTED {
                return Ok(Value::Null);
            }

            if !retry_resp.status().is_success() {
                return Err(McpzipError::Http(format!(
                    "HTTP {} after auth",
                    retry_resp.status()
                )));
            }

            // Store session ID from retry response
            if let Some(sid) = retry_resp.headers().get("mcp-session-id") {
                if let Ok(s) = sid.to_str() {
                    *self.session_id.lock().await = Some(s.into());
                }
            }

            let content_type = retry_resp.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();

            if content_type.contains("text/event-stream") {
                return self.parse_sse_response(retry_resp).await;
            }

            return retry_resp.json().await
                .map_err(|e| McpzipError::Http(e.to_string()));
        }
        Err(McpzipError::Auth("server returned 401, no OAuth handler".into()))
    }
}

#[async_trait]
impl Upstream for HttpUpstream {
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
        use crate::mcp::protocol::*;
        use crate::types::{compact_params_from_schema, prefixed_name};

        let req = make_request(self.next_id(), "tools/list", None);
        let resp = self.post_jsonrpc(&serde_json::to_value(&req)?).await?;

        let result: ListToolsResult = serde_json::from_value(
            resp.get("result")
                .cloned()
                .unwrap_or(serde_json::json!({"tools": []})),
        )?;

        Ok(result
            .tools
            .into_iter()
            .map(|t| {
                let schema = t.input_schema.unwrap_or(Value::Null);
                let compact = compact_params_from_schema(&schema);
                ToolEntry {
                    name: prefixed_name(&self.name, &t.name),
                    server_name: self.name.clone(),
                    original_name: t.name,
                    description: t.description.unwrap_or_default(),
                    input_schema: schema,
                    compact_params: compact,
                }
            })
            .collect())
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, McpzipError> {
        use crate::mcp::protocol::*;

        let req = make_request(
            self.next_id(),
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": args
            })),
        );

        let resp = self.post_jsonrpc(&serde_json::to_value(&req)?).await?;

        if let Some(result) = resp.get("result") {
            // Try to extract text content as raw JSON
            if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                if content.len() == 1 {
                    if let Some(text) = content[0].get("text").and_then(|t| t.as_str()) {
                        if let Ok(v) = serde_json::from_str::<Value>(text) {
                            return Ok(v);
                        }
                        return Ok(Value::String(text.into()));
                    }
                }
            }
            return Ok(result.clone());
        }

        if let Some(error) = resp.get("error") {
            return Err(McpzipError::Protocol(format!("RPC error: {}", error)));
        }

        Err(McpzipError::Protocol("no result or error in response".into()))
    }

    async fn close(&self) -> Result<(), McpzipError> {
        self.alive.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}
