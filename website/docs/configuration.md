---
sidebar_position: 3
---

# Configuration

mcpzip is configured via a JSON file at `~/.config/compressed-mcp-proxy/config.json`.

## Full Example

```json
{
  "gemini_api_key": "AIza...",
  "search": {
    "model": "gemini-2.0-flash",
    "default_limit": 5
  },
  "idle_timeout_minutes": 5,
  "call_timeout_seconds": 120,
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
  }
}
```

## Top-Level Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `gemini_api_key` | string | `$GEMINI_API_KEY` | API key for Gemini-powered semantic search |
| `search.model` | string | `"gemini-2.0-flash"` | Gemini model to use for search |
| `search.default_limit` | integer | `5` | Default number of search results |
| `idle_timeout_minutes` | integer | `5` | Close idle upstream connections after this many minutes |
| `call_timeout_seconds` | integer | `120` | Default timeout for tool calls |
| `mcpServers` | object | required | Map of server name to server config |

## Server Config

Each entry in `mcpServers` describes an upstream MCP server.

### stdio (default)

Spawns a local process that speaks MCP over stdin/stdout.

```json
{
  "slack": {
    "command": "npx",
    "args": ["-y", "@anthropic/slack-mcp"],
    "env": {
      "SLACK_TOKEN": "xoxb-..."
    }
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `command` | string | Command to execute (required) |
| `args` | string[] | Command arguments |
| `env` | object | Environment variables |

### http

Connects to a remote MCP server via HTTP (Streamable HTTP). Supports OAuth 2.1 with PKCE.

```json
{
  "todoist": {
    "type": "http",
    "url": "https://todoist.com/mcp"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | `"http"` | Required |
| `url` | string | Server URL (required) |
| `headers` | object | Custom HTTP headers (skips OAuth if present) |

When no custom headers are set, mcpzip will attempt OAuth 2.1 authentication if the server requires it. It opens your browser for the auth flow and persists tokens to `~/.config/compressed-mcp-proxy/auth/`.

### sse

Legacy SSE transport for older MCP servers.

```json
{
  "legacy": {
    "type": "sse",
    "url": "https://example.com/mcp/sse"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | `"sse"` | Required |
| `url` | string | SSE endpoint URL (required) |

## CLI Flags

```bash
# Custom config path
mcpzip serve --config /path/to/config.json

# Migration
mcpzip migrate --dry-run
mcpzip migrate --claude-config ~/.claude.json --config output.json
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GEMINI_API_KEY` | Gemini API key (overrides config file) |
