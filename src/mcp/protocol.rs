use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request ID - either a number or string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(u64),
    Str(String),
}

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Id,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 notification (no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Incoming message: could be request, response, or notification.
/// Uses manual deserialization (not #[serde(untagged)]) per plan review.
#[derive(Debug, Clone)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

impl JsonRpcMessage {
    /// Parse a JSON-RPC message from a serde_json::Value.
    /// Dispatches based on field presence:
    /// - id + method = Request
    /// - id + no method = Response
    /// - method + no id = Notification
    pub fn from_value(v: Value) -> Result<Self, crate::error::McpzipError> {
        let obj = v.as_object().ok_or_else(|| {
            crate::error::McpzipError::Protocol("message must be an object".into())
        })?;

        let has_id = obj.contains_key("id");
        let has_method = obj.contains_key("method");

        if has_id && has_method {
            let req: JsonRpcRequest = serde_json::from_value(Value::Object(obj.clone()))?;
            Ok(JsonRpcMessage::Request(req))
        } else if has_id {
            let resp: JsonRpcResponse = serde_json::from_value(Value::Object(obj.clone()))?;
            Ok(JsonRpcMessage::Response(resp))
        } else if has_method {
            let notif: JsonRpcNotification = serde_json::from_value(Value::Object(obj.clone()))?;
            Ok(JsonRpcMessage::Notification(notif))
        } else {
            Err(crate::error::McpzipError::Protocol(
                "message has neither 'id' nor 'method'".into(),
            ))
        }
    }
}

// --- MCP-specific types ---

/// Client capabilities sent during initialize.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {}

/// Server capabilities returned from initialize.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsCapability {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourcesCapability {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptsCapability {}

/// Params for initialize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Result of initialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// A tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        rename = "inputSchema",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub input_schema: Option<Value>,
}

/// Params for tools/call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// Result of tools/call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ContentItem>,
    #[serde(rename = "isError", default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// A content item in tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "text")]
    Text { text: String },
}

/// Result of tools/list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<ToolInfo>,
}

/// Helper to create a JSON-RPC 2.0 response.
pub fn make_response(id: Id, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(result),
        error: None,
    }
}

/// Helper to create a JSON-RPC 2.0 error response.
pub fn make_error_response(id: Id, code: i64, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: None,
        }),
    }
}

/// Helper to create a JSON-RPC 2.0 request.
pub fn make_request(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Id::Number(id),
        method: method.into(),
        params,
    }
}

/// Helper to create a JSON-RPC 2.0 notification.
pub fn make_notification(method: &str, params: Option<Value>) -> JsonRpcNotification {
    JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: method.into(),
        params,
    }
}

// Standard JSON-RPC error codes
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_roundtrip() {
        let req = make_request(1, "tools/list", None);
        let json_str = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.id, Id::Number(1));
        assert_eq!(parsed.method, "tools/list");
    }

    #[test]
    fn test_response_with_result() {
        let resp = make_response(Id::Number(1), json!({"tools": []}));
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains("tools"));
        assert!(!json_str.contains("error"));
    }

    #[test]
    fn test_response_with_error() {
        let resp = make_error_response(Id::Number(1), METHOD_NOT_FOUND, "not found".into());
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains("error"));
        assert!(json_str.contains("-32601"));
    }

    #[test]
    fn test_notification_no_id() {
        let notif = make_notification("initialized", None);
        let json_str = serde_json::to_string(&notif).unwrap();
        assert!(!json_str.contains("\"id\""));
        assert!(json_str.contains("initialized"));
    }

    #[test]
    fn test_id_number_vs_string() {
        let num = Id::Number(42);
        let s = Id::Str("abc".into());
        assert_ne!(num, s);

        let json_num = serde_json::to_string(&num).unwrap();
        assert_eq!(json_num, "42");

        let json_str = serde_json::to_string(&s).unwrap();
        assert_eq!(json_str, "\"abc\"");
    }

    #[test]
    fn test_call_tool_params() {
        let params = CallToolParams {
            name: "send_message".into(),
            arguments: Some(json!({"channel": "#general"})),
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["name"], "send_message");
    }

    #[test]
    fn test_call_tool_result_with_text() {
        let result = CallToolResult {
            content: vec![ContentItem::Text {
                text: "hello".into(),
            }],
            is_error: None,
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "hello");
    }

    #[test]
    fn test_message_dispatch_request() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });
        let msg = JsonRpcMessage::from_value(v).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Request(_)));
    }

    #[test]
    fn test_message_dispatch_response() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"tools": []}
        });
        let msg = JsonRpcMessage::from_value(v).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Response(_)));
    }

    #[test]
    fn test_message_dispatch_notification() {
        let v = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });
        let msg = JsonRpcMessage::from_value(v).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Notification(_)));
    }

    #[test]
    fn test_message_dispatch_invalid() {
        let v = json!({"jsonrpc": "2.0"});
        assert!(JsonRpcMessage::from_value(v).is_err());
    }

    #[test]
    fn test_initialize_result_serde() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".into(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {}),
                resources: None,
                prompts: None,
            },
            server_info: ServerInfo {
                name: "mcpzip".into(),
                version: "0.1.0".into(),
            },
            instructions: Some("Use search_tools to find tools.".into()),
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["protocolVersion"], "2024-11-05");
        assert!(v["capabilities"]["tools"].is_object());
        assert_eq!(v["serverInfo"]["name"], "mcpzip");
    }

    // --- New tests ---

    #[test]
    fn test_request_with_params() {
        let req = make_request(
            42,
            "tools/call",
            Some(json!({"name": "test", "arguments": {"a": 1}})),
        );
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["id"], 42);
        assert_eq!(v["method"], "tools/call");
        assert_eq!(v["params"]["name"], "test");
        assert_eq!(v["params"]["arguments"]["a"], 1);
    }

    #[test]
    fn test_response_with_error_object() {
        let resp = make_error_response(
            Id::Number(99),
            INVALID_PARAMS,
            "missing required field".into(),
        );
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["id"], 99);
        assert!(v["result"].is_null());
        assert_eq!(v["error"]["code"], INVALID_PARAMS);
        assert_eq!(v["error"]["message"], "missing required field");
    }

    #[test]
    fn test_notification_with_params() {
        let notif = make_notification("progress", Some(json!({"percent": 50})));
        let v = serde_json::to_value(&notif).unwrap();
        assert!(!v.as_object().unwrap().contains_key("id"));
        assert_eq!(v["method"], "progress");
        assert_eq!(v["params"]["percent"], 50);
    }

    #[test]
    fn test_message_dispatch_non_object_fails() {
        assert!(JsonRpcMessage::from_value(json!(42)).is_err());
        assert!(JsonRpcMessage::from_value(json!("hello")).is_err());
        assert!(JsonRpcMessage::from_value(json!([1, 2, 3])).is_err());
        assert!(JsonRpcMessage::from_value(json!(null)).is_err());
        assert!(JsonRpcMessage::from_value(json!(true)).is_err());
    }

    #[test]
    fn test_message_dispatch_extra_fields_ignored() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "test",
            "extra_field": "should be ignored",
            "another": 42
        });
        let msg = JsonRpcMessage::from_value(v).unwrap();
        match msg {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.method, "test");
                assert_eq!(req.id, Id::Number(1));
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_message_dispatch_string_id() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": "abc-123",
            "method": "test"
        });
        let msg = JsonRpcMessage::from_value(v).unwrap();
        match msg {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.id, Id::Str("abc-123".into()));
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn test_content_item_serde() {
        let item = ContentItem::Text {
            text: "hello world".into(),
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["type"], "text");
        assert_eq!(v["text"], "hello world");

        // Deserialize back
        let parsed: ContentItem = serde_json::from_value(v).unwrap();
        match parsed {
            ContentItem::Text { text } => assert_eq!(text, "hello world"),
        }
    }

    #[test]
    fn test_make_response_fields() {
        let resp = make_response(Id::Str("req-1".into()), json!({"status": "ok"}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Id::Str("req-1".into()));
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["status"], "ok");
    }

    #[test]
    fn test_make_error_response_fields() {
        let resp = make_error_response(Id::Number(5), INTERNAL_ERROR, "boom".into());
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Id::Number(5));
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(err.message, "boom");
        assert!(err.data.is_none());
    }

    #[test]
    fn test_make_notification_fields() {
        let notif = make_notification("test/method", None);
        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "test/method");
        assert!(notif.params.is_none());
    }

    #[test]
    fn test_make_request_fields() {
        let req = make_request(100, "my/method", Some(json!({"key": "value"})));
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Id::Number(100));
        assert_eq!(req.method, "my/method");
        assert_eq!(req.params.unwrap()["key"], "value");
    }

    #[test]
    fn test_tool_info_optional_fields() {
        // Minimal ToolInfo with no description or schema
        let ti = ToolInfo {
            name: "test".into(),
            description: None,
            input_schema: None,
        };
        let v = serde_json::to_value(&ti).unwrap();
        assert_eq!(v["name"], "test");
        assert!(!v.as_object().unwrap().contains_key("description"));
        assert!(!v.as_object().unwrap().contains_key("inputSchema"));

        // Full ToolInfo
        let ti = ToolInfo {
            name: "test".into(),
            description: Some("A test tool".into()),
            input_schema: Some(json!({"type": "object"})),
        };
        let v = serde_json::to_value(&ti).unwrap();
        assert_eq!(v["description"], "A test tool");
        assert_eq!(v["inputSchema"]["type"], "object");
    }

    #[test]
    fn test_call_tool_params_serde() {
        let params = CallToolParams {
            name: "my_tool".into(),
            arguments: Some(json!({"key": "value"})),
        };
        let json_str = serde_json::to_string(&params).unwrap();
        let parsed: CallToolParams = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.name, "my_tool");
        assert_eq!(parsed.arguments.unwrap()["key"], "value");
    }

    #[test]
    fn test_call_tool_params_no_args() {
        let params = CallToolParams {
            name: "simple_tool".into(),
            arguments: None,
        };
        let v = serde_json::to_value(&params).unwrap();
        assert!(!v.as_object().unwrap().contains_key("arguments"));
    }

    #[test]
    fn test_call_tool_result_serde() {
        let result = CallToolResult {
            content: vec![
                ContentItem::Text {
                    text: "line 1".into(),
                },
                ContentItem::Text {
                    text: "line 2".into(),
                },
            ],
            is_error: Some(true),
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["content"].as_array().unwrap().len(), 2);
        assert_eq!(v["isError"], true);

        let parsed: CallToolResult = serde_json::from_value(v).unwrap();
        assert_eq!(parsed.content.len(), 2);
        assert_eq!(parsed.is_error, Some(true));
    }

    #[test]
    fn test_error_code_constants() {
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }
}
