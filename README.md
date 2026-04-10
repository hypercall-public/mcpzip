<p align="center">
  <img src="website/static/img/logo.svg" alt="mcpzip logo" width="120" />
</p>

<h1 align="center">mcpzip</h1>

<p align="center">
  <strong>Aggregate hundreds of MCP tools behind 3 meta-tools.</strong><br>
  Search, describe, execute &mdash; without blowing up your context window.
</p>

<p align="center">
  <a href="https://pkg.go.dev/github.com/hypercall-public/mcpzip"><img src="https://pkg.go.dev/badge/github.com/hypercall-public/mcpzip.svg" alt="Go Reference"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="https://hypercall-public.github.io/mcpzip">Documentation</a> &bull;
  <a href="https://github.com/hypercall-public/mcpzip/releases">Releases</a> &bull;
  <a href="https://github.com/hypercall-public/mcpzip/issues">Issues</a>
</p>

---

## The Problem

Every MCP server you add to Claude Code dumps its full tool schemas into the context window. 5 servers with 50 tools each = 250 tool definitions loaded on every message. Your context fills up, latency increases, and the model gets confused by irrelevant tools.

## The Solution

**mcpzip** sits between Claude and your MCP servers. It exposes just 3 tools:

| Tool | Purpose |
|------|---------|
| `search_tools` | Find tools by keyword or natural language query |
| `describe_tool` | Get the full schema for a specific tool |
| `execute_tool` | Run a tool on its upstream server |

Claude searches for what it needs, gets the schema, and executes &mdash; all without loading hundreds of tool definitions into context.

## How It Works

```
Claude Code                mcpzip                    MCP Servers
    |                        |                           |
    |-- search_tools ------->|                           |
    |   "send a message"     |-- (keyword + LLM search)  |
    |<-- results ------------|                           |
    |   slack__send_message   |                           |
    |   telegram__send_msg    |                           |
    |                        |                           |
    |-- describe_tool ------>|                           |
    |   slack__send_message   |                           |
    |<-- full schema --------|                           |
    |                        |                           |
    |-- execute_tool ------->|                           |
    |   slack__send_message   |-- tools/call ----------->|
    |   {channel, text}      |<-- result ----------------|
    |<-- result -------------|                           |
```

## Features

- **Context compression**: 3 tools instead of hundreds
- **Smart search**: Keyword matching + optional Gemini-powered semantic search
- **Instant startup**: Serves from disk-cached tool catalog, refreshes in background
- **All transports**: stdio, HTTP (Streamable HTTP), SSE
- **OAuth 2.1**: Browser-based PKCE flow, reuses mcp-remote tokens
- **Connection pooling**: Idle timeout, automatic reconnection
- **Per-call timeout**: Override the default timeout for slow tools
- **Auto-migration**: Import your existing Claude Code MCP config in one command
- **~5MB binary**: Single static binary, no runtime dependencies

## Quick Start

### Install from source

```bash
cargo install --git https://github.com/hypercall-public/mcpzip
```

### Migrate your existing Claude Code config

```bash
mcpzip migrate
```

This reads your `~/.claude.json` or `~/.claude/config.json`, creates a mcpzip config with all your servers, and replaces the individual entries with a single `mcpzip` entry.

### Or set up manually

```bash
mcpzip init
```

### Configure Claude Code

Add mcpzip to your Claude Code config (`~/.claude.json`):

```json
{
  "mcpServers": {
    "mcpzip": {
      "command": "mcpzip",
      "args": ["serve"]
    }
  }
}
```

## Configuration

mcpzip config lives at `~/.config/compressed-mcp-proxy/config.json`:

```json
{
  "mcpServers": {
    "slack": {
      "command": "npx",
      "args": ["-y", "@anthropic/slack-mcp"],
      "env": {
        "SLACK_TOKEN": "xoxb-..."
      }
    },
    "github": {
      "command": "gh-mcp"
    },
    "todoist": {
      "type": "http",
      "url": "https://todoist.com/mcp"
    },
    "gmail": {
      "type": "http",
      "url": "https://gmail.mcp.run/sse",
      "headers": {
        "Authorization": "Bearer ..."
      }
    }
  },
  "gemini_api_key": "...",
  "search": {
    "model": "gemini-2.0-flash",
    "default_limit": 5
  },
  "idle_timeout_minutes": 5,
  "call_timeout_seconds": 120
}
```

### Server Types

| Type | Config | Description |
|------|--------|-------------|
| `stdio` (default) | `command`, `args`, `env` | Spawns a local process |
| `http` | `url`, `headers` | MCP Streamable HTTP with OAuth |
| `sse` | `url` | Legacy SSE transport |

### Search

Without a Gemini API key, mcpzip uses keyword search (tokenization + scoring). With a key, it adds LLM-powered semantic search that understands natural language queries like "send someone a message on slack".

Set the key via environment variable or config:

```bash
export GEMINI_API_KEY=your-key
```

## Architecture

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

### Key Components

- **Catalog**: Maintains a cached index of all tools from all upstream servers. Persists to disk. Background refresh on startup.
- **Searcher**: Two-tier search &mdash; fast keyword scoring, optional Gemini semantic reranking. Query results are cached.
- **Manager**: Connection pool for upstream servers. Lazy connection with idle timeout. Concurrent `list_tools` across all servers.
- **MCP Server**: NDJSON-over-stdio server implementing the MCP protocol. Handles `initialize`, `tools/list`, and `tools/call`.

## Development

```bash
# Build
cargo build

# Run tests (150+ tests)
cargo test

# Run the proxy
cargo run -- serve

# Run with custom config
cargo run -- serve --config path/to/config.json

# Dry-run migration
cargo run -- migrate --dry-run
```

## Built by

[Hypercall](https://hypercall.xyz)

## License

[MIT](LICENSE)
