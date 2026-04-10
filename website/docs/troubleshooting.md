---
sidebar_position: 8
title: Troubleshooting
description: Common issues, solutions, and debugging techniques for mcpzip
---

# Troubleshooting

Common issues and how to resolve them.

## Quick Diagnostics

Before diving into specific issues, run these checks:

```bash
# Check mcpzip is installed
mcpzip --version

# Check config is valid
cat ~/.config/compressed-mcp-proxy/config.json | python3 -m json.tool

# Check cache exists
ls -la ~/.config/compressed-mcp-proxy/cache/tools.json

# Check OAuth tokens
ls -la ~/.config/compressed-mcp-proxy/auth/

# Run with debug logging
RUST_LOG=mcpzip=debug mcpzip serve
```

---

## Connection Issues

<details>
<summary><strong>mcpzip starts but no tools are found</strong></summary>

**Symptoms**: `search_tools` returns empty results.

**Likely cause**: The tool catalog is empty (no cache and upstream servers failed to connect).

**Solution**:

1. Check if the cache file exists:
```bash
ls -la ~/.config/compressed-mcp-proxy/cache/tools.json
```

2. If it doesn't exist, mcpzip hasn't successfully connected to any server yet. Run with debug logging:
```bash
RUST_LOG=mcpzip=debug mcpzip serve
```

3. Look for connection errors in the output. Common causes:
   - `command not found` -- the server's command isn't in PATH
   - `connection refused` -- HTTP server is down
   - `timeout` -- server took too long to respond (>30s)

4. Test the upstream server directly:
```bash
# For stdio servers
echo '{"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{}},"id":1}' | npx -y @anthropic/slack-mcp
```

</details>

<details>
<summary><strong>Server fails to connect with "command not found"</strong></summary>

**Symptoms**: Debug logs show `command not found` for a stdio server.

**Cause**: The server command isn't installed or isn't in PATH.

**Solution**:

1. Verify the command exists:
```bash
which npx  # or whatever command your server uses
```

2. If using `npx`, make sure Node.js is installed:
```bash
node --version
npm --version
```

3. If the command is installed but not in PATH when running via Claude Code, add the full path:
```json
{
  "mcpServers": {
    "slack": {
      "command": "/usr/local/bin/npx",
      "args": ["-y", "@anthropic/slack-mcp"]
    }
  }
}
```

</details>

<details>
<summary><strong>HTTP server returns connection refused</strong></summary>

**Symptoms**: Debug logs show `connection refused` or `connection reset` for an HTTP server.

**Cause**: The remote server is down, the URL is wrong, or there's a network issue.

**Solution**:

1. Test the URL directly:
```bash
curl -v https://todoist.com/mcp
```

2. Check the URL in your config -- make sure it's the MCP endpoint, not the main website.

3. Check your network/firewall settings.

4. If the server requires OAuth, you might see a `401` before the connection is established -- this is normal, mcpzip will handle the OAuth flow.

</details>

<details>
<summary><strong>Timeout errors during tool execution</strong></summary>

**Symptoms**: `execute_tool` returns a timeout error.

**Cause**: The upstream server took longer than the call timeout (default: 120s).

**Solution**:

Increase the call timeout in your config:

```json
{
  "call_timeout_seconds": 300,
  "mcpServers": { ... }
}
```

Some tools (e.g., complex search queries, large data exports) can legitimately take several minutes.

</details>

---

## Search Issues

<details>
<summary><strong>Search returns irrelevant results</strong></summary>

**Symptoms**: `search_tools` returns tools that don't match the query.

**Cause**: Keyword search alone may not understand natural language queries well.

**Solution**:

1. **Add a Gemini API key** for semantic search:
```bash
export GEMINI_API_KEY=your-key
```

2. **Use more specific queries**:
   - Instead of "help me communicate" try "slack send message"
   - Include the server name if you know it: "todoist create task"

3. **Increase the result limit** so the right tool is more likely to appear:
```json
{
  "search": {
    "default_limit": 10
  }
}
```

</details>

<details>
<summary><strong>Semantic search (Gemini) not working</strong></summary>

**Symptoms**: Only keyword results are returned, no LLM-enhanced results.

**Cause**: Gemini API key is missing or invalid.

**Solution**:

1. Check if the key is set:
```bash
echo $GEMINI_API_KEY
```

2. Or check your config:
```bash
cat ~/.config/compressed-mcp-proxy/config.json | grep gemini
```

3. Verify the key works:
```bash
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key=$GEMINI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"contents":[{"parts":[{"text":"hello"}]}]}'
```

4. Check debug logs for Gemini-related errors:
```bash
RUST_LOG=mcpzip=debug mcpzip serve 2>&1 | grep -i gemini
```

</details>

---

## OAuth Issues

<details>
<summary><strong>Browser doesn't open for OAuth</strong></summary>

**Symptoms**: mcpzip says it's opening the browser but nothing happens.

**Solution**:

1. Check the terminal output for the authorization URL
2. Copy the URL and open it manually
3. On headless/remote systems, use API key auth via `headers` instead:

```json
{
  "mcpServers": {
    "server": {
      "type": "http",
      "url": "https://example.com/mcp",
      "headers": {
        "Authorization": "Bearer your-token"
      }
    }
  }
}
```

</details>

<details>
<summary><strong>OAuth token expired / 401 after working previously</strong></summary>

**Symptoms**: A server that was working starts returning 401 errors.

**Solution**:

1. Clear cached tokens:
```bash
rm ~/.config/compressed-mcp-proxy/auth/*.json
```

2. Restart mcpzip -- it will trigger a fresh OAuth flow

3. If the problem recurs quickly, the server might be issuing very short-lived tokens. Check with the server provider.

</details>

<details>
<summary><strong>"Invalid redirect_uri" during OAuth</strong></summary>

**Symptoms**: OAuth flow fails with an "invalid redirect_uri" error in the browser.

**Cause**: The OAuth application's registered callback URLs don't match mcpzip's callback server.

**Solution**: This is typically a server-side issue. Contact the MCP server provider. mcpzip uses a dynamic localhost port for the callback.

</details>

---

## Configuration Issues

<details>
<summary><strong>"at least one MCP server must be defined" error</strong></summary>

**Cause**: The `mcpServers` object in your config is empty.

**Solution**: Add at least one server:

```json
{
  "mcpServers": {
    "github": {
      "command": "gh-mcp"
    }
  }
}
```

</details>

<details>
<summary><strong>"stdio server must have a command" error</strong></summary>

**Cause**: A server without a `type` field (defaults to stdio) is missing the `command` field.

**Solution**: Either add a `command` or set the correct `type`:

```json
{
  "mcpServers": {
    "my-server": {
      "command": "my-mcp-server"
    }
  }
}
```

Or for HTTP servers:

```json
{
  "mcpServers": {
    "my-server": {
      "type": "http",
      "url": "https://example.com/mcp"
    }
  }
}
```

</details>

<details>
<summary><strong>Config file not found</strong></summary>

**Cause**: No config at the default path `~/.config/compressed-mcp-proxy/config.json`.

**Solution**:

1. Create the config directory and file:
```bash
mkdir -p ~/.config/compressed-mcp-proxy
```

2. Use the setup wizard:
```bash
mcpzip init
```

3. Or migrate from Claude Code:
```bash
mcpzip migrate
```

4. Or specify a custom path:
```bash
mcpzip serve --config /path/to/config.json
```

</details>

---

## Debug Logging

mcpzip uses the `RUST_LOG` environment variable for log control:

```bash
# Standard output (errors and status only)
mcpzip serve

# Debug logging (connection details, search queries, timings)
RUST_LOG=mcpzip=debug mcpzip serve

# Trace logging (full protocol messages, very verbose)
RUST_LOG=mcpzip=trace mcpzip serve

# Log specific modules
RUST_LOG=mcpzip::transport=debug,mcpzip::search=debug mcpzip serve
```

### What Debug Logging Shows

| Module | What You'll See |
|--------|----------------|
| `mcpzip::transport` | Connection lifecycle, reconnection attempts |
| `mcpzip::search` | Search queries, scores, cache hits/misses |
| `mcpzip::catalog` | Catalog refresh, tool count changes |
| `mcpzip::auth` | OAuth flow steps, token refresh |
| `mcpzip::proxy` | Meta-tool invocations, argument parsing |
| `mcpzip::mcp` | Raw MCP protocol messages (trace level) |

:::tip
When filing a bug report, include the output of `RUST_LOG=mcpzip=debug mcpzip serve` to help us diagnose the issue. Redact any API keys or tokens.
:::

---

## Still Stuck?

Open an issue at [github.com/hypercall-public/mcpzip/issues](https://github.com/hypercall-public/mcpzip/issues) with:

1. mcpzip version (`mcpzip --version`)
2. Your OS and architecture
3. Your config (redact secrets)
4. Debug log output
5. Steps to reproduce
6. Expected vs actual behavior
