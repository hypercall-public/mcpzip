---
sidebar_position: 6
---

# Transports

mcpzip supports three transport types for connecting to upstream MCP servers.

## stdio

The default transport. Spawns a local process and communicates via NDJSON over stdin/stdout.

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

- Process is spawned on first use (lazy connection)
- Automatically restarted if the process exits
- Environment variables are passed to the child process
- The process is killed on shutdown

## HTTP (Streamable HTTP)

Connects to remote MCP servers using the Streamable HTTP transport. Responses are parsed as SSE event streams.

```json
{
  "todoist": {
    "type": "http",
    "url": "https://todoist.com/mcp"
  }
}
```

### OAuth 2.1

When no custom headers are configured, mcpzip automatically handles OAuth authentication:

1. Sends an initial request to the server
2. If it gets a 401 with a `WWW-Authenticate` header containing resource metadata, starts the OAuth flow
3. Discovers authorization and token endpoints from the resource's well-known metadata
4. Opens your browser for authorization (Authorization Code flow with PKCE)
5. Runs a local callback server to receive the auth code
6. Exchanges the code for tokens and persists them to disk

Tokens are cached at `~/.config/compressed-mcp-proxy/auth/{hash}.json` where the hash is derived from the server URL. mcpzip also checks for tokens previously saved by `mcp-remote`.

### Custom Headers

If you set `headers`, OAuth is skipped and the headers are sent with every request. Useful for API key authentication:

```json
{
  "gmail": {
    "type": "http",
    "url": "https://gmail.mcp.run/sse",
    "headers": {
      "Authorization": "Bearer your-api-key"
    }
  }
}
```

## SSE (Legacy)

For older MCP servers that use the Server-Sent Events transport.

```json
{
  "legacy-server": {
    "type": "sse",
    "url": "https://example.com/mcp/sse"
  }
}
```

## Connection Lifecycle

All transports share the same lifecycle managed by the connection pool:

1. **Lazy connect** — connections are established on first use
2. **Idle timeout** — connections are closed after `idle_timeout_minutes` (default: 5)
3. **Reconnect** — if a connection drops, it's re-established on the next call
4. **Concurrent startup** — during catalog refresh, all servers are connected concurrently with a 30-second per-server timeout
5. **Graceful shutdown** — all connections are closed when mcpzip exits
