# Test Strategy: Rust Rewrite of mcpzip

## Testing Approach

Port all 138 Go tests + supplement with Rust-specific tests. Target ~165 total tests.

## Test Categories

### 1. Pure Unit Tests (inline `#[cfg(test)]` modules)

**types.rs** (~12 tests)
- `parse_prefixed_name("slack__send_message")` -> `("slack", "send_message")`
- `parse_prefixed_name("no_separator")` -> error
- `parse_prefixed_name("a__b__c")` -> `("a", "b__c")` (split on first)
- `make_prefixed_name("slack", "send")` -> `"slack__send"`
- `compact_params` on object schema -> required param names
- `compact_params` on schema with no required -> empty
- `compact_params` on null/empty schema -> empty
- `ServerConfig::effective_type()` with URL -> "http"
- `ServerConfig::effective_type()` with command -> "stdio"
- Rust supplement: ToolEntry Send + Sync bounds
- Rust supplement: serde roundtrip for ToolEntry with RawValue
- Rust supplement: serde roundtrip for ProxyConfig

**error.rs** (~3 tests)
- Error Display formatting
- Error downcasting (thiserror From impls)
- Rust supplement: McpzipError is Send + Sync

### 2. Config Tests (inline, with tempdir)

**config.rs** (~14 tests)
- Load valid config from file
- Load config with multiple servers
- Load config missing file -> error
- Load config invalid JSON -> error
- Load config empty servers -> validation error
- Load config with env overrides
- config_dir() returns correct path
- cache_dir() returns correct path
- auth_dir() returns correct path
- EffectiveType for http, stdio, sse
- Validation: server with neither command nor URL
- Config with idle_timeout override
- Config with gemini_api_key
- Config with per-server instructions

### 3. MCP Protocol Tests (inline)

**mcp/protocol.rs** (~8 tests)
- JsonRpcRequest serialize/deserialize roundtrip
- JsonRpcResponse with result
- JsonRpcResponse with error
- JsonRpcNotification serialize
- ID enum: number vs string
- CallToolParams with RawValue arguments
- CallToolResult with TextContent
- InitializeParams/Result roundtrip

**mcp/transport.rs** (~4 tests)
- MemoryTransport send/receive roundtrip
- MemoryTransport concurrent send/receive
- StdioTransport NDJSON framing (newline-delimited)
- Rust supplement: Transport trait is object-safe

**mcp/client.rs** (~5 tests)
- Initialize handshake via MemoryTransport
- list_tools returns parsed tools
- call_tool returns raw result
- Request ID increments
- Error response handling

**mcp/server.rs** (~5 tests)
- Dispatches tools/list to handler
- Dispatches tools/call to handler
- Handles initialize/initialized automatically
- Unknown method returns error
- Rust supplement: server handles concurrent requests

### 4. Auth Tests (inline, with tempdir)

**auth/store.rs** (~8 tests)
- Save + load roundtrip
- Load missing file -> None
- Load corrupt JSON -> None
- Save creates directory with 0700
- Save writes file with 0600
- Different URLs get different files
- Rust supplement: sha256 hash matches Go implementation
- Rust supplement: TokenStore is Send + Sync

**auth/oauth.rs** (~6 tests)
- Callback server starts on ephemeral port
- Multiple servers get independent handlers
- Token persistence after successful auth
- Rust supplement: OAuthHandler is Send + Sync
- PKCE code verifier generation
- Callback server shutdown on drop

### 5. Transport Tests (inline, behavioral)

**transport/manager.rs** (~15 tests)
- Pool reuse: same server returns cached upstream
- Pool miss: new server triggers connect
- Idle reaper: connection removed after timeout
- Idle reaper: recently-used connection kept
- call_tool: success path
- call_tool: retry on first failure (evict + reconnect)
- call_tool: permanent failure after retry exhaustion
- list_tools_all: collects from multiple upstreams
- list_tools_all: partial success (skip failed)
- close_all: shuts down all upstreams
- Configurable idle timeout from config
- Rust supplement: Manager is Send + Sync
- Rust supplement: concurrent call_tool to different servers
- Rust supplement: concurrent call_tool to same server
- Rust supplement: exponential backoff timing

**transport/stdio.rs** (~3 tests)
- StdioUpstream spawns process and connects
- alive() reflects process status
- close() kills child process

### 6. Catalog Tests (inline, with tempdir)

**catalog/cache.rs** (~4 tests)
- Save + load roundtrip (pretty JSON)
- Load missing file -> empty vec
- Load corrupt file -> error
- Save creates parent directory

**catalog/catalog.rs** (~10 tests)
- Load from disk cache
- get_tool returns correct tool
- get_tool missing returns None
- list_tools returns all
- list_tools_for_server filters correctly
- refresh_all updates in-memory and disk
- refresh_all partial success (skip failed server)
- Rust supplement: Catalog is Send + Sync
- Rust supplement: concurrent get_tool during refresh
- Empty catalog behavior

### 7. Search Tests (inline)

**search/keyword.rs** (~10 tests)
- Exact name match scores highest
- Prefix match scores above substring
- Case-insensitive matching
- Tokenization: camelCase split
- Tokenization: underscore split
- Limit parameter respected
- Empty query returns empty
- No matching tools returns empty
- Rust supplement: score ordering is stable
- Rust supplement: unicode handling

**search/query_cache.rs** (~8 tests)
- Exact cache hit
- Normalized key (lowercase, sorted)
- Token overlap partial hit
- Cache miss returns None
- Thread safety: concurrent reads
- Thread safety: concurrent writes
- Rust supplement: cache eviction (if implemented)
- Rust supplement: QueryCache is Send + Sync

**search/orchestrated.rs** (~6 tests)
- Cache shortcut: returns cached result
- Falls back to keyword search on cache miss
- Caches keyword search results
- LLM fallback (stub returns error, keyword result used)
- Respects limit parameter
- Empty catalog returns empty

### 8. Proxy Tests (inline + integration)

**proxy/handlers.rs** (~16 tests)
- search_tools: returns formatted results
- search_tools: empty query
- search_tools: with limit
- describe_tool: returns full schema
- describe_tool: unknown tool -> error
- execute_tool: success path
- execute_tool: unknown tool -> error content (IsError)
- execute_tool: double-encoded string arguments unwrapped
- execute_tool: normal object arguments passed through
- execute_tool: per-call timeout
- execute_tool: default 2-minute timeout
- execute_tool: upstream error -> IsError content
- Rust supplement: handlers are Send + Sync
- Rust supplement: concurrent execute_tool calls
- execute_tool: timeout exceeded -> error content
- execute_tool: empty arguments

**proxy/resources.rs** (~7 tests)
- Resource URI prefixing
- Resource URI unprefixing (for forwarding)
- Prompt name prefixing
- Prompt name unprefixing
- List resources from multiple servers
- Read resource forwards correctly
- Rust supplement: unknown resource -> error

**proxy/instructions.rs** (~3 tests)
- Generate instructions from config
- Per-server instructions included
- Empty servers -> empty instructions

### 9. Integration Tests (separate test files)

**tests/integration.rs** (~4 tests)
- Full search -> describe -> execute flow
- Multiple servers with different transports (mock)
- Catalog refresh during operation
- Error propagation through layers

**tests/e2e.rs** (~5 tests)
- Full proxy lifecycle: start -> tools/list -> tools/call -> shutdown
- Cache persistence: tools.json written and reloaded
- Upstream failure: graceful degradation
- Signal handling: SIGTERM triggers shutdown
- Multiple concurrent tool calls

**tests/mcp_protocol.rs** (~7 tests)
- Protocol compliance via MemoryTransport
- Initialize handshake produces correct capabilities
- tools/list returns 3 meta-tools
- tools/call dispatches to correct handler
- Error responses have correct JSON-RPC format
- Notification handling (initialized)
- Unknown method returns method-not-found error

**tests/cli.rs** (~4 tests)
- serve: starts and responds to initialize
- init: creates config file
- migrate: reads Claude Code config
- --help output includes all subcommands

## Mock Strategy

Hand-written mocks (no mock framework), matching Go pattern:

```rust
// MockUpstream for transport tests
struct MockUpstream {
    tools: Vec<ToolEntry>,
    call_results: Arc<Mutex<VecDeque<Result<Box<RawValue>, McpzipError>>>>,
    alive: AtomicBool,
}

#[async_trait]
impl Upstream for MockUpstream { ... }

// MockConnectFn for manager tests
fn mock_connect_fn(upstreams: HashMap<String, Arc<MockUpstream>>) -> ConnectFn {
    Arc::new(move |name, _cfg| {
        let upstream = upstreams.get(&name).cloned();
        Box::pin(async move {
            upstream.ok_or(McpzipError::ServerNotFound(name))
                .map(|u| Box::new(u) as Box<dyn Upstream>)
        })
    })
}
```

## Test Execution

```bash
# Full suite
cargo test

# Specific module
cargo test --lib types
cargo test --lib mcp
cargo test --lib transport

# Integration tests only
cargo test --test integration --test e2e --test mcp_protocol --test cli

# With output
cargo test -- --nocapture

# Specific test
cargo test test_parse_prefixed_name
```

## Total Test Count

| Category | Count |
|----------|-------|
| types + error | 15 |
| config | 14 |
| mcp (protocol + transport + client + server) | 22 |
| auth (store + oauth) | 14 |
| transport (manager + stdio) | 18 |
| catalog (cache + catalog) | 14 |
| search (keyword + cache + orchestrated) | 24 |
| proxy (handlers + resources + instructions) | 26 |
| integration + e2e + protocol + cli | 20 |
| **Total** | **~167** |
