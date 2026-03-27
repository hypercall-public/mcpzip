use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use crate::error::McpzipError;
use crate::transport::Upstream;
use crate::types::{ServerConfig, ToolEntry};

/// Legacy SSE upstream transport.
/// Uses persistent GET for server-sent events + separate POST for client messages.
pub struct SseUpstream {
    name: String,
    _url: String,
    alive: AtomicBool,
}

impl SseUpstream {
    pub async fn new(name: String, cfg: &ServerConfig) -> Result<Self, McpzipError> {
        let url = cfg.url.as_deref().ok_or_else(|| {
            McpzipError::Config(format!("server {:?}: missing url", name))
        })?;

        // TODO: Full SSE implementation
        // 1. Connect to URL via GET, receive SSE stream
        // 2. Parse SSE events for endpoint URL
        // 3. Send JSON-RPC via POST to endpoint URL
        // 4. Receive responses via SSE stream

        Ok(Self {
            name,
            _url: url.into(),
            alive: AtomicBool::new(true),
        })
    }
}

#[async_trait]
impl Upstream for SseUpstream {
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
        Err(McpzipError::Transport(format!(
            "SSE transport not yet implemented for {:?}",
            self.name
        )))
    }

    async fn call_tool(
        &self,
        _tool_name: &str,
        _args: Value,
    ) -> Result<Value, McpzipError> {
        Err(McpzipError::Transport(format!(
            "SSE transport not yet implemented for {:?}",
            self.name
        )))
    }

    async fn close(&self) -> Result<(), McpzipError> {
        self.alive.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}
