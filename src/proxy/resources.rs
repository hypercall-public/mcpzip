use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::McpzipError;
use crate::proxy::server::ProxyServer;
use crate::types;

/// An MCP resource from an upstream server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub server_name: String,
}

/// An MCP prompt from an upstream server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    pub server_name: String,
}

/// Returns a server-prefixed URI.
pub fn prefix_uri(server: &str, uri: &str) -> String {
    types::prefixed_name(server, uri)
}

/// Split a prefixed URI into (server, original_uri).
pub fn parse_prefixed_uri(prefixed: &str) -> Result<(&str, &str), McpzipError> {
    types::parse_prefixed_name(prefixed)
}

impl ProxyServer {
    /// List all resources (stub - returns empty).
    pub fn list_resources(&self) -> Vec<Resource> {
        Vec::new()
    }

    /// Read a resource by prefixed URI (stub - not yet implemented).
    pub fn read_resource(&self, prefixed_uri: &str) -> Result<Value, McpzipError> {
        let (_server, _uri) = parse_prefixed_uri(prefixed_uri)?;
        Err(McpzipError::Protocol(
            "resource reading not yet implemented".into(),
        ))
    }

    /// List all prompts (stub - returns empty).
    pub fn list_prompts(&self) -> Vec<Prompt> {
        Vec::new()
    }

    /// Get a prompt by prefixed name (stub - not yet implemented).
    pub fn get_prompt(&self, prefixed_name: &str) -> Result<Value, McpzipError> {
        let (_server, _name) = types::parse_prefixed_name(prefixed_name)?;
        Err(McpzipError::Protocol(
            "prompt retrieval not yet implemented".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_uri() {
        assert_eq!(
            prefix_uri("slack", "file:///channels.json"),
            "slack__file:///channels.json"
        );
    }

    #[test]
    fn test_parse_prefixed_uri() {
        let (server, uri) = parse_prefixed_uri("slack__file:///channels.json").unwrap();
        assert_eq!(server, "slack");
        assert_eq!(uri, "file:///channels.json");
    }

    #[test]
    fn test_parse_prefixed_uri_invalid() {
        assert!(parse_prefixed_uri("no-separator").is_err());
    }
}
