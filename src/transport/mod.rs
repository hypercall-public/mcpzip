pub mod http;
pub mod manager;
pub mod sse;
pub mod stdio;

pub use manager::Manager;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::McpzipError;
use crate::types::{ServerConfig, ToolEntry};

/// An upstream MCP server connection.
#[async_trait]
pub trait Upstream: Send + Sync {
    /// List all tools from this upstream server.
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError>;

    /// Invoke a tool and return the raw JSON result.
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, McpzipError>;

    /// Shut down the connection.
    async fn close(&self) -> Result<(), McpzipError>;

    /// Check if the connection is still usable.
    fn alive(&self) -> bool;
}

/// Factory function type for creating upstream connections.
pub type ConnectFn = Arc<
    dyn Fn(
            String,
            ServerConfig,
        ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Upstream>, McpzipError>> + Send>>
        + Send
        + Sync,
>;
