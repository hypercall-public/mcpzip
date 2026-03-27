# Specification: Rust Rewrite of mcpzip

## S1: Compatibility

- **Drop-in replacement**: Same binary name (`mcpzip`), same config.json format, same CLI commands (`serve`, `init`, `migrate`), same cache paths, same auth token paths
- Existing users swap the binary with zero config changes
- Config location: `~/.config/compressed-mcp-proxy/config.json`
- Cache location: `~/.config/compressed-mcp-proxy/cache/tools.json`
- Auth tokens: `~/.config/compressed-mcp-proxy/auth/{sha256(url)[:16]}.json`

## S2: MCP Protocol Layer

- **Build from scratch**: Minimal custom JSON-RPC 2.0 / NDJSON layer (~400-600 lines)
- Only implements what the proxy uses: `initialize`/`initialized` handshake, `tools/list`, `tools/call`
- Also: `resources/list`, `resources/read`, `prompts/list`, `prompts/get` for forwarding
- No external MCP SDK dependency
- Wire format: newline-delimited JSON-RPC 2.0 over stdio

## S3: Upstream Transports

- **All three at launch**: stdio (subprocess), Streamable HTTP, SSE (legacy)
- Stdio: `tokio::process::Command`, pipe stdin/stdout, env inherited + per-server overrides
- Streamable HTTP: POST-based with SSE response streaming (MCP 2025-03-26 spec)
- SSE: Legacy persistent GET + separate POST endpoint
- Lazy connection (no upstream connects at startup)
- One-shot retry on stale connections with configurable retry + exponential backoff

## S4: OAuth

- **Always enabled** (no feature flag)
- Full OAuth 2.1: PKCE, dynamic client registration (RFC 7591), browser callback
- Local HTTP server on ephemeral port for browser redirect
- Token persistence to disk (same paths as Go version, mode 0600)
- Cross-platform browser open (`open` on macOS, `xdg-open` on Linux, `cmd /c start` on Windows)

## S5: Meta-Tools

- **All 3 meta-tools**: `search_tools`, `describe_tool`, `execute_tool`
- search_tools: keyword-only search (Gemini stub stays stub)
- describe_tool: returns full tool schema for a named tool
- execute_tool: forwards call to upstream, with double-encoding argument unwrap
- **Default 2-minute timeout** on all tool calls, overridable per-call via `timeout` field (seconds)

## S6: Resource/Prompt Forwarding

- Forward resources and prompts from upstream servers
- Server-prefixed URIs (same scheme as Go version)
- Full parity with Go's current stub implementation

## S7: Server Instructions

- Generate server instructions from config
- Include per-server instruction strings
- Match Go behavior exactly

## S8: CLI

- **Framework**: clap (derive macros)
- **Commands**: `serve`, `init`, `migrate`
- `init`: interactive wizard using `dialoguer` crate
- `migrate`: reads Claude Code config, same logic as Go version

## S9: Error Handling

- **thiserror** for custom error enums
- Tool errors as `IsError: true` content (not protocol errors) -- matches Go
- Improved retry logic: configurable retry count, exponential backoff beyond Go's one-shot retry

## S10: Concurrency

- **Runtime**: tokio (full features)
- **Shared state**: `tokio::sync::RwLock` for connection pool (needs to hold across .await during connect); `std::sync::RwLock` acceptable for catalog and query cache (short critical sections, no .await while locked)
- **Async traits**: `async-trait` crate for Upstream, Searcher, ToolLister traits
- Connection pool behind `Arc<tokio::sync::RwLock<...>>` + per-server `tokio::sync::Mutex` to prevent thundering herd
- Catalog and query cache behind `Arc<std::sync::RwLock<...>>`
- Idle connection reaper via `tokio::spawn` + `tokio::time::interval`
- Signal handling via `tokio::signal`
- Background catalog refresh via `tokio::spawn`

## S11: JSON Handling

- **RawValue everywhere possible**: `Box<serde_json::RawValue>` for opaque forwarding (InputSchema, arguments passthrough)
- `serde_json::Value` only where manipulation needed (double-encoding fix)
- Preserve `__` name separator convention (split on first occurrence)
- Pretty-print cache files with `serde_json::to_writer_pretty`

## S12: Logging

- **tracing** crate with stderr subscriber
- Filterable via `RUST_LOG` env var
- All diagnostic output to stderr (stdout = MCP protocol channel)

## S13: Memory Target

- **Under 5MB RSS** target
- Tool catalog is ~700KB, tokio runtime ~1-2MB, leaves headroom for connections
- Aggressive: minimize buffering, use RawValue to avoid parsing forwarded JSON

## S14: Testing

- **Port Go tests + supplement with Rust-specific tests**
- Port all 138 Go test cases (behavioral spec for the port)
- Add Rust-specific: Send/Sync bounds, drop semantics, async cancellation
- Hand-written mocks (match Go pattern, no mock framework)
- `ConnectFunc` injection as primary testing seam
- In-memory transport via `tokio::io::duplex()` for protocol tests
- `tempdir` for filesystem isolation

## S15: Project Structure

- **Replace at root**: Rust project at repo root, remove Go code
- **Keep Go alongside temporarily** in `go/` subdirectory during implementation, remove after verification
- Binary name: `mcpzip`
- Cargo.toml at repo root

## S16: Distribution

- **cargo-dist / GitHub Actions** for automated cross-platform builds
- Targets: macOS (arm64/x86_64), Linux (x86_64/aarch64), Windows

## S17: Rust Toolchain

- **Latest stable** (no MSRV guarantee)
- Edition 2021 or 2024

## S18: Dependency Map

| Purpose | Crate |
|---------|-------|
| Async runtime | tokio (full) |
| Serialization | serde, serde_json |
| CLI | clap (derive) |
| HTTP client | reqwest |
| Logging | tracing, tracing-subscriber |
| Error handling | thiserror |
| Async traits | async-trait |
| OAuth2 | oauth2 |
| Hashing | sha2, hex |
| File paths | dirs |
| Browser open | open |
| Interactive prompts | dialoguer |
| Signal handling | tokio::signal (built-in) |

## S19: Key Conventions (Preserved from Go)

1. `server__toolname` separator (`__`, split on first occurrence)
2. Tool errors as `IsError: true` content, not protocol errors
3. Partial success in ListToolsAll (skip failed servers)
4. Cache-first startup (serve from disk, refresh in background)
5. All diagnostic output to stderr
6. Double-encoding argument unwrap (LLM quirk)
7. Lazy connections (no upstream connects at startup)
8. Retry on stale connections (improved: configurable + backoff)
9. File permissions: config/tokens 0600, auth dir 0700
10. Default 2-minute timeout on tool calls, overridable per-call

## S20: Idle Timeout

- **Configurable via config.json** (e.g., `"idle_timeout_minutes": 5`)
- Default: 5 minutes (Go default is 10, intentionally reduced for Rust)
- Applied to connection pool reaper

## S21: Admin Tools

- `proxy_status`: returns JSON with `tool_count` and `server_names`
- `proxy_refresh`: triggers on-demand catalog refresh, returns status + new tool count
- Both handled inside execute_tool before catalog lookup (they may not be in catalog)

## S22: Review Findings (Incorporated)

Technical decisions from plan review:
- Connection pool uses `tokio::sync::RwLock` (not `std::sync::RwLock`) to avoid blocking tokio runtime during connect
- Per-server `tokio::sync::Mutex` to prevent thundering herd on concurrent connects to same server
- `call_tool` args parameter: `serde_json::Value` in Manager (cloneable for retry), convert to RawValue at Upstream boundary
- MCP server handler signature: accept `String` (not `&str`) for tool names in closures returning futures
- `JsonRpcMessage` deserialization: manual impl dispatching on field presence (not `#[serde(untagged)]`)
- `CallToolResult` to `RawValue` conversion: handle structured content, single text content (with JSON validity check), and full content array fallback
- Additional dependencies: `tempfile` (test), `axum` (OAuth callback), `tokio-util` (CancellationToken), SSE parsing crate (e.g., `reqwest-eventsource`)
- `EffectiveType` defaults to "stdio" (matches Go behavior, not auto-detect from URL)
