# Specification: mcpzip (Compressed MCP Proxy)

## Overview

mcpzip is an MCP proxy server that aggregates multiple upstream MCP servers and exposes them through a Search + Execute pattern. Instead of loading hundreds of tool schemas into Claude's context window, Claude interacts with 3 lightweight meta-tools to discover and invoke upstream tools on demand.

## Problem Statement

MCP tool definitions consume 10-57% of Claude's 200k context window. With all servers enabled (Telegram x2, Linear, Notion, Slack, Paper, Chrome, Vercel, Google Workspace x3), tool schemas can consume nearly 100% of available context. Even Claude Code's built-in deferred/ToolSearch mechanism still loads tool names into the system prompt.

## Solution

A single MCP proxy binary (`mcpzip`) that:
1. Aggregates all upstream MCP servers behind one connection
2. Exposes only 3 meta-tools to Claude (search_tools, describe_tool, execute_tool)
3. Uses LLM-powered search (Gemini Flash) to match natural language queries to tools
4. Caches tool catalogs to disk for fast startup

---

## Core Functionality

### Meta-Tools (Always Exposed)

These 3 tools are the only ones Claude sees in its tool list:

#### 1. search_tools
- **Input**: `query` (string, required), `limit` (integer, optional, default 5)
- **Behavior**: Takes a natural language query, sends it with the full tool catalog to Gemini Flash, returns the top matching tools
- **Output**: Array of matches, each containing:
  - `name`: fully qualified prefixed name (e.g., `telegram-jakesyl__send_message`)
  - `description`: first sentence of the tool's description
  - `params`: compact parameter summary (name + type for each param, required markers)
- **Caching**: Results are cached with keyword-overlap semantic matching. Subsequent similar queries return cached results without hitting Gemini.

#### 2. describe_tool
- **Input**: `name` (string, required) - the fully qualified tool name from search results
- **Behavior**: Returns the complete, uncompressed tool schema from the cached catalog
- **Output**: Full JSON Schema including all parameter descriptions, types, enums, defaults
- **Purpose**: Optional step for complex tools where Claude needs full schema detail before calling execute_tool

#### 3. execute_tool
- **Input**: `name` (string, required), `arguments` (object, required)
- **Behavior**: Routes the call to the correct upstream server, forwards arguments as-is, returns the upstream response unmodified
- **Output**: Upstream tool's response (content array, isError flag) passed through transparently
- **Validation**: None - arguments are forwarded as-is. Upstream server handles validation.

### Admin Tools (Searchable Only)

These tools are discoverable via search_tools but not in the static tool list:

#### proxy_status
- Returns: connected upstream servers, tool counts per server, cache age, server health

#### proxy_refresh
- Forces re-fetch of tool catalogs from all upstream servers
- Updates the disk cache

### Resources and Prompts

Resources and prompts from upstream servers are forwarded directly with their full definitions (no search pattern). They are aggregated and prefixed with the server name to avoid collisions.

### Server Instructions

Upstream server instructions are summarized using the LLM (Gemini Flash) into a concise merged version. This summarized instruction is returned in the proxy's initialize response.

---

## Architecture

### Deployment Model
- Single proxy replaces ALL MCP server entries in Claude Code config
- Claude Code connects to mcpzip via stdio
- mcpzip connects to upstream servers via stdio or HTTP

### Connection Lifecycle
```
Startup:
  1. Load disk-cached tool catalog (instant)
  2. Start serving immediately
  3. Background: refresh catalog from each upstream independently
     - Per-server timeouts (don't let one slow server block others)
     - Non-blocking (don't prevent proxy shutdown)

First tool execution for a server:
  1. Establish connection to upstream (stdio spawn or HTTP connect)
  2. Execute the tool call
  3. Keep connection in pool

Subsequent calls to same server:
  1. Reuse pooled connection (fast)

Idle timeout:
  1. If no calls to a server for N minutes, tear down connection
  2. Re-establish on next call
```

### Upstream Failure Handling
- On failure: attempt restart with backoff
- If still failing: mark server's tools as unavailable
- search_tools continues returning results from healthy servers
- execute_tool returns clear error for failed server's tools

### Tool Name Collisions
- All tools are auto-prefixed with their server name: `servername__toolname`
- Example: `telegram-jakesyl__send_message`, `slack__channels_list`

---

## Configuration

### Config Format
JSON, matching Claude Code's .mcp.json schema for upstream server definitions.

### Config Location
`~/.config/compressed-mcp-proxy/config.json`

### Config Schema
```json
{
  "gemini_api_key": "...",
  "search": {
    "default_limit": 5,
    "model": "gemini-2.0-flash"
  },
  "idle_timeout_minutes": 10,
  "mcpServers": {
    "telegram-jakesyl": {
      "command": "python",
      "args": ["/path/to/telegram-mcp/main.py"],
      "env": {
        "TELEGRAM_API_ID": "...",
        "TELEGRAM_API_HASH": "...",
        "TELEGRAM_SESSION_NAME": "jakesyl"
      }
    },
    "slack": {
      "type": "http",
      "url": "https://slack-mcp.example.com/mcp"
    }
  }
}
```

### Cache Location
`~/.config/compressed-mcp-proxy/cache/tools.json` - disk-cached tool catalog

### Secrets
API keys and tokens stored directly in config.json. No env var indirection required (but env vars in server definitions are supported for upstream server env).

---

## CLI Commands

### `mcpzip serve`
Run the proxy in stdio mode. This is what Claude Code invokes.

### `mcpzip init`
Interactive setup wizard:
1. Asks which Claude Code MCP servers to import
2. Prompts for Gemini API key (or detects from GEMINI_API_KEY env var)
3. Generates config.json
4. Updates Claude Code config to point to mcpzip
5. Performs initial tool catalog fetch

### `mcpzip migrate`
Automatic migration:
1. Reads Claude Code MCP config (~/.claude.json and/or .mcp.json)
2. Moves server entries to mcpzip config
3. Replaces them with single mcpzip entry in Claude Code config
4. Fetches initial tool catalog

---

## LLM-Powered Search

### Backend
- Default: Gemini Flash (gemini-2.0-flash)
- Configurable: support multiple backends (Gemini, OpenAI, Anthropic, Ollama)
- API key resolution order: GEMINI_API_KEY env var -> config.json -> fallback to keyword search

### Search Implementation
- Full catalog prompt: send complete list of tool names + descriptions to Gemini with the user's query
- Gemini returns the top N matching tool names
- Works well for catalogs under ~500 tools

### Semantic Cache
- Cache key: normalized, lowercased query string
- Cache match: keyword overlap > 60% threshold between new query and cached queries
- Cache scope: per-session (cleared when proxy restarts)
- Cache hit: return cached results instantly (no Gemini call)

### Fallback
- If no Gemini API key: fall back to keyword/fuzzy matching on tool names and descriptions
- If Gemini call fails: fall back to keyword matching with error logged to stderr

---

## Technical Constraints

### Language and SDK
- Go with github.com/modelcontextprotocol/go-sdk (official, v1.4.0+)
- Gemini via Google AI Go SDK or REST API

### Transport
- Downstream (to Claude Code): stdio only
- Upstream (to real MCP servers): both stdio and HTTP

### Performance
- Startup: instant from disk cache (sub-100ms Go binary)
- search_tools latency: whatever Gemini takes (typically 500ms-1.5s)
- execute_tool latency: upstream server latency + proxy overhead (~1ms)
- Memory: ~18MB base + per-connection overhead

### Logging
- Stderr debug logging (visible in Claude Code's MCP server logs)
- Tool names and timing, no argument/result content by default

---

## Testing Requirements

### Acceptance Criteria (all three equally important)

1. **Tool call accuracy**: Given a natural language intent, does search_tools find the correct upstream tool? Measured across a test suite of queries.

2. **Proxy transparency**: Does execute_tool return exactly what the upstream server returns? No data loss, no corruption, no modification of responses.

3. **Token savings**: Does the proxy reduce context usage by the expected amount? Measurable: 3 meta-tool schemas vs N full tool schemas.

### Test Categories
- Unit tests: search algorithm, cache logic, config parsing, name prefixing
- Integration tests: full proxy loop (search -> execute) against mock upstream servers
- E2E tests: real upstream MCP servers with real tool calls
- Benchmark tests: token count comparison (with proxy vs without)

---

## Distribution

- `go install github.com/user/mcpzip@latest` - for Go developers
- GitHub Releases with prebuilt binaries (macOS arm64/amd64, Linux amd64) via goreleaser
- Homebrew tap for macOS users

---

## Out of Scope (v1)

- Web UI or dashboard
- Multi-user / multi-tenant support
- Authentication for downstream connections
- Rate limiting
- Tool result compression (only tool definitions are compressed)
- Custom compression formats (TOON, XML encoding)
- Embedding-based semantic search (may add later)
- Upstream server auto-discovery (user must configure explicitly)

---

## Open Questions (Resolved)

All questions from the specification interview have been resolved. Key tradeoffs accepted:
- LLM search adds cost (~$0.001/call via Gemini Flash) but provides best accuracy
- Full catalog prompt works for <500 tools; may need optimization if tool count grows significantly
- Keyword fallback ensures proxy works without API key, just with degraded search quality
