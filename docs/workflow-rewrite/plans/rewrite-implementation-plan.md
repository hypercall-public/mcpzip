# Implementation Plan: Rust Rewrite of mcpzip

## Implementation Phases

Implementation follows the dependency graph from the architecture. Each phase is independently testable.

---

### Phase 1: Project Scaffold + Pure Leaf Types

**Files**: `Cargo.toml`, `src/main.rs`, `src/error.rs`, `src/types.rs`
**Dependencies**: None
**Estimated Tests**: ~15

#### Task 1.1: Cargo.toml + main.rs stub
- Create `Cargo.toml` with all dependencies from architecture
- Create minimal `src/main.rs` that compiles

#### Task 1.2: Error types (`src/error.rs`)
- `McpzipError` enum with thiserror
- Variants: Io, Json, Protocol, Transport, Config, Auth, Timeout, ToolNotFound, ServerNotFound

#### Task 1.3: Core types (`src/types.rs`)
- `ToolEntry` struct (server_name, tool_name, description, input_schema as `Box<RawValue>`)
- `ServerConfig` struct (command, args, env, url, transport_type, instructions)
- `ProxyConfig` struct (mcp_servers HashMap, gemini_api_key, idle_timeout)
- `SearchResult` struct (server_name, tool_name, description, score)
- `parse_prefixed_name()` function (split on first `__`)
- `make_prefixed_name()` function
- `compact_params()` - extract required param names from JSON schema
- Port all 10 Go `types_test.go` tests

---

### Phase 2: Configuration

**Files**: `src/config.rs`
**Dependencies**: Phase 1 (types, error)
**Estimated Tests**: ~14

#### Task 2.1: Config loading and validation (`src/config.rs`)
- `load()` - read and parse `~/.config/compressed-mcp-proxy/config.json`
- `load_from()` - load from explicit path (testable)
- `config_dir()`, `cache_dir()`, `auth_dir()` path helpers
- Validation: at least one server, valid transport types, URL required for http/sse
- `EffectiveType()` logic: if URL set -> "http", if command set -> "stdio"
- Port all 14 Go `config_test.go` tests

---

### Phase 3: MCP Protocol Layer

**Files**: `src/mcp/mod.rs`, `src/mcp/protocol.rs`, `src/mcp/transport.rs`, `src/mcp/client.rs`, `src/mcp/server.rs`
**Dependencies**: Phase 1 (types, error)
**Estimated Tests**: ~20

#### Task 3.1: Protocol types (`src/mcp/protocol.rs`)
- `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcNotification`, `JsonRpcError`
- `ID` enum (Number/String)
- MCP-specific params: `InitializeParams`, `InitializeResult`, `CallToolParams`, `CallToolResult`, `ListToolsResult`
- `TextContent`, `ToolInfo`, `ServerCapabilities`, `ClientCapabilities`

#### Task 3.2: Transport trait + implementations (`src/mcp/transport.rs`)
- `McpTransport` trait: `async fn send(&self, msg)`, `async fn receive(&self) -> msg`
- `StdioTransport` - reads NDJSON from stdin, writes to stdout
- `MemoryTransport` - uses `tokio::io::DuplexStream` for tests
- `ProcessTransport` - wraps `tokio::process::Child` stdin/stdout

#### Task 3.3: MCP Client (`src/mcp/client.rs`)
- `McpClient` struct wrapping a transport
- `initialize()` handshake (send initialize, receive result, send initialized notification)
- `list_tools()` -> `Vec<ToolInfo>`
- `call_tool(name, args)` -> `Box<RawValue>`
- Request ID tracking with `AtomicU64`
- Response routing via `HashMap<ID, oneshot::Sender>`
- Background reader task that dispatches responses

#### Task 3.4: MCP Server (`src/mcp/server.rs`)
- `McpServer` struct with registered handlers
- `run()` - read loop on stdin, dispatch by method name
- Handler registration: `on_tools_list`, `on_tools_call`
- Also: `on_resources_list`, `on_resources_read`, `on_prompts_list`, `on_prompts_get`
- Handles `initialize`/`initialized` handshake automatically
- Returns `ServerCapabilities` with tools, resources, prompts

---

### Phase 4: Auth

**Files**: `src/auth/mod.rs`, `src/auth/store.rs`, `src/auth/oauth.rs`
**Dependencies**: Phase 1 (types, error)
**Estimated Tests**: ~12

#### Task 4.1: Token store (`src/auth/store.rs`)
- `TokenStore` struct with `base_dir: PathBuf`
- `load(server_url)` -> `Option<Token>` (sha256 hash filename)
- `save(server_url, token)` -> write JSON, mode 0600
- `ensure_dir()` -> create dir with mode 0700
- Port 6 Go `store_test.go` tests

#### Task 4.2: OAuth handler (`src/auth/oauth.rs`)
- `OAuthHandler` struct wrapping `oauth2::Client` + `TokenStore`
- `new(server_url, store)` -> creates handler
- OAuth 2.1 metadata discovery (`.well-known/oauth-authorization-server`)
- Dynamic client registration (RFC 7591) via POST to registration endpoint
- PKCE code challenge generation
- `authorize()` -> spawns axum callback server on localhost:0, opens browser, waits for code
- Token exchange and persistence
- Port 6 Go `oauth_test.go` tests

---

### Phase 5: Transport Layer

**Files**: `src/transport/mod.rs`, `src/transport/stdio.rs`, `src/transport/http.rs`, `src/transport/sse.rs`, `src/transport/manager.rs`
**Dependencies**: Phase 1 (types, error), Phase 3 (mcp), Phase 4 (auth)
**Estimated Tests**: ~20

#### Task 5.1: Upstream trait + ConnectFn (`src/transport/mod.rs`)
- `Upstream` trait: `list_tools`, `call_tool`, `close`, `alive`
- `ConnectFn` type alias: `Arc<dyn Fn(String, ServerConfig) -> BoxFuture<Result<Box<dyn Upstream>>> + Send + Sync>`
- `default_connect()` factory function

#### Task 5.2: StdioUpstream (`src/transport/stdio.rs`)
- Spawns subprocess via `tokio::process::Command`
- Wraps `McpClient` with `ProcessTransport`
- `list_tools()` -> MCP client list_tools
- `call_tool()` -> MCP client call_tool
- `close()` -> kill child process
- `alive()` -> check child process status

#### Task 5.3: StreamableHttpUpstream (`src/transport/http.rs`)
- POST requests with JSON-RPC body
- SSE response streaming for long operations
- Session ID tracking via `Mcp-Session-Id` header
- OAuth integration: if 401, trigger OAuth flow then retry
- reqwest client with connection pooling

#### Task 5.4: SseUpstream (`src/transport/sse.rs`)
- Legacy SSE transport
- Persistent GET for server-sent events
- Separate POST for client-to-server messages
- Endpoint discovery from SSE stream

#### Task 5.5: Connection Manager (`src/transport/manager.rs`)
- `Manager` struct with `Arc<RwLock<HashMap<String, PoolEntry>>>`
- `PoolEntry`: upstream, last_used timestamp
- `get_upstream(name)` -> lazy connect via ConnectFn
- `call_tool(server, tool, args)` -> get upstream + call + retry on failure
- Idle reaper: `tokio::spawn` + `tokio::time::interval`, configurable timeout
- Retry: evict + reconnect + retry once (with exponential backoff for repeated failures)
- Implements `ToolLister` trait (list_tools from all upstreams via JoinSet)
- Port 12 Go `manager_test.go` tests

---

### Phase 6: Catalog

**Files**: `src/catalog/mod.rs`, `src/catalog/catalog.rs`, `src/catalog/cache.rs`
**Dependencies**: Phase 1 (types), Phase 5 (transport - ToolLister)
**Estimated Tests**: ~13

#### Task 6.1: Disk cache (`src/catalog/cache.rs`)
- `load_cache(path)` -> `Vec<ToolEntry>` (reads tools.json)
- `save_cache(path, tools)` -> write pretty JSON

#### Task 6.2: Catalog (`src/catalog/catalog.rs`)
- `Catalog` struct with `Arc<RwLock<CatalogInner>>`
- `CatalogInner`: all_tools Vec, by_name HashMap, by_server HashMap
- `load()` -> populate from disk cache
- `refresh_all(lister)` -> call ToolLister, update in-memory, save to disk
- `get_tool(prefixed_name)` -> Option<ToolEntry>
- `list_tools()` -> Vec<ToolEntry>
- `list_tools_for_server(name)` -> Vec<ToolEntry>
- Partial success: skip servers that fail during refresh
- Port 13 Go `catalog_test.go` tests

---

### Phase 7: Search

**Files**: `src/search/mod.rs`, `src/search/keyword.rs`, `src/search/query_cache.rs`, `src/search/llm.rs`, `src/search/orchestrated.rs`
**Dependencies**: Phase 1 (types), Phase 6 (catalog)
**Estimated Tests**: ~22

#### Task 7.1: Keyword searcher (`src/search/keyword.rs`)
- `KeywordSearcher` struct holding reference to catalog
- Tokenization: split on whitespace, underscores, camelCase boundaries
- Scoring: exact match > prefix > contains, weighted by field (name > description)
- Top-N results by score
- Port 9 Go `keyword_test.go` tests

#### Task 7.2: Query cache (`src/search/query_cache.rs`)
- `QueryCache` struct with `Arc<RwLock<HashMap<String, Vec<SearchResult>>>>`
- Normalized cache key (lowercase, sorted tokens)
- Token overlap matching for partial cache hits
- Thread safety tests
- Port 7 Go `cache_test.go` tests

#### Task 7.3: Gemini searcher stub (`src/search/llm.rs`)
- `GeminiSearcher` struct (API key, model name)
- `search()` -> returns "not yet implemented" error
- Placeholder for future LLM-powered search

#### Task 7.4: Orchestrated searcher (`src/search/orchestrated.rs`)
- `OrchestratedSearcher` wrapping keyword + LLM + cache
- Flow: check cache -> keyword search -> (future: LLM fallback) -> cache result
- Port 6 Go `search_test.go` tests

---

### Phase 8: Proxy

**Files**: `src/proxy/mod.rs`, `src/proxy/server.rs`, `src/proxy/handlers.rs`, `src/proxy/resources.rs`, `src/proxy/instructions.rs`
**Dependencies**: Phase 1 (types), Phase 3 (mcp), Phase 5 (transport), Phase 6 (catalog), Phase 7 (search)
**Estimated Tests**: ~23

#### Task 8.1: Instructions (`src/proxy/instructions.rs`)
- Generate instruction string from config
- Include per-server instruction strings
- Port 2 Go `instructions_test.go` tests

#### Task 8.2: Resource/Prompt forwarding (`src/proxy/resources.rs`)
- Resource URI prefixing with server name
- Prompt name prefixing with server name
- Forward `resources/list`, `resources/read`, `prompts/list`, `prompts/get`
- Port 7 Go `resources_test.go` tests

#### Task 8.3: Meta-tool handlers (`src/proxy/handlers.rs`)
- `handle_search_tools(args)` -> search catalog, format results
- `handle_describe_tool(args)` -> lookup tool, return full schema
- `handle_execute_tool(args)` -> parse name, unwrap double-encoded args, call upstream
- Admin tools: `proxy_status` (returns tool count + server names) and `proxy_refresh` (triggers catalog refresh)
- Default 2-minute timeout, overridable per-call
- Port 14 Go `handlers_test.go` tests + proxy_status/proxy_refresh tests

#### Task 8.4: Proxy server (`src/proxy/server.rs`)
- `Server` struct tying everything together
- Register 3 meta-tools with MCP server
- Register resource/prompt handlers
- Set server instructions
- `run(ctx)` -> start MCP server stdio loop

---

### Phase 9: CLI

**Files**: `src/cli/mod.rs`, `src/cli/serve.rs`, `src/cli/init.rs`, `src/cli/migrate.rs`, `src/cli/status.rs`, update `src/main.rs`
**Dependencies**: All above
**Estimated Tests**: ~10

#### Task 9.1: CLI framework (`src/cli/mod.rs` + `src/main.rs`)
- clap derive struct with subcommands: serve, init, migrate, status
- Global flags: --config

#### Task 9.2: Serve command (`src/cli/serve.rs`)
- Load config
- Create TokenStore, ConnectFn, Manager
- Load catalog from cache
- Create searcher
- Create proxy Server
- Spawn background catalog refresh
- Spawn signal handler (SIGTERM/SIGINT)
- Block on server.run()

#### Task 9.3: Init wizard (`src/cli/init.rs`)
- dialoguer-based interactive setup
- Ask for server name, type, command/URL
- Write config.json

#### Task 9.4: Migrate command (`src/cli/migrate.rs`)
- Read Claude Code's `~/.claude/` config
- Convert to mcpzip config format
- Port Go migrate logic + tests

---

### Phase 10: Integration + E2E Tests

**Files**: `tests/integration.rs`, `tests/e2e.rs`, `tests/mcp_protocol.rs`, `tests/cli.rs`
**Dependencies**: All above
**Estimated Tests**: ~16

#### Task 10.1: Integration tests (`tests/integration.rs`)
- Multi-component: search -> describe -> execute flow
- Port 4 Go `integration_test.go` tests

#### Task 10.2: E2E tests (`tests/e2e.rs`)
- Full lifecycle: startup, cache, shutdown
- Port 5 Go `e2e_test.go` tests

#### Task 10.3: Protocol tests (`tests/mcp_protocol.rs`)
- MCP protocol correctness via MemoryTransport
- Port 7 Go `mcp_test.go` tests

#### Task 10.4: CLI tests (`tests/cli.rs`)
- Smoke tests for each subcommand
- Port Go cli tests

---

## Go Code Migration Plan

Before Phase 1 starts:
1. Create `go-legacy/` directory
2. Move all Go files (*.go, go.mod, go.sum) into `go-legacy/`
3. Move `internal/` into `go-legacy/`
4. Move `cmd/` into `go-legacy/`
5. Keep `.gitignore`, `CLAUDE.md`, `docs/` at root
6. Rust project starts at root with `Cargo.toml`, `src/`

---

## Test Strategy Per Phase

| Phase | Test Type | Test Command |
|-------|-----------|-------------|
| 1 | Unit (inline `#[cfg(test)]`) | `cargo test --lib types error` |
| 2 | Unit + filesystem | `cargo test --lib config` |
| 3 | Unit + protocol | `cargo test --lib mcp` |
| 4 | Unit + filesystem | `cargo test --lib auth` |
| 5 | Behavioral (mock upstreams) | `cargo test --lib transport` |
| 6 | Unit + filesystem | `cargo test --lib catalog` |
| 7 | Unit | `cargo test --lib search` |
| 8 | Unit (mock dependencies) | `cargo test --lib proxy` |
| 9 | Smoke | `cargo test --lib cli` |
| 10 | Integration + E2E | `cargo test --test integration --test e2e --test mcp_protocol --test cli` |

Full test suite: `cargo test`

---

## Parallel Implementation Opportunities

These phases can run in parallel (no dependencies between them):
- Phase 2 (config) + Phase 3 (MCP protocol) + Phase 4 (auth) -- all only depend on Phase 1
- Phase 6 (catalog) + Phase 7 (search) can partially overlap once Phase 5 traits are defined

Sequential dependencies:
- Phase 5 (transport) requires Phase 3 + Phase 4
- Phase 8 (proxy) requires Phase 5 + Phase 6 + Phase 7
- Phase 9 (CLI) requires Phase 8
- Phase 10 (integration tests) requires Phase 9

---

## Total Estimated Scope

- **30+ source files** to create
- **~4,000-5,000 lines** of Rust (vs ~2,500 Go)
- **~165 tests** (ported from Go + Rust-specific supplements)
- **4 integration test files**
