# Codebase Exploration: Rust Rewrite of mcpzip

## Overview

mcpzip is a ~2,500 line Go MCP proxy (+ ~3,000 lines of tests) that aggregates N upstream MCP servers behind 3 meta-tools (`search_tools`, `describe_tool`, `execute_tool`). It speaks MCP JSON-RPC over stdio downstream to Claude, and connects to upstreams via stdio subprocesses, streamable HTTP, or SSE.

---

## Architecture

### Dual Protocol Roles
- **Downstream server** (facing Claude): `mcp.Server` over `mcp.StdioTransport` exposing 3 meta-tools
- **Upstream client** (facing real servers): `mcp.Client` + `mcp.ClientSession` per upstream, lazy-connected and pooled

### Data Flow (execute_tool)
```
stdin JSON-RPC -> mcp.Server dispatch -> HandleExecuteTool
  -> unwrap double-encoded args (LLM quirk)
  -> optional per-call timeout
  -> catalog.GetTool (verify exists)
  -> types.ParsePrefixedName("slack__send_message") -> ("slack", "send_message")
  -> transport.Manager.CallTool
    -> GetUpstream (pool hit or lazy connect)
    -> upstream.CallTool (SDK call to real server)
    -> on error: evict + retry once
  -> result back as TextContent
stdout JSON-RPC response
```

### Startup Sequence
1. Set GOMEMLIMIT=20MB
2. Load config JSON
3. Create TokenStore + ConnectFunc (OAuth-capable or not, based on build tag)
4. Create transport.Manager (starts idle reaper goroutine)
5. Load tool catalog from disk cache (instant, may be stale)
6. Create searcher (keyword-only if no Gemini API key)
7. Create proxy.Server
8. Spawn background catalog refresh goroutine
9. Spawn signal handler (SIGTERM/SIGINT -> context cancel)
10. Block on `srv.Run(ctx)` (stdio MCP server loop)

### Module Dependency Graph
```
cmd/mcpzip/main.go -> internal/cli
internal/cli -> config, types, auth, catalog, search, transport, proxy
internal/proxy -> catalog, search, transport, types, go-sdk/mcp
internal/transport -> types, auth (OAuth factory), go-sdk/mcp
internal/catalog -> types
internal/search -> types
internal/config -> types
internal/auth -> golang.org/x/oauth2, go-sdk/auth (build-tagged)
internal/types -> (no internal deps, pure leaf)
```

---

## Key Interfaces (Go -> Rust Traits)

### Upstream (transport/upstream.go)
```go
type Upstream interface {
    ListTools(ctx) ([]ToolEntry, error)
    CallTool(ctx, toolName string, args json.RawMessage) (json.RawMessage, error)
    Close() error
    Alive() bool
}
```
Implementors: `StdioUpstream`, `HTTPUpstream`. Rust: `#[async_trait] trait Upstream: Send + Sync`.

### Searcher (search/search.go)
```go
type Searcher interface {
    Search(ctx, query string, limit int) ([]SearchResult, error)
}
```
Implementors: `KeywordSearcher`, `GeminiSearcher` (stub), `OrchestratedSearcher`. Rust: `#[async_trait] trait Searcher`.

### ToolLister (catalog/catalog.go)
```go
type ToolLister interface {
    ListToolsAll(ctx) (map[string][]ToolEntry, error)
}
```
Implemented by `*transport.Manager`. Rust: `#[async_trait] trait ToolLister`.

### ConnectFunc (transport/factory.go)
```go
type ConnectFunc func(ctx, name string, cfg ServerConfig) (Upstream, error)
```
Injected into Manager for testing. Rust: `Arc<dyn Fn(...) -> BoxFuture<Result<Box<dyn Upstream>>> + Send + Sync>` or a `Connector` trait.

---

## Concurrency Model

| Goroutine | Purpose | Rust Equivalent |
|-----------|---------|----------------|
| Manager.reaper() | Periodic idle connection cleanup | `tokio::spawn` + `tokio::time::interval` |
| Background catalog refresh | One-shot RefreshAll at startup | `tokio::spawn` |
| Signal handler | Cancel context on SIGTERM/SIGINT | `tokio::signal` + `CancellationToken` |
| Per-server ListToolsAll workers | One goroutine per upstream during refresh | `tokio::task::JoinSet` |
| OAuth callback HTTP server | Serves browser redirect | `axum` in `tokio::spawn` |
| MCP server run loop | Main blocking loop | `tokio::main` |

### Shared State

| State | Protection | Rust Equivalent |
|-------|-----------|----------------|
| Catalog (tools, byName, byServer) | `sync.RWMutex` | `Arc<tokio::sync::RwLock<CatalogInner>>` |
| Connection pool | `sync.RWMutex` | `Arc<tokio::sync::RwLock<HashMap<String, PoolEntry>>>` |
| Query cache | `sync.RWMutex` | `Arc<tokio::sync::RwLock<HashMap<String, Vec<SearchResult>>>>` |
| Upstream session/alive | `sync.Mutex` per upstream | `Arc<Mutex<UpstreamInner>>` or `AtomicBool` for alive |

---

## External Boundaries

### MCP Protocol (Wire Format)
- Newline-delimited JSON-RPC 2.0 over stdio (NDJSON)
- `initialize`/`initialized` handshake, `tools/list`, `tools/call`
- Tool errors: `IsError: true` in result content, NOT protocol errors
- The proxy uses a small slice of MCP: AddTool, CallTool, ListTools, StdioTransport, CommandTransport, StreamableClientTransport

### Upstream Transports
- **Stdio**: `exec.CommandContext` spawns subprocess, SDK pipes stdin/stdout. Env inherited + overrides.
- **Streamable HTTP**: POST-based with SSE response streaming (MCP 2025-03-26 spec). Complex (~2,200 lines in SDK).
- **SSE**: Legacy persistent GET + separate POST endpoint.

### OAuth Flow (build-tagged `mcp_go_client_oauth`)
- Local HTTP server on `localhost:0` (random port) for browser callback
- Dynamic client registration (RFC 7591)
- PKCE + authorization code exchange
- Token persistence: `~/.config/compressed-mcp-proxy/auth/{sha256(url)[:32]}.json`
- Cross-platform browser open: `open` (macOS), `xdg-open` (Linux), `cmd /c start` (Windows)

### Filesystem
| Path | Format | Purpose |
|------|--------|---------|
| `~/.config/compressed-mcp-proxy/config.json` | JSON | Main config |
| `~/.config/compressed-mcp-proxy/cache/tools.json` | JSON array (indented) | Tool catalog cache |
| `~/.config/compressed-mcp-proxy/auth/<hash>.json` | JSON, mode 0600 | OAuth tokens |

### Gemini API
- **Currently a stub** - `GeminiSearcher.Search()` returns "not yet implemented"
- Config: `GEMINI_API_KEY` env var or `gemini_api_key` in config, model defaults to `gemini-2.0-flash`
- Only keyword search works today

---

## JSON Handling Patterns

| Go Pattern | Usage | Rust Equivalent |
|-----------|-------|----------------|
| `json.RawMessage` | Opaque JSON forwarding (InputSchema, Arguments) | `Box<serde_json::RawValue>` or `serde_json::Value` |
| `map[string]any` | SDK boundary (CallToolParams.Arguments) | `HashMap<String, serde_json::Value>` |
| Inline anonymous structs | One-shot marshal/unmarshal | `serde_json::json!` macro or local `#[derive(Serialize)]` struct |
| `json.MarshalIndent` | Pretty-print cache files | `serde_json::to_writer_pretty` |
| `omitempty` struct tags | Suppress zero-value fields | `#[serde(skip_serializing_if = "Option::is_none")]` |
| Type switch on `interface{}` | Handle `type: "string"` vs `type: ["string", "null"]` | `#[serde(untagged)] enum` |

### Critical Quirk: Double-Encoded Arguments
LLMs sometimes send `arguments` as `"{\"key\":\"val\"}"` (JSON string) instead of `{"key":"val"}` (object). The proxy detects `args[0] == '"'` and unwraps one level. Must be preserved.

---

## Build Tags / Feature Gating

Go uses `//go:build mcp_go_client_oauth` with file pairs:
- `factory_oauth.go` / `factory_nooauth.go`
- `oauth.go` / `oauth_disabled.go`

Rust equivalent: Cargo features with `#[cfg(feature = "oauth")]`.

---

## Testing

### Test Inventory: 138 tests across 17 files

| Package | Tests | Category |
|---------|-------|----------|
| types | 10 | Pure unit (name parsing, schema extraction) |
| config | 14 | Unit + filesystem (config loading, validation) |
| catalog | 13 | Unit + filesystem (catalog CRUD, cache roundtrip) |
| search/keyword | 9 | Pure unit (scoring, tokenization) |
| search/cache | 7 | Unit + concurrency (overlap matching, thread safety) |
| search/orchestration | 6 | Unit (fallback logic, cache shortcut) |
| transport/manager | 12 | Behavioral (pool reuse, reaper timing, retry) |
| proxy/handlers | 14 | Unit (3 meta-tools, double-encoding fix) |
| proxy/resources | 7 | Unit (URI prefixing stubs) |
| proxy/instructions | 2 | Unit (instruction string generation) |
| proxy/integration | 4 | Multi-component (search->describe->execute) |
| proxy/e2e | 5 | Lifecycle (cache persistence, upstream failure) |
| proxy/mcp | 7 | Protocol-level via `mcp.NewInMemoryTransports()` |
| auth/store | 6 | Unit + filesystem (token roundtrip, permissions) |
| auth/oauth | 6 | Integration (callback server, build-tagged) |
| cli | 10 | Smoke + migration tests |

### Key Test Patterns
- **Hand-written mocks** implementing interfaces (no mock framework)
- **`ConnectFunc` injection** as the primary testing seam
- **In-memory MCP transport** for protocol-level tests
- **`t.TempDir()`** for filesystem isolation
- **Table-driven tests** for parsing functions
- **White-box** (same package) for handler tests, **black-box** (`_test` package) for integration/E2E

### Test Gaps
- No tests for real stdio/HTTP upstream connections
- No tests for SSE transport path
- Gemini search stub never tested with real API
- `cli/init.go` wizard and `cli/status.go` untested

### Test Command
```bash
go test -tags mcp_go_client_oauth ./...  # full suite
go test ./...                             # without OAuth tests
```

---

## Dependency Mapping: Go -> Rust

| Go Component | Go Package | Rust Crate | Notes |
|---|---|---|---|
| MCP server (stdio) | `go-sdk/mcp.Server` | **Build from scratch** | ~200-300 lines NDJSON + JSON-RPC |
| MCP client (stdio subprocess) | `go-sdk/mcp.Client` + `CommandTransport` | **Build from scratch** + `tokio::process` | |
| MCP client (HTTP streamable) | `go-sdk/mcp.StreamableClientTransport` | `reqwest` + custom layer | Complex; defer after stdio works |
| MCP client (SSE) | `go-sdk/mcp.SSEClientTransport` | `reqwest` + SSE streaming | |
| JSON | `encoding/json` | `serde`, `serde_json` | |
| OAuth2 tokens | `golang.org/x/oauth2` | `oauth2` crate | Token type + PKCE flow |
| OAuth callback server | `net/http` | `axum` | Ephemeral local server |
| CLI | `flag` (stdlib) | `clap` (derive) | |
| Async runtime | Go runtime | `tokio` (full) | |
| HTTP client | `net/http` | `reqwest` | For Gemini + HTTP upstreams |
| Hashing | `crypto/sha256` | `sha2` + `hex` | Token store filenames |
| File paths | `path/filepath` + `os.UserHomeDir()` | `dirs` + `std::path` | |
| Signal handling | `os/signal` | `tokio::signal` | |
| Logging | `fmt.Fprintf(os.Stderr, ...)` | `tracing` + stderr subscriber | |
| Browser open | `os/exec` (platform switch) | `open` crate | |

### Critical: No Official Rust MCP SDK
There is no official Rust MCP SDK from Anthropic. Community crates (`rmcp`, `mcp-rs`) exist but lack maturity. **Recommendation: implement the minimal MCP stdio layer directly** (~400-600 lines). The proxy only uses a small slice of MCP (initialize handshake, tools/list, tools/call).

### Binary Size
- Go stripped: ~7.4 MB
- Expected Rust release+stripped: ~2-4 MB

---

## Key Conventions to Preserve

1. **`server__toolname` separator** (`NameSeparator = "__"`, split on first occurrence)
2. **Tool errors as `IsError: true` content**, not protocol errors
3. **Partial success semantics** in ListToolsAll (skip failed servers)
4. **Cache-first startup** (serve immediately from disk, refresh in background)
5. **All diagnostic output to stderr** (stdout = MCP protocol channel)
6. **Double-encoding argument unwrap** (LLM quirk workaround)
7. **Lazy connection** (no upstream connects at startup)
8. **One-shot retry** on stale connection (evict + reconnect + retry)
9. **File permissions**: config/tokens 0600, auth dir 0700

---

## Risks and Recommendations

1. **MCP SDK gap**: Build minimal JSON-RPC/stdio layer. Budget ~500 lines. The wire format is simple NDJSON.
2. **Streamable HTTP complexity**: The go-sdk's streamable transport is ~2,200 lines. Defer HTTP upstream support; implement stdio first (covers most MCP servers).
3. **`json.RawMessage` lifetime complexity**: Use `Box<serde_json::RawValue>` for stored schemas, `serde_json::Value` for dynamic args.
4. **Async trait object safety**: Use `async-trait` crate or Rust 1.75+ RPITIT for `Upstream` and `Searcher` traits.
5. **Token JSON format**: Preserve Go's `oauth2.Token` field names (`access_token`, `token_type`, etc.) for backward compat with existing cached tokens. (Note: Go's `oauth2.Token` uses lowercase JSON tags, not PascalCase.)
6. **In-memory transport for tests**: Use `tokio::io::duplex()` if no SDK provides one.
