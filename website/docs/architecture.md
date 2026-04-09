---
sidebar_position: 5
---

# Architecture

## Overview

mcpzip is a dual client/server proxy. On the downstream side, it acts as an MCP server (stdio) that Claude connects to. On the upstream side, it acts as an MCP client connecting to your real MCP servers.

```
                    ┌─────────────────────────────────────────┐
                    │              mcpzip proxy               │
                    │                                         │
  Claude ──stdio──> │  MCP Server ──> ProxyServer             │
                    │                   │                     │
                    │         ┌─────────┼──────────┐          │
                    │         │         │          │          │
                    │    search_tools  describe  execute      │
                    │         │         │          │          │
                    │         v         v          v          │
                    │      Searcher   Catalog   Manager       │
                    │     (kw+LLM)   (cached)  (pool)        │
                    │                              │          │
                    └──────────────────────────────┼──────────┘
                                                   │
                              ┌────────────────────┼────────┐
                              │                    │        │
                         stdio/http/sse       stdio/http   sse
                              │                    │        │
                          Server A            Server B   Server C
```

## Components

### MCP Server (`src/mcp/server.rs`)

NDJSON-over-stdio server implementing the MCP protocol. Handles:
- `initialize` / `initialized` handshake
- `tools/list` — returns the 3 meta-tool definitions
- `tools/call` — dispatches to ProxyServer handlers

### ProxyServer (`src/proxy/`)

The core logic that implements the 3 meta-tools:
- **`handle_search_tools`** — delegates to Searcher, formats results
- **`handle_describe_tool`** — looks up tool in Catalog, returns full schema
- **`handle_execute_tool`** — resolves server from prefixed name, calls via Manager

Also handles admin tools:
- `proxy_status` — returns tool count and server names
- `proxy_refresh` — triggers a catalog refresh

### Catalog (`src/catalog/`)

Maintains a cached index of all tools from all upstream servers.

- **Disk cache** (`~/.config/compressed-mcp-proxy/cache/tools.json`) — persisted between restarts
- **Background refresh** — on startup, serves from cache immediately while connecting to upstream servers in the background
- **Merge on refresh** — if a server fails to connect, keeps its cached tools rather than dropping them

Tool names are prefixed with the server name: `slack__send_message`, `todoist__create_task`. The `__` separator is used to route `execute_tool` calls to the correct upstream server.

### Searcher (`src/search/`)

Two-tier search engine:

1. **KeywordSearcher** — tokenize query, score against tool metadata
2. **GeminiSearcher** (optional) — send compact tool catalog + query to Gemini, get ranked results
3. **OrchestratedSearcher** — runs both, merges and deduplicates

All results go through a **QueryCache** that normalizes queries for cache hits.

### Manager (`src/transport/manager.rs`)

Connection pool for upstream MCP servers:

- **Lazy connections** — servers connect on first use
- **Idle timeout** — disconnects servers after configurable idle period
- **Concurrent list_tools** — connects to all servers in parallel with per-server timeout (30s)
- **Reconnection** — automatic reconnect on connection failure

### Transport (`src/transport/`)

Three transport implementations:

| Transport | File | Protocol |
|-----------|------|----------|
| stdio | `stdio.rs` | Spawn process, NDJSON over stdin/stdout |
| HTTP | `http.rs` | Streamable HTTP with SSE responses, OAuth 2.1 |
| SSE | `sse.rs` | Legacy Server-Sent Events |

### Auth (`src/auth/`)

- **TokenStore** — persists OAuth tokens to disk (`~/.config/compressed-mcp-proxy/auth/`)
- **OAuthHandler** — implements OAuth 2.1 Authorization Code flow with PKCE
  - Discovers authorization/token endpoints from resource metadata
  - Opens browser for authorization
  - Runs local callback server
  - Reuses tokens from mcp-remote if available

## Startup Flow

1. Load config from disk
2. Load cached tool catalog (instant — no network)
3. Create transport manager, searcher, proxy server
4. Start MCP server on stdio
5. Spawn background task to refresh catalog:
   - Connect to all upstream servers concurrently (30s timeout per server)
   - Merge new tools with cached tools
   - Persist updated catalog to disk
6. Begin serving requests immediately (from cache)

## Tool Name Convention

Tools are namespaced by server: `{server}__{tool}`. The double underscore (`__`) separator is used because:
- Single underscore is common in tool names
- Easy to split on first occurrence
- Human-readable in search results

Examples:
- `slack__send_message`
- `todoist__create_task`
- `gmail-personal__send_email`
