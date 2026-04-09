# mcpzip

## Project Overview
An MCP proxy that aggregates multiple upstream MCP servers and exposes them via a Search + Execute pattern. Instead of loading hundreds of tool schemas into context, Claude uses 3 meta-tools (search_tools, describe_tool, execute_tool) to discover and invoke upstream tools on demand.

## Tech Stack
- Rust (async with tokio)
- Gemini Flash for LLM-powered tool search (configurable backend)
- Single binary deployment

## Key Architecture
- Dual client/server proxy: Server (stdio downstream to Claude) + Client (stdio/HTTP upstream to real MCP servers)
- 3 meta-tools: search_tools, describe_tool (optional), execute_tool
- Disk-cached tool catalog with async background refresh
- Connection pool with idle timeout for upstream servers
- Full MCP proxy (tools via search pattern, resources + prompts forwarded directly)

## Commands
- `cargo build` - Build
- `cargo test` - Run tests
- `cargo run -- serve` - Run proxy (stdio mode)
- `cargo run -- init` - Interactive setup wizard
- `cargo run -- migrate` - Auto-migrate from Claude Code config

## Project Structure
```
src/
  main.rs          - Entry point
  lib.rs           - Module declarations
  config.rs        - Configuration loading
  error.rs         - Error types
  types.rs         - Core types (ToolEntry, ServerConfig, ProxyConfig)
  cli/
    mod.rs         - CLI definition (clap)
    serve.rs       - Proxy server command
    init.rs        - Interactive setup wizard
    migrate.rs     - Claude Code config migration
  auth/
    store.rs       - Token persistence
    oauth.rs       - OAuth 2.1 browser flow with PKCE
  proxy/
    server.rs      - ProxyServer and meta-tool definitions
    handlers.rs    - search_tools, describe_tool, execute_tool handlers
    instructions.rs - Dynamic instructions generation
    resources.rs   - Resource/prompt forwarding
  catalog/
    cache.rs       - Disk cache
    catalog.rs     - Tool catalog with background refresh
  search/
    keyword.rs     - Keyword-based search
    llm.rs         - Gemini-powered semantic search
    orchestrated.rs - Orchestrated search (keyword + LLM)
    query_cache.rs - Query result caching
  transport/
    manager.rs     - Connection pool with idle timeout
    stdio.rs       - Stdio upstream transport
    http.rs        - HTTP/Streamable HTTP transport
    sse.rs         - SSE transport
  mcp/
    protocol.rs    - MCP protocol types
    server.rs      - MCP server (NDJSON over stdio)
    client.rs      - MCP client
    transport.rs   - Transport abstraction
```

## Config Location
~/.config/compressed-mcp-proxy/config.json
