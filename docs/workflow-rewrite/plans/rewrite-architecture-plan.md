# Architecture Plan: Rust Rewrite of mcpzip

## Component Diagram

```
                          stdin (NDJSON)
                               |
                               v
                    ┌──────────────────────┐
                    │    mcp::Server        │
                    │  (JSON-RPC 2.0 /     │
                    │   stdio NDJSON)       │
                    └──────────┬───────────┘
                               │
                               v
                    ┌──────────────────────┐
                    │    proxy::Server      │
                    │  search_tools         │
                    │  describe_tool        │
                    │  execute_tool         │
                    └───┬──────┬──────┬────┘
                        │      │      │
              ┌─────────┘      │      └──────────┐
              v                v                  v
     ┌──────────────┐ ┌──────────────┐  ┌──────────────────┐
     │   search     │ │   catalog    │  │ transport::Manager│
     │  (Keyword +  │ │ (disk cache  │  │  (pool + reaper + │
     │   LLM stub)  │ │  + refresh)  │  │   retry + timeout)│
     └──────────────┘ └──────┬───────┘  └────────┬─────────┘
                             │                   │
                             │ ToolLister trait   │ ConnectFn
                             └──────►────────────┘
                                                 │
                     ┌──────────────┬────────────┼────────────┐
                     v              v             v            v
              ┌───────────┐ ┌───────────┐ ┌──────────┐ ┌──────────┐
              │  Stdio    │ │ Streamable│ │   SSE    │ │  Memory  │
              │ Upstream  │ │   HTTP    │ │ Upstream │ │ (tests)  │
              └─────┬─────┘ └─────┬─────┘ └────┬─────┘ └──────────┘
                    │             │             │
                    v             v             v
              ┌───────────┐ ┌──────────────────────┐
              │ subprocess│ │   reqwest + OAuth     │
              │ (tokio::  │ │   (token store,       │
              │  process) │ │    PKCE, DCR)         │
              └───────────┘ └──────────────────────┘

     ┌─────────────────────────────────────────────────────┐
     │                  Shared Leaf Layers                  │
     │                                                     │
     │  types   config   auth   error   mcp::protocol      │
     └─────────────────────────────────────────────────────┘
```

---

## 1. Module / Crate Structure

Single crate (`mcpzip`), flat module tree under `src/`. No workspace; binary is small enough that a single crate keeps compile times fast and avoids inter-crate dependency wiring.

```
Cargo.toml
src/
  main.rs                  # entry point, clap dispatch
  cli/
    mod.rs                 # re-exports
    serve.rs               # serve subcommand
    init.rs                # interactive wizard
    migrate.rs             # Claude Code migration
    status.rs              # status subcommand
  config.rs                # load, validate, paths
  types.rs                 # ToolEntry, SearchResult, ServerConfig, ProxyConfig
  error.rs                 # McpzipError enum hierarchy
  mcp/
    mod.rs                 # re-exports
    protocol.rs            # JSON-RPC 2.0 message types (Request, Response, Notification)
    server.rs              # downstream MCP server (stdio NDJSON read loop)
    client.rs              # upstream MCP client (handshake, tools/list, tools/call)
    transport.rs           # Transport trait + StdioTransport, MemoryTransport
  transport/
    mod.rs                 # re-exports, Upstream trait, ConnectFn type alias
    manager.rs             # connection pool, idle reaper, retry, CallTool
    stdio.rs               # StdioUpstream (subprocess)
    http.rs                # StreamableHttpUpstream
    sse.rs                 # SseUpstream (legacy)
  auth/
    mod.rs                 # re-exports
    store.rs               # TokenStore (disk persistence)
    oauth.rs               # OAuthHandler, PKCE flow, browser callback, DCR
  catalog/
    mod.rs                 # re-exports
    catalog.rs             # Catalog struct (in-memory + refresh)
    cache.rs               # disk read/write for tools.json
  search/
    mod.rs                 # re-exports, Searcher trait, NewSearcher factory
    keyword.rs             # KeywordSearcher
    llm.rs                 # GeminiSearcher stub
    query_cache.rs         # QueryCache (normalized key + token overlap)
    orchestrated.rs        # OrchestratedSearcher (keyword + LLM + cache)
  proxy/
    mod.rs                 # re-exports
    server.rs              # proxy::Server, tool registration, Run
    handlers.rs            # HandleSearchTools, HandleDescribeTool, HandleExecuteTool
    resources.rs           # Resource/Prompt forwarding stubs
    instructions.rs        # Server instructions generation
```

### Go-to-Rust Module Mapping

| Go Package | Rust Module | Notes |
|---|---|---|
| `internal/types` | `src/types.rs` | Pure leaf, no deps |
| `internal/config` | `src/config.rs` | Depends on `types` |
| `internal/auth` | `src/auth/` | `store.rs` + `oauth.rs` (always compiled, no feature gate) |
| `internal/transport` | `src/transport/` | `Upstream` trait + `Manager` + concrete impls |
| `internal/catalog` | `src/catalog/` | `Catalog` + disk cache |
| `internal/search` | `src/search/` | `Searcher` trait + keyword + LLM stub + cache |
| `internal/proxy` | `src/proxy/` | Meta-tool handlers, MCP server wiring |
| `internal/cli` | `src/cli/` | clap subcommands |
| N/A (go-sdk) | `src/mcp/` | Custom JSON-RPC 2.0 / NDJSON layer |

---

## 2. Core Traits

All async traits use the `async-trait` crate. Trait objects are `Arc<dyn Trait>` where shared, `Box<dyn Trait>` where owned.

### Upstream

```rust
// src/transport/mod.rs

use async_trait::async_trait;
use serde_json::value::RawValue;

use crate::error::McpzipError;
use crate::types::ToolEntry;

#[async_trait]
pub trait Upstream: Send + Sync {
    /// List all tools from this upstream server.
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError>;

    /// Invoke a tool and return the raw JSON result.
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Box<RawValue>,
    ) -> Result<Box<RawValue>, McpzipError>;

    /// Shut down the connection.
    async fn close(&self) -> Result<(), McpzipError>;

    /// Check if the connection is still usable.
    fn alive(&self) -> bool;
}
```

### Searcher

```rust
// src/search/mod.rs

use async_trait::async_trait;

use crate::error::McpzipError;
use crate::types::SearchResult;

#[async_trait]
pub trait Searcher: Send + Sync {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, McpzipError>;
}
```

### ToolLister

```rust
// src/catalog/mod.rs

use async_trait::async_trait;
use std::collections::HashMap;

use crate::error::McpzipError;
use crate::types::ToolEntry;

#[async_trait]
pub trait ToolLister: Send + Sync {
    /// Fetch tools from all configured upstream servers.
    /// Partial failures are tolerated: failed servers are omitted from the map.
    async fn list_tools_all(&self) -> Result<HashMap<String, Vec<ToolEntry>>, McpzipError>;
}
```

### ConnectFn

```rust
// src/transport/mod.rs

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::McpzipError;
use crate::types::ServerConfig;

/// Factory function for creating upstream connections.
/// Injected into Manager for testing.
pub type ConnectFn = Arc<
    dyn Fn(
            String,          // server name
            ServerConfig,    // server config (cloned)
        ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Upstream>, McpzipError>> + Send>>
        + Send
        + Sync,
>;
```

### MCP Transport (for the custom MCP layer)

```rust
// src/mcp/transport.rs

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

/// A bidirectional NDJSON transport for the MCP protocol layer.
/// Implemented by StdioTransport (stdin/stdout) and MemoryTransport (tests).
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC message (serialized, newline-terminated).
    async fn send(&self, message: &[u8]) -> Result<(), std::io::Error>;

    /// Receive the next JSON-RPC message (one line).
    /// Returns None on EOF.
    async fn recv(&self) -> Result<Option<String>, std::io::Error>;

    /// Shut down the transport.
    async fn close(&self) -> Result<(), std::io::Error>;
}
```

---

## 3. MCP Protocol Layer

Custom implementation. The Go SDK provides `mcp.Server`, `mcp.Client`, `mcp.StdioTransport`, `mcp.CommandTransport`, etc. We replace all of these with approximately 400-600 lines of Rust.

### Wire Format

- **NDJSON** (newline-delimited JSON) over stdio
- Each line is a complete JSON-RPC 2.0 message
- Messages: Request (has `id` + `method`), Response (has `id` + `result`/`error`), Notification (has `method`, no `id`)

### Protocol Message Types

```rust
// src/mcp/protocol.rs

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// A JSON-RPC 2.0 request ID. Can be a number or string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

/// Outgoing or incoming JSON-RPC 2.0 request.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String, // always "2.0"
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Box<RawValue>>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Box<RawValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Box<RawValue>>,
}

/// JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Box<RawValue>>,
}

/// Union type for any incoming message (parsed via id/method presence).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

// --- MCP-specific param/result types ---

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: Implementation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    // Minimal: we do not advertise any client capabilities
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: Implementation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCapability {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Box<RawValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Box<RawValue>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
}
```

### MCP Server (Downstream)

```rust
// src/mcp/server.rs

/// Runs the MCP server loop over a McpTransport.
///
/// 1. Reads lines from transport
/// 2. Parses JSON-RPC messages
/// 3. Dispatches:
///    - "initialize" -> respond with capabilities + instructions
///    - "initialized" -> notification, no response
///    - "tools/list" -> return 3 meta-tool definitions
///    - "tools/call" -> delegate to registered handler
///    - "resources/list", "resources/read", "prompts/list", "prompts/get" -> forward
/// 4. Writes JSON-RPC response line
///
/// Runs until EOF or cancellation token fires.

pub struct McpServer {
    info: Implementation,
    instructions: String,
    tools: Vec<ToolDefinition>,
    /// Handler invoked for tools/call requests.
    /// Receives (tool_name, arguments_raw) and returns CallToolResult.
    call_handler: Box<dyn Fn(&str, Option<Box<RawValue>>) -> Pin<Box<dyn Future<Output = CallToolResult> + Send>> + Send + Sync>,
}

impl McpServer {
    pub async fn run(&self, transport: Box<dyn McpTransport>, cancel: CancellationToken) -> Result<(), McpzipError>;
}
```

### MCP Client (Upstream)

```rust
// src/mcp/client.rs

/// Connects to an upstream MCP server, performs the initialize handshake,
/// and provides tools/list and tools/call RPCs.
///
/// The client owns a McpTransport and manages request ID generation
/// and response correlation (via a pending requests map).

pub struct McpClient {
    transport: Box<dyn McpTransport>,
    next_id: AtomicI64,
    pending: Arc<std::sync::Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
}

impl McpClient {
    /// Connect and perform initialize/initialized handshake.
    pub async fn connect(transport: Box<dyn McpTransport>) -> Result<Self, McpzipError>;

    /// Send tools/list and collect all tools.
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McpzipError>;

    /// Send tools/call and return the result.
    pub async fn call_tool(&self, name: &str, args: Option<Box<RawValue>>) -> Result<CallToolResult, McpzipError>;

    /// Shut down.
    pub async fn close(&self) -> Result<(), McpzipError>;
}
```

The client spawns an internal reader task (`tokio::spawn`) that reads lines from the transport and routes responses to the matching `oneshot::Sender` in the `pending` map. This is the standard pattern for multiplexing requests over a single bidirectional stream.

---

## 4. Data Types

```rust
// src/types.rs

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::HashMap;

pub const NAME_SEPARATOR: &str = "__";

/// A cached tool from an upstream MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,           // prefixed: "servername__toolname"
    pub server_name: String,
    pub original_name: String,
    pub description: String,
    /// Full JSON Schema, stored opaquely. Never parsed by the proxy
    /// except for compact_params generation.
    pub input_schema: Box<RawValue>,
    pub compact_params: String, // "param1:type*, param2:type"
}

/// Returned by the search engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub compact_params: String,
}

/// Defines how to connect to an upstream MCP server.
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
}

impl ServerConfig {
    pub fn effective_type(&self) -> &str {
        self.server_type.as_deref().unwrap_or("stdio")
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

/// Full proxy configuration (deserialized from config.json).
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

// --- Helper functions (match Go behavior exactly) ---

pub fn prefixed_name(server: &str, tool: &str) -> String {
    format!("{}{}{}", server, NAME_SEPARATOR, tool)
}

pub fn parse_prefixed_name(name: &str) -> Result<(&str, &str), McpzipError> {
    match name.find(NAME_SEPARATOR) {
        Some(idx) => Ok((&name[..idx], &name[idx + NAME_SEPARATOR.len()..])),
        None => Err(McpzipError::InvalidName(name.to_string())),
    }
}

/// Generate compact parameter summary from a JSON Schema.
/// Format: "param1:type*, param2:type" where * marks required params.
pub fn compact_params_from_schema(schema: &RawValue) -> String {
    // Parse minimally: extract "properties" and "required" fields.
    // Sort param names alphabetically for determinism.
    // For each property, extract type via extract_type().
    // Append * if in required set.
    // ... (direct port of Go logic)
    todo!()
}
```

---

## 5. Concurrency Architecture

### Tokio Task Mapping

| Go Goroutine | Rust Equivalent | Cancellation |
|---|---|---|
| `Manager.reaper()` | `tokio::spawn` + `tokio::time::interval` | `CancellationToken` |
| Background catalog refresh | `tokio::spawn` (one-shot) | Parent `CancellationToken` |
| Signal handler | `tokio::signal::ctrl_c()` / `tokio::signal::unix::signal(SIGTERM)` | Triggers `CancellationToken::cancel()` |
| Per-server ListToolsAll workers | `tokio::task::JoinSet` | Per-task `tokio::time::timeout(30s)` |
| MCP server read loop | `tokio::main` blocking (or spawned task) | `CancellationToken` |
| MCP client reader task | `tokio::spawn` per upstream McpClient | Drop transport closes reader |
| OAuth callback HTTP server | `tokio::spawn` + `axum::serve` | Shutdown signal after code received |

### Shared State Design

The spec mandates `std::sync::RwLock` (not `tokio::sync`). This means all lock acquisitions must be scoped to NOT hold across `.await` points. The pattern is: acquire lock, clone/copy data out, drop lock, then await.

| State | Type | Protection | Access Pattern |
|---|---|---|---|
| Catalog inner | `CatalogInner { tools, by_name, by_server }` | `Arc<std::sync::RwLock<CatalogInner>>` | Read-heavy: `AllTools()`, `GetTool()`. Write-rare: `RefreshAll()`. |
| Connection pool | `HashMap<String, PoolEntry>` | `Arc<std::sync::RwLock<HashMap<String, PoolEntry>>>` | Read+write on every `GetUpstream`. Reaper writes periodically. |
| Query cache | `HashMap<String, Vec<SearchResult>>` | `Arc<std::sync::RwLock<HashMap<String, Vec<SearchResult>>>>` | Read on every search, write on LLM results. |
| Per-upstream alive flag | `AtomicBool` | No lock needed | Checked in `Alive()`, set in `Close()`. |
| MCP client pending map | `HashMap<RequestId, oneshot::Sender>` | `std::sync::Mutex` | Insert on send, remove on receive. Never held across await. |

### Lock Discipline

```rust
// CORRECT: clone data out, drop lock, then await
fn get_upstream(&self, name: &str) -> Result<Arc<dyn Upstream>, McpzipError> {
    let pool = self.pool.read().unwrap();
    if let Some(entry) = pool.get(name) {
        if entry.upstream.alive() {
            return Ok(Arc::clone(&entry.upstream));
        }
    }
    drop(pool);
    // Now safe to .await for connect
    ...
}

// WRONG: holding std::sync lock across await
// let pool = self.pool.write().unwrap();
// let upstream = self.connect(name).await; // DEADLOCK RISK
```

---

## 6. Transport Implementations

### Stdio (Subprocess)

```rust
// src/transport/stdio.rs

use tokio::process::{Child, Command};
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct StdioUpstream {
    client: McpClient,  // owns the subprocess transport
    alive: AtomicBool,
}

impl StdioUpstream {
    pub async fn new(name: &str, cfg: &ServerConfig) -> Result<Self, McpzipError> {
        let mut cmd = Command::new(cfg.command.as_deref().unwrap());
        if let Some(args) = &cfg.args {
            cmd.args(args);
        }
        // Inherit parent env, then overlay per-server env
        if let Some(env_map) = &cfg.env {
            for (k, v) in env_map {
                cmd.env(k, v);
            }
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit()); // diagnostic to parent stderr

        let child = cmd.spawn()?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let transport = StdioMcpTransport::new(
            BufReader::new(stdout),
            stdin,
            child,
        );
        let client = McpClient::connect(Box::new(transport)).await?;

        Ok(Self { client, alive: AtomicBool::new(true) })
    }
}
```

The `StdioMcpTransport` wraps the child process's stdin/stdout as a `McpTransport` implementation. It uses `BufReader::read_line` for receiving and `write_all` + newline for sending.

### Streamable HTTP

```rust
// src/transport/http.rs

/// MCP 2025-03-26 Streamable HTTP transport.
///
/// - POST requests to server endpoint for each RPC
/// - Response is SSE stream (event: message, data: JSON-RPC response)
/// - Optionally receives session ID from Mcp-Session header
/// - Sends session ID back on subsequent requests
///
/// Does NOT multiplex multiple requests over one SSE stream; each
/// tools/call gets its own POST->SSE roundtrip. This simplifies the
/// implementation vs. the Go SDK's 2,200-line version.

pub struct StreamableHttpUpstream {
    client: reqwest::Client, // with OAuth middleware if needed
    endpoint: String,
    session_id: std::sync::Mutex<Option<String>>,
    alive: AtomicBool,
}
```

The streamable HTTP transport does NOT use `McpClient` internally. Instead it directly sends JSON-RPC requests via `reqwest::Client::post()` and parses the SSE response body. This avoids the complexity of a persistent bidirectional channel. The `initialize`/`initialized` handshake is done in the constructor.

### SSE (Legacy)

```rust
// src/transport/sse.rs

/// Legacy SSE transport.
///
/// - Persistent GET to endpoint for server-sent events
/// - POST to a separate endpoint for client->server messages
/// - The GET response includes an "endpoint" event with the POST URL
///
/// Uses reqwest for both the SSE stream and POSTs.

pub struct SseUpstream {
    // SSE reader task handle
    // POST endpoint URL (discovered from SSE stream)
    // Pending response map (like McpClient)
    alive: AtomicBool,
}
```

### Memory Transport (Tests)

```rust
// src/mcp/transport.rs

/// In-memory bidirectional transport for testing.
/// Created in pairs via `memory_transport_pair()`.
/// Backed by `tokio::sync::mpsc` channels.
pub struct MemoryTransport {
    tx: tokio::sync::mpsc::Sender<String>,
    rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<String>>,
}

pub fn memory_transport_pair() -> (MemoryTransport, MemoryTransport) {
    let (tx_a, rx_a) = tokio::sync::mpsc::channel(32);
    let (tx_b, rx_b) = tokio::sync::mpsc::channel(32);
    (
        MemoryTransport { tx: tx_a, rx: tokio::sync::Mutex::new(rx_b) },
        MemoryTransport { tx: tx_b, rx: tokio::sync::Mutex::new(rx_a) },
    )
}
```

---

## 7. OAuth Flow

OAuth is always enabled (no feature gate, unlike Go's build tags).

### Components

```rust
// src/auth/store.rs

use sha2::{Sha256, Digest};

pub struct TokenStore {
    base_dir: PathBuf,
}

/// Persisted token format. Field names match Go's oauth2.Token JSON encoding
/// for backward compatibility with existing cached tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>, // RFC 3339
}

impl TokenStore {
    pub fn new(base_dir: PathBuf) -> Self;

    pub fn load(&self, server_url: &str) -> Result<Option<StoredToken>, McpzipError>;

    pub fn save(&self, server_url: &str, token: &StoredToken) -> Result<(), McpzipError>;

    fn path(&self, server_url: &str) -> PathBuf {
        let hash = Sha256::digest(server_url.as_bytes());
        let name = hex::encode(&hash[..16]); // 32 hex chars
        self.base_dir.join(format!("{}.json", name))
    }
}
```

```rust
// src/auth/oauth.rs

/// Performs the full OAuth 2.1 flow:
/// 1. Discover server metadata (RFC 8414)
/// 2. Dynamic client registration (RFC 7591)
/// 3. Generate PKCE code_verifier + code_challenge
/// 4. Start local HTTP server on ephemeral port
/// 5. Open browser to authorization URL
/// 6. Receive callback with authorization code
/// 7. Exchange code for tokens
/// 8. Persist tokens via TokenStore
///
/// Uses the `oauth2` crate for PKCE and token exchange.
/// Uses `axum` for the local callback server.
/// Uses `open` crate for cross-platform browser launch.

pub struct OAuthHandler {
    store: TokenStore,
    server_url: String,
}

impl OAuthHandler {
    /// Get a valid access token, performing the OAuth flow if needed.
    /// Checks disk cache first, then initiates browser flow.
    pub async fn get_token(&self) -> Result<String, McpzipError>;

    /// Create a reqwest client with OAuth middleware (Authorization header injection).
    pub fn build_client(&self) -> reqwest::Client;
}
```

### Data Flow

1. `StreamableHttpUpstream::new()` creates `OAuthHandler` for the server URL
2. Handler checks `TokenStore::load()` for cached token
3. If token exists and not expired, inject `Authorization: Bearer <token>` into reqwest
4. If token expired but refresh_token exists, attempt refresh via `oauth2` crate
5. If no token or refresh fails, initiate browser flow:
   - `GET /.well-known/oauth-authorization-server` for server metadata
   - `POST` to registration endpoint with client metadata (DCR)
   - Generate PKCE challenge
   - Start `axum` server on `localhost:0`
   - Open browser to auth URL
   - Wait for callback with code
   - Exchange code for tokens
   - Save via `TokenStore::save()`

---

## 8. Connection Pool

```rust
// src/transport/manager.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

struct PoolEntry {
    upstream: Arc<dyn Upstream>,
    last_used: Instant,
}

pub struct Manager {
    configs: HashMap<String, ServerConfig>,
    pool: Arc<RwLock<HashMap<String, PoolEntry>>>,
    idle_timeout: Duration,
    call_timeout: Duration,
    connect: ConnectFn,
    cancel: CancellationToken,
    /// Mutex protecting per-server connect-in-progress to avoid thundering herd.
    connecting: std::sync::Mutex<HashMap<String, ()>>,
}

impl Manager {
    pub fn new(
        configs: HashMap<String, ServerConfig>,
        idle_timeout: Duration,
        call_timeout: Duration,
        connect: ConnectFn,
    ) -> Arc<Self> {
        let mgr = Arc::new(Self { ... });
        // Spawn idle reaper task
        let mgr_clone = Arc::clone(&mgr);
        tokio::spawn(async move { mgr_clone.reaper().await });
        mgr
    }

    /// Get a pooled upstream, creating one if needed.
    /// If existing connection is dead, evicts and reconnects.
    pub async fn get_upstream(&self, server_name: &str) -> Result<Arc<dyn Upstream>, McpzipError> {
        // 1. Read lock: check pool for alive connection
        // 2. If found and alive: update last_used, return
        // 3. Drop read lock
        // 4. Write lock: evict dead entry if present
        // 5. Drop write lock
        // 6. Await connect (no lock held)
        // 7. Write lock: insert new entry
    }

    /// Call a tool with retry-on-stale semantics.
    /// If the first call fails, evict the connection and retry once
    /// with a fresh connection.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Box<RawValue>,
    ) -> Result<Box<RawValue>, McpzipError> {
        let upstream = self.get_upstream(server_name).await?;

        let call_timeout = self.call_timeout;
        let result = if call_timeout > Duration::ZERO {
            tokio::time::timeout(call_timeout, upstream.call_tool(tool_name, args.clone())).await
        } else {
            Ok(upstream.call_tool(tool_name, args.clone()).await)
        };

        match result {
            Ok(Ok(data)) => return Ok(data),
            Ok(Err(_)) | Err(_) => {
                // Evict stale connection
                self.evict(server_name).await;
                // Retry once with fresh connection
                let upstream = self.get_upstream(server_name).await?;
                let result = if call_timeout > Duration::ZERO {
                    tokio::time::timeout(call_timeout, upstream.call_tool(tool_name, args)).await
                        .map_err(|_| McpzipError::Timeout)?
                } else {
                    upstream.call_tool(tool_name, args).await
                };
                result
            }
        }
    }

    async fn reaper(&self) {
        let interval = std::cmp::min(
            Duration::from_secs(60),
            self.idle_timeout / 2,
        );
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => self.reap_idle().await,
                _ = self.cancel.cancelled() => return,
            }
        }
    }

    async fn reap_idle(&self) {
        let now = Instant::now();
        let mut pool = self.pool.write().unwrap();
        let expired: Vec<String> = pool.iter()
            .filter(|(_, entry)| now.duration_since(entry.last_used) > self.idle_timeout)
            .map(|(name, _)| name.clone())
            .collect();
        for name in expired {
            if let Some(entry) = pool.remove(&name) {
                // Close outside lock? No -- close is sync-safe (sets AtomicBool).
                // The actual async cleanup can be spawned.
                let upstream = entry.upstream;
                tokio::spawn(async move { let _ = upstream.close().await; });
            }
        }
    }
}

#[async_trait]
impl ToolLister for Manager {
    async fn list_tools_all(&self) -> Result<HashMap<String, Vec<ToolEntry>>, McpzipError> {
        let mut join_set = tokio::task::JoinSet::new();
        for (name, _cfg) in &self.configs {
            let name = name.clone();
            let self_ref = ... ; // need Arc<Self>
            join_set.spawn(async move {
                let result = tokio::time::timeout(
                    Duration::from_secs(30),
                    self_ref.list_tools_for_server(&name),
                ).await;
                (name, result)
            });
        }

        let mut all_tools = HashMap::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((name, Ok(Ok(tools)))) => { all_tools.insert(name, tools); }
                _ => { /* log warning, skip failed server */ }
            }
        }
        Ok(all_tools)
    }
}
```

### Pool Characteristics

- **Lazy connect**: No connections at startup. First `get_upstream` triggers connect.
- **Idle reaper**: Runs every `min(60s, idle_timeout/2)`. Closes connections idle past threshold.
- **One-shot retry**: On call failure, evict + reconnect + retry exactly once.
- **Default call timeout**: 2 minutes (120 seconds), configurable via `call_timeout_seconds` in config.
- **Default idle timeout**: 5 minutes (300 seconds), configurable via `idle_timeout_minutes` in config.

---

## 9. Catalog

```rust
// src/catalog/catalog.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

struct CatalogInner {
    tools: Vec<ToolEntry>,
    by_name: HashMap<String, usize>,    // name -> index into tools
    by_server: HashMap<String, Vec<usize>>, // server_name -> indices
}

pub struct Catalog {
    inner: Arc<RwLock<CatalogInner>>,
    lister: Arc<dyn ToolLister>,
    cache_path: PathBuf,
}

impl Catalog {
    pub fn new(lister: Arc<dyn ToolLister>, cache_path: PathBuf) -> Self;

    /// Load from disk cache. Missing/corrupt cache is not an error.
    pub fn load(&self) -> Result<(), McpzipError>;

    /// Fetch from all upstreams, update in-memory state, write cache to disk.
    /// Partial success: servers that fail are omitted.
    pub async fn refresh_all(&self) -> Result<(), McpzipError>;

    /// Return a snapshot of all tools (cloned Vec).
    pub fn all_tools(&self) -> Vec<ToolEntry>;

    /// Lookup by prefixed name.
    pub fn get_tool(&self, prefixed_name: &str) -> Result<ToolEntry, McpzipError>;

    /// Tools for a specific server.
    pub fn server_tools(&self, server_name: &str) -> Vec<ToolEntry>;

    /// Total tool count.
    pub fn tool_count(&self) -> usize;

    /// Sorted list of server names.
    pub fn server_names(&self) -> Vec<String>;
}
```

### Disk Cache

```rust
// src/catalog/cache.rs

/// Read tool entries from JSON cache file.
pub fn read_cache(path: &Path) -> Result<Vec<ToolEntry>, McpzipError>;

/// Write tool entries as pretty-printed JSON to cache file.
/// Creates parent directories as needed. File permissions: 0600.
pub fn write_cache(path: &Path, entries: &[ToolEntry]) -> Result<(), McpzipError>;
```

### Refresh Semantics

1. `Catalog::load()` is called synchronously at startup (blocking read from disk)
2. `Catalog::refresh_all()` is spawned as a background `tokio::spawn` task
3. Refresh calls `ToolLister::list_tools_all()` which fans out to all servers concurrently
4. Each server gets 30-second timeout. Failed servers are skipped (partial success).
5. Results are merged, sorted by prefixed name, and stored under write lock
6. Cache file is written to disk after lock is released

---

## 10. Search

### Searcher Trait + Factory

```rust
// src/search/mod.rs

pub fn new_searcher(
    api_key: Option<&str>,
    model: &str,
    catalog_fn: Arc<dyn Fn() -> Vec<ToolEntry> + Send + Sync>,
) -> Arc<dyn Searcher> {
    let kw = Arc::new(KeywordSearcher::new(catalog_fn.clone()));
    match api_key {
        None | Some("") => kw,
        Some(key) => Arc::new(OrchestratedSearcher::new(
            kw,
            Arc::new(GeminiSearcher::new(key.to_string(), model.to_string())),
            QueryCache::new(),
        )),
    }
}
```

### Keyword Scorer

```rust
// src/search/keyword.rs

pub struct KeywordSearcher {
    catalog_fn: Arc<dyn Fn() -> Vec<ToolEntry> + Send + Sync>,
}

impl KeywordSearcher {
    fn tokenize(s: &str) -> Vec<String>;   // lowercase, split on whitespace + underscores, dedup
    fn score_entry(entry: &ToolEntry, tokens: &[String]) -> usize; // count matching tokens
}
```

Direct port of Go logic. Tokenize splits on whitespace and underscores, lowercased, deduplicated. Score counts how many query tokens appear as substrings in the tool's `name + " " + description`.

### Query Cache

```rust
// src/search/query_cache.rs

pub struct QueryCache {
    store: std::sync::RwLock<HashMap<String, Vec<SearchResult>>>,
}

impl QueryCache {
    pub fn get(&self, query: &str) -> Option<Vec<SearchResult>>;  // exact + 60% token overlap
    pub fn put(&self, query: &str, results: Vec<SearchResult>);
}
```

### Orchestrated Search

```rust
// src/search/orchestrated.rs

pub struct OrchestratedSearcher {
    keyword: Arc<dyn Searcher>,
    llm: Arc<dyn Searcher>,
    cache: QueryCache,
}
```

Search order: cache hit -> LLM rerank (falls back to keyword on failure) -> keyword.

---

## 11. CLI

```rust
// src/main.rs + src/cli/

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mcpzip", version, about = "MCP proxy with search-based tool discovery")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the MCP proxy server
    Serve {
        /// Path to config file
        #[arg(long, default_value_t = default_config_path())]
        config: String,
    },
    /// Interactive setup wizard
    Init,
    /// Migrate from Claude Code config
    Migrate {
        /// Output config file path
        #[arg(long, default_value_t = default_config_path())]
        config: String,
        /// Path to Claude Code config (auto-detected if empty)
        #[arg(long)]
        claude_config: Option<String>,
        /// Show what would happen without writing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Show proxy status and server info
    Status {
        /// Path to config file
        #[arg(long, default_value_t = default_config_path())]
        config: String,
    },
    /// Print version
    Version,
}
```

### Serve Command Startup Sequence

```rust
// src/cli/serve.rs

pub async fn run_serve(config_path: &str) -> Result<(), McpzipError> {
    // 1. Load config
    let cfg = config::load(config_path)?;
    eprintln!("mcpzip: starting proxy ({} servers)", cfg.mcp_servers.len());

    // 2. Resolve Gemini API key (env > config)
    let api_key = std::env::var("GEMINI_API_KEY").ok()
        .or(cfg.gemini_api_key.clone());

    // 3. Create transport manager
    let store = TokenStore::new(config::auth_dir());
    let connect_fn = build_connect_fn(store);
    let idle_timeout = Duration::from_secs(cfg.idle_timeout_minutes.unwrap_or(5) * 60);
    let call_timeout = Duration::from_secs(cfg.call_timeout_seconds.unwrap_or(120));
    let manager = Manager::new(cfg.mcp_servers.clone(), idle_timeout, call_timeout, connect_fn);

    // 4. Load catalog from disk cache
    let catalog = Catalog::new(Arc::clone(&manager) as Arc<dyn ToolLister>, config::cache_path());
    let _ = catalog.load(); // non-fatal
    eprintln!("mcpzip: loaded {} tools from cache", catalog.tool_count());

    // 5. Create searcher
    let model = cfg.search.model.as_deref().unwrap_or("gemini-2.0-flash");
    let catalog_ref = catalog.clone(); // Arc internally
    let catalog_fn = Arc::new(move || catalog_ref.all_tools());
    let searcher = search::new_searcher(api_key.as_deref(), model, catalog_fn);

    // 6. Create proxy server
    let srv = proxy::Server::new(catalog.clone(), searcher, Arc::clone(&manager));

    // 7. Background catalog refresh
    let catalog_bg = catalog.clone();
    let cancel = CancellationToken::new();
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            if let Err(e) = catalog_bg.refresh_all().await {
                eprintln!("mcpzip: background refresh error: {e}");
            } else {
                eprintln!("mcpzip: catalog refreshed ({} tools)", catalog_bg.tool_count());
            }
        }
    });

    // 8. Signal handler
    let cancel_sig = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        eprintln!("\nmcpzip: shutting down");
        cancel_sig.cancel();
    });

    // 9. Run MCP server over stdio
    eprintln!("mcpzip: serving MCP over stdio");
    let transport = StdioServerTransport::new(); // stdin reader + stdout writer
    srv.run(Box::new(transport), cancel).await?;

    // 10. Cleanup
    manager.close().await
}
```

---

## 12. Error Types

```rust
// src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpzipError {
    // --- Configuration ---
    #[error("config error: {0}")]
    Config(String),

    #[error("invalid config: {0}")]
    ConfigValidation(String),

    // --- Transport / Connection ---
    #[error("connection error for {server}: {source}")]
    Connection {
        server: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("unknown server: {0}")]
    UnknownServer(String),

    #[error("connection closed")]
    ConnectionClosed,

    #[error("unknown transport type: {0}")]
    UnknownTransportType(String),

    // --- Protocol ---
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc { code: i64, message: String },

    // --- Tool Operations ---
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    #[error("invalid prefixed name {0}: missing separator \"__\"")]
    InvalidName(String),

    #[error("tool call timeout after {0:?}")]
    Timeout(std::time::Duration),

    // --- Search ---
    #[error("search error: {0}")]
    Search(String),

    // --- Auth ---
    #[error("OAuth error: {0}")]
    OAuth(String),

    #[error("token store error: {0}")]
    TokenStore(String),

    // --- IO ---
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
```

---

## 13. Testing Seams

### Primary Seam: ConnectFn Injection

The same pattern as Go. `Manager::new()` accepts a `ConnectFn`. Tests inject a closure that returns mock upstreams.

```rust
#[cfg(test)]
fn mock_connect_fn(
    upstreams: HashMap<String, Arc<dyn Upstream>>,
) -> ConnectFn {
    Arc::new(move |name: String, _cfg: ServerConfig| {
        let upstream = upstreams.get(&name).cloned();
        Box::pin(async move {
            upstream
                .map(|u| Box::new(u) as Box<dyn Upstream>) // not quite right; see below
                .ok_or_else(|| McpzipError::UnknownServer(name))
        })
    })
}
```

### Mock Upstream

```rust
#[cfg(test)]
pub struct MockUpstream {
    pub tools: Vec<ToolEntry>,
    pub call_result: Box<RawValue>,
    pub alive: AtomicBool,
}

#[async_trait]
impl Upstream for MockUpstream {
    async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
        Ok(self.tools.clone())
    }
    async fn call_tool(&self, tool_name: &str, args: Box<RawValue>) -> Result<Box<RawValue>, McpzipError> {
        // Return pre-configured result, or echo back tool_name + args
        Ok(self.call_result.clone())
    }
    async fn close(&self) -> Result<(), McpzipError> {
        self.alive.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}
```

### In-Memory MCP Transport

For protocol-level tests (equivalent to Go's `mcp.NewInMemoryTransports()`):

```rust
#[cfg(test)]
pub fn memory_transport_pair() -> (MemoryTransport, MemoryTransport);
```

Tests create a pair, pass one to `McpServer::run()` and one to `McpClient::connect()`, then exercise the full JSON-RPC protocol in-process.

### Filesystem Isolation

```rust
#[cfg(test)]
fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
```

Used for: config loading tests, cache roundtrip tests, token store tests.

### Test Organization

| Rust Module | Test File | Category | Go Equivalent |
|---|---|---|---|
| `types` | `src/types.rs` (inline `#[cfg(test)]` mod) | Pure unit | `types_test.go` |
| `config` | `src/config.rs` (inline) | Unit + filesystem | `config_test.go` |
| `catalog` | `src/catalog/catalog.rs` + `cache.rs` (inline) | Unit + filesystem | `catalog_test.go` |
| `search::keyword` | `src/search/keyword.rs` (inline) | Pure unit | `keyword_test.go` |
| `search::query_cache` | `src/search/query_cache.rs` (inline) | Unit + concurrency | `cache_test.go` |
| `search::orchestrated` | `src/search/orchestrated.rs` (inline) | Unit | `search_test.go` |
| `transport::manager` | `src/transport/manager.rs` (inline) | Behavioral | `manager_test.go` |
| `proxy::handlers` | `src/proxy/handlers.rs` (inline) | Unit | `handlers_test.go` |
| `proxy::resources` | `src/proxy/resources.rs` (inline) | Unit | `resources_test.go` |
| `proxy::instructions` | `src/proxy/instructions.rs` (inline) | Unit | `instructions_test.go` |
| `proxy` integration | `tests/integration.rs` | Multi-component | `integration_test.go` |
| `proxy` e2e | `tests/e2e.rs` | Lifecycle | `e2e_test.go` |
| `proxy` mcp | `tests/mcp_protocol.rs` | Protocol-level | `mcp_test.go` |
| `auth::store` | `src/auth/store.rs` (inline) | Unit + filesystem | `store_test.go` |
| `auth::oauth` | `src/auth/oauth.rs` (inline) | Integration | `oauth_test.go` |
| `cli` | `tests/cli.rs` | Smoke + migration | `cli_test.go` |

---

## 14. Component Dependency Graph

```
                          types  (leaf: no deps)
                            ^
                            |
               ┌────────────┼────────────────┐
               |            |                |
            config        error          mcp::protocol
               ^            ^                ^
               |            |                |
               |      ┌─────┼──────┐         |
               |      |     |      |    mcp::transport
               |      |     |      |         ^
               |      |     |      |    ┌────┼────┐
               |      |     |      |    |         |
               |   auth/  catalog  | mcp::server mcp::client
               |   store     ^     |    ^         ^
               |      ^      |     |    |         |
               |      |      |     |    |    ┌────┘
               |   auth/     |     |    |    |
               |   oauth     |     | proxy  transport/
               |      ^      |     |   ^    {stdio,http,sse}
               |      |      |     |   |         ^
               |      └──┐   |     |   |         |
               |         |   |     |   |    transport/
               |         |   |     |   |    manager
               |         |   |     |   |    ^    ^
               |         |   |     |   |    |    |
               |    search/  |     |   └────┤    |
               |    {keyword,|     |        |    |
               |     cache,  |     |   catalog   |
               |     llm,    |     |        |    |
               |     orch.}  |     |        |    |
               |         ^   |     |        |    |
               |         |   |     |        |    |
               └─────────┴───┴─────┴────────┴────┘
                              |
                           cli/serve
                              ^
                              |
                           main.rs
```

### Build Sequence (Implement in This Order)

Each phase is independently testable. No phase depends on an incomplete later phase.

**Phase 1: Pure Leaf Types (0 deps, enables all later phases)**
1. `src/error.rs` -- error enum
2. `src/types.rs` -- ToolEntry, ServerConfig, ProxyConfig, name parsing, compact_params

**Phase 2: Configuration (depends on Phase 1)**
3. `src/config.rs` -- load, validate, path helpers

**Phase 3: MCP Protocol Layer (depends on Phase 1)**
4. `src/mcp/protocol.rs` -- JSON-RPC message types
5. `src/mcp/transport.rs` -- McpTransport trait + MemoryTransport
6. `src/mcp/client.rs` -- MCP client (handshake, list_tools, call_tool)
7. `src/mcp/server.rs` -- MCP server (dispatch loop)

**Phase 4: Auth (depends on Phase 1)**
8. `src/auth/store.rs` -- TokenStore (disk persistence)
9. `src/auth/oauth.rs` -- OAuth handler (PKCE, DCR, browser callback)

**Phase 5: Transport Layer (depends on Phase 1, 3, 4)**
10. `src/transport/mod.rs` -- Upstream trait, ConnectFn type
11. `src/transport/stdio.rs` -- StdioUpstream
12. `src/transport/http.rs` -- StreamableHttpUpstream
13. `src/transport/sse.rs` -- SseUpstream
14. `src/transport/manager.rs` -- Pool, reaper, retry (implements ToolLister)

**Phase 6: Catalog (depends on Phase 1, 5)**
15. `src/catalog/cache.rs` -- disk read/write
16. `src/catalog/catalog.rs` -- Catalog struct

**Phase 7: Search (depends on Phase 1, 6)**
17. `src/search/keyword.rs` -- KeywordSearcher
18. `src/search/query_cache.rs` -- QueryCache
19. `src/search/llm.rs` -- GeminiSearcher stub
20. `src/search/orchestrated.rs` -- OrchestratedSearcher
21. `src/search/mod.rs` -- Searcher trait + factory

**Phase 8: Proxy (depends on Phase 1, 3, 5, 6, 7)**
22. `src/proxy/instructions.rs`
23. `src/proxy/resources.rs`
24. `src/proxy/handlers.rs` -- meta-tool handlers
25. `src/proxy/server.rs` -- wire up MCP server + handlers

**Phase 9: CLI (depends on all above)**
26. `src/cli/serve.rs`
27. `src/cli/init.rs`
28. `src/cli/migrate.rs`
29. `src/cli/status.rs`
30. `src/main.rs`

**Phase 10: Integration Tests (depends on all above)**
31. `tests/integration.rs`
32. `tests/e2e.rs`
33. `tests/mcp_protocol.rs`
34. `tests/cli.rs`

---

## Data Flow: execute_tool

Step-by-step trace of a tool execution request:

```
1. Claude writes JSON-RPC request to stdin:
   {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"execute_tool","arguments":{"name":"slack__send_message","arguments":{"channel":"#general","text":"hello"}}}}

2. mcp::server reads line from stdin, deserializes to JsonRpcRequest

3. mcp::server dispatches method="tools/call" to proxy::Server call handler

4. proxy::handlers::HandleExecuteTool:
   a. Deserialize arguments -> ExecuteToolArgs { name: "slack__send_message", arguments: {...}, timeout: None }
   b. Double-encoding check: if args starts with '"', unwrap one JSON string level
   c. Verify tool exists: catalog.get_tool("slack__send_message") -> Ok(ToolEntry)
   d. Parse prefixed name: parse_prefixed_name("slack__send_message") -> ("slack", "send_message")
   e. Apply per-call timeout if specified (tokio::time::timeout)

5. transport::Manager::call_tool("slack", "send_message", args):
   a. get_upstream("slack"):
      - Read pool: no entry (lazy connect)
      - Call connect_fn("slack", ServerConfig{command:"slack-mcp",...})
      - StdioUpstream::new() spawns subprocess, performs MCP handshake
      - Write pool: insert PoolEntry { upstream, last_used: now }
   b. upstream.call_tool("send_message", args):
      - McpClient sends: {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"send_message","arguments":{...}}}
      - McpClient awaits response via oneshot channel
      - Returns Box<RawValue> with result JSON
   c. On error: evict("slack"), retry once with fresh connection

6. proxy::handlers returns CallToolResult { content: [Text { text: "<raw json>" }], is_error: None }

7. mcp::server serializes JsonRpcResponse and writes to stdout:
   {"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"{\"ok\":true}"}]}}
```

---

## File List

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Create | Project manifest with all dependencies |
| `src/main.rs` | Create | Entry point, clap dispatch |
| `src/error.rs` | Create | McpzipError enum (thiserror) |
| `src/types.rs` | Create | ToolEntry, ServerConfig, ProxyConfig, name helpers |
| `src/config.rs` | Create | Load, validate, path helpers |
| `src/mcp/mod.rs` | Create | Re-exports |
| `src/mcp/protocol.rs` | Create | JSON-RPC 2.0 message types |
| `src/mcp/transport.rs` | Create | McpTransport trait, StdioTransport, MemoryTransport |
| `src/mcp/server.rs` | Create | Downstream MCP server (read loop + dispatch) |
| `src/mcp/client.rs` | Create | Upstream MCP client (handshake + RPC) |
| `src/auth/mod.rs` | Create | Re-exports |
| `src/auth/store.rs` | Create | TokenStore (sha256-keyed disk persistence) |
| `src/auth/oauth.rs` | Create | OAuth 2.1 flow (PKCE, DCR, browser, axum callback) |
| `src/transport/mod.rs` | Create | Upstream trait, ConnectFn type, DefaultConnect |
| `src/transport/manager.rs` | Create | Pool, idle reaper, retry, ToolLister impl |
| `src/transport/stdio.rs` | Create | StdioUpstream (subprocess) |
| `src/transport/http.rs` | Create | StreamableHttpUpstream |
| `src/transport/sse.rs` | Create | SseUpstream (legacy) |
| `src/catalog/mod.rs` | Create | Re-exports, ToolLister trait |
| `src/catalog/catalog.rs` | Create | Catalog struct (in-memory + refresh) |
| `src/catalog/cache.rs` | Create | Disk cache read/write |
| `src/search/mod.rs` | Create | Searcher trait, new_searcher factory |
| `src/search/keyword.rs` | Create | KeywordSearcher |
| `src/search/query_cache.rs` | Create | QueryCache (exact + token overlap) |
| `src/search/llm.rs` | Create | GeminiSearcher stub |
| `src/search/orchestrated.rs` | Create | OrchestratedSearcher |
| `src/proxy/mod.rs` | Create | Re-exports |
| `src/proxy/server.rs` | Create | proxy::Server, tool registration, Run |
| `src/proxy/handlers.rs` | Create | search/describe/execute meta-tool handlers |
| `src/proxy/resources.rs` | Create | Resource/Prompt forwarding stubs |
| `src/proxy/instructions.rs` | Create | Server instructions generation |
| `src/cli/mod.rs` | Create | Re-exports, Cli struct |
| `src/cli/serve.rs` | Create | Serve subcommand |
| `src/cli/init.rs` | Create | Init wizard (dialoguer) |
| `src/cli/migrate.rs` | Create | Claude Code config migration |
| `src/cli/status.rs` | Create | Status display |
| `tests/integration.rs` | Create | Multi-component integration tests |
| `tests/e2e.rs` | Create | End-to-end lifecycle tests |
| `tests/mcp_protocol.rs` | Create | Protocol-level tests via MemoryTransport |
| `tests/cli.rs` | Create | CLI smoke tests |

---

## Cargo.toml Dependencies

```toml
[package]
name = "mcpzip"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mcpzip"
path = "src/main.rs"

[dependencies]
# Async
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }  # CancellationToken
async-trait = "0.1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# CLI
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"

# HTTP
reqwest = { version = "0.12", features = ["json"] }

# OAuth
oauth2 = "4"
axum = "0.7"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Crypto
sha2 = "0.10"
hex = "0.4"

# File paths
dirs = "5"

# Browser open
open = "5"

# Error handling
thiserror = "2"

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
```

---

## Key Conventions Preserved

| # | Convention | Implementation |
|---|---|---|
| 1 | `server__toolname` separator | `NAME_SEPARATOR = "__"`, `parse_prefixed_name` splits on first occurrence |
| 2 | Tool errors as `IsError: true` | Handlers return `CallToolResult { is_error: Some(true), .. }`, never JSON-RPC error |
| 3 | Partial success in ListToolsAll | `JoinSet` collects results; failed servers logged and skipped |
| 4 | Cache-first startup | `catalog.load()` from disk, then `tokio::spawn(refresh_all)` |
| 5 | All diagnostic output to stderr | `eprintln!` and `tracing` subscriber writing to stderr |
| 6 | Double-encoding argument unwrap | Check `args.as_ref().get().starts_with('"')`, unwrap one level |
| 7 | Lazy connections | Pool starts empty; `get_upstream` triggers connect on first use |
| 8 | Retry on stale connections | `call_tool`: on failure, evict + reconnect + retry once |
| 9 | File permissions | Cache dirs: 0755, files: 0600, auth dir: 0700 |
| 10 | Default 2-min tool timeout | `call_timeout_seconds` defaults to 120, overridable per-call |

---

## Memory Budget

Target: under 5MB RSS.

| Component | Estimated RSS |
|---|---|
| tokio runtime (multi-thread) | ~1.5 MB |
| Tool catalog (500 tools, ~1.4KB each) | ~0.7 MB |
| reqwest client (if HTTP upstream) | ~0.5 MB |
| Stack + static | ~0.3 MB |
| Buffers (NDJSON lines) | ~0.1 MB |
| **Total estimate** | **~3.1 MB** |

Key memory decisions:
- `Box<RawValue>` avoids parsing forwarded JSON (InputSchema, arguments, results)
- No persistent SSE buffers unless SSE transport is active
- `tokio::process` child processes have their own RSS (not counted)
- Single-threaded tokio flavor (`#[tokio::main(flavor = "current_thread")]`) could save ~0.5 MB if needed