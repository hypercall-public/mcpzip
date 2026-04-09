use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const NAME_SEPARATOR: &str = "__";

/// A cached tool from an upstream MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub server_name: String,
    pub original_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub compact_params: String,
}

/// Result from search engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub compact_params: String,
}

/// How to connect to an upstream MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

impl ServerConfig {
    pub fn effective_type(&self) -> &str {
        match self.server_type.as_deref() {
            Some(t) if !t.is_empty() => t,
            _ => "stdio",
        }
    }
}

/// Search engine settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Full proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini_api_key: Option<String>,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_timeout_seconds: Option<u64>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, ServerConfig>,
}

/// Health info for an upstream server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub last_refresh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Returns "server__tool".
pub fn prefixed_name(server: &str, tool: &str) -> String {
    format!("{}{}{}", server, NAME_SEPARATOR, tool)
}

/// Splits "server__tool" into (server, tool). Splits on first occurrence of "__".
pub fn parse_prefixed_name(name: &str) -> Result<(&str, &str), crate::error::McpzipError> {
    match name.find(NAME_SEPARATOR) {
        Some(idx) => Ok((&name[..idx], &name[idx + NAME_SEPARATOR.len()..])),
        None => Err(crate::error::McpzipError::Protocol(format!(
            "invalid prefixed name {:?}: missing separator {:?}",
            name, NAME_SEPARATOR
        ))),
    }
}

/// Generate compact parameter summary from a JSON Schema.
/// Format: "param1:type*, param2:type" where * marks required params.
pub fn compact_params_from_schema(schema: &serde_json::Value) -> String {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return String::new(),
    };

    let properties = match obj.get("properties").and_then(|v| v.as_object()) {
        Some(p) => p,
        None => return String::new(),
    };

    if properties.is_empty() {
        return String::new();
    }

    let required: std::collections::HashSet<&str> = obj
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut names: Vec<&str> = properties.keys().map(|s| s.as_str()).collect();
    names.sort();

    let parts: Vec<String> = names
        .iter()
        .map(|name| {
            let typ = extract_type(&properties[*name]);
            if required.contains(name) {
                format!("{}:{}*", name, typ)
            } else {
                format!("{}:{}", name, typ)
            }
        })
        .collect();

    parts.join(", ")
}

fn extract_type(value: &serde_json::Value) -> &str {
    // Handle "type": "string"
    if let Some(t) = value.get("type").and_then(|v| v.as_str()) {
        return t;
    }

    // Handle "type": ["string", "null"]
    if let Some(arr) = value.get("type").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                if s != "null" {
                    return s;
                }
            }
        }
        if let Some(first) = arr.first().and_then(|v| v.as_str()) {
            return first;
        }
    }

    // Handle anyOf
    if let Some(any_of) = value.get("anyOf").and_then(|v| v.as_array()) {
        for item in any_of {
            if let Some(t) = item.get("type").and_then(|v| v.as_str()) {
                if t != "null" {
                    return t;
                }
            }
        }
        if let Some(first) = any_of.first() {
            if let Some(t) = first.get("type").and_then(|v| v.as_str()) {
                return t;
            }
        }
    }

    "any"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prefixed_name() {
        assert_eq!(
            prefixed_name("slack", "send_message"),
            "slack__send_message"
        );
    }

    #[test]
    fn test_parse_prefixed_name() {
        let (server, tool) = parse_prefixed_name("slack__send_message").unwrap();
        assert_eq!(server, "slack");
        assert_eq!(tool, "send_message");
    }

    #[test]
    fn test_parse_prefixed_name_first_occurrence() {
        let (server, tool) = parse_prefixed_name("a__b__c").unwrap();
        assert_eq!(server, "a");
        assert_eq!(tool, "b__c");
    }

    #[test]
    fn test_parse_prefixed_name_no_separator() {
        assert!(parse_prefixed_name("no_separator").is_err());
    }

    #[test]
    fn test_effective_type_default() {
        let cfg = ServerConfig {
            server_type: None,
            command: Some("echo".into()),
            args: None,
            env: None,
            url: None,
            headers: None,
        };
        assert_eq!(cfg.effective_type(), "stdio");
    }

    #[test]
    fn test_effective_type_http() {
        let cfg = ServerConfig {
            server_type: Some("http".into()),
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com".into()),
            headers: None,
        };
        assert_eq!(cfg.effective_type(), "http");
    }

    #[test]
    fn test_effective_type_empty_string() {
        let cfg = ServerConfig {
            server_type: Some(String::new()),
            command: Some("echo".into()),
            args: None,
            env: None,
            url: None,
            headers: None,
        };
        assert_eq!(cfg.effective_type(), "stdio");
    }

    #[test]
    fn test_compact_params_basic() {
        let schema = json!({
            "type": "object",
            "properties": {
                "channel": {"type": "string"},
                "message": {"type": "string"}
            },
            "required": ["channel"]
        });
        assert_eq!(
            compact_params_from_schema(&schema),
            "channel:string*, message:string"
        );
    }

    #[test]
    fn test_compact_params_nullable_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": ["string", "null"]}
            }
        });
        assert_eq!(compact_params_from_schema(&schema), "name:string");
    }

    #[test]
    fn test_compact_params_any_of() {
        let schema = json!({
            "type": "object",
            "properties": {
                "value": {"anyOf": [{"type": "integer"}, {"type": "null"}]}
            }
        });
        assert_eq!(compact_params_from_schema(&schema), "value:integer");
    }

    #[test]
    fn test_compact_params_empty() {
        assert_eq!(compact_params_from_schema(&json!(null)), "");
        assert_eq!(compact_params_from_schema(&json!({})), "");
        assert_eq!(compact_params_from_schema(&json!({"properties": {}})), "");
    }

    #[test]
    fn test_compact_params_no_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {"type": "string"},
                "b": {"type": "number"}
            }
        });
        assert_eq!(compact_params_from_schema(&schema), "a:string, b:number");
    }

    #[test]
    fn test_tool_entry_serde_roundtrip() {
        let entry = ToolEntry {
            name: "slack__send".into(),
            server_name: "slack".into(),
            original_name: "send".into(),
            description: "Send a message".into(),
            input_schema: json!({"type": "object"}),
            compact_params: "msg:string*".into(),
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        let parsed: ToolEntry = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.name, entry.name);
        assert_eq!(parsed.server_name, entry.server_name);
    }

    #[test]
    fn test_proxy_config_serde() {
        let json_str = r#"{
            "mcpServers": {
                "slack": {
                    "command": "slack-mcp",
                    "args": ["--token", "xxx"]
                },
                "todoist": {
                    "type": "http",
                    "url": "https://todoist.com/mcp"
                }
            }
        }"#;
        let cfg: ProxyConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 2);
        assert_eq!(cfg.mcp_servers["slack"].effective_type(), "stdio");
        assert_eq!(cfg.mcp_servers["todoist"].effective_type(), "http");
    }

    #[test]
    fn test_tool_entry_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ToolEntry>();
        assert_send_sync::<SearchResult>();
        assert_send_sync::<ProxyConfig>();
    }
}
