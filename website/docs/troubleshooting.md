---
sidebar_position: 8
title: Troubleshooting
description: Common issues, solutions, and debugging techniques
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

2. Run with debug logging:
```bash
RUST_LOG=mcpzip=debug mcpzip serve
```

3. Look for connection errors. Common causes:
   - `command not found` -- the server's command is not in PATH
   - `connection refused` -- HTTP server is down
   - `timeout` -- server took too long to respond (>30s)

</details>

<details>
<summary><strong>Server fails to connect with "command not found"</strong></summary>

**Cause**: The server command is not installed or not in PATH.

**Solution**:

1. Verify the command exists: `which npx`
2. If using `npx`, make sure Node.js is installed
3. Use the full path in your config:
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

**Solution**:

1. Test the URL directly: `curl -v https://todoist.com/mcp`
2. Verify the URL in your config is the MCP endpoint, not the main website
3. Check your network/firewall settings

</details>

<details>
<summary><strong>Timeout errors during tool execution</strong></summary>

**Solution**: Increase the call timeout in your config:

```json
{
  "call_timeout_seconds": 300,
  "mcpServers": { ... }
}
```

Some tools (complex queries, large exports) can legitimately take several minutes.

</details>

---

## Search Issues

<details>
<summary><strong>Search returns irrelevant results</strong></summary>

**Solutions**:

1. **Add a Gemini API key** for semantic search: `export GEMINI_API_KEY=your-key`
2. **Use more specific queries**: "slack send message" instead of "help me communicate"
3. **Increase the result limit**:
```json
{
  "search": { "default_limit": 10 }
}
```

</details>

<details>
<summary><strong>Semantic search (Gemini) not working</strong></summary>

1. Check if the key is set: `echo $GEMINI_API_KEY`
2. Verify the key works:
```bash
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key=$GEMINI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"contents":[{"parts":[{"text":"hello"}]}]}'
```

3. Check debug logs: `RUST_LOG=mcpzip=debug mcpzip serve 2>&1 | grep -i gemini`

</details>

---

## OAuth Issues

<details>
<summary><strong>Browser does not open for OAuth</strong></summary>

1. Check the terminal output for the authorization URL
2. Copy the URL and open it manually
3. On headless systems, use API key auth via `headers` instead

</details>

<details>
<summary><strong>OAuth token expired / 401 after previously working</strong></summary>

Clear cached tokens and restart:

```bash
rm ~/.config/compressed-mcp-proxy/auth/*.json
```

</details>

---

## Configuration Issues

<details>
<summary><strong>"at least one MCP server must be defined"</strong></summary>

The `mcpServers` object in your config is empty. Add at least one server.

</details>

<details>
<summary><strong>"stdio server must have a command"</strong></summary>

A server without a `type` field defaults to stdio and requires a `command`. Either add a `command` or set `"type": "http"`.

</details>

<details>
<summary><strong>Config file not found</strong></summary>

Create it with the setup wizard:

```bash
mcpzip init
```

Or migrate from Claude Code:

```bash
mcpzip migrate
```

Or specify a custom path:

```bash
mcpzip serve --config /path/to/config.json
```

</details>

---

## Debug Logging

Control log verbosity with `RUST_LOG`:

```bash
# Debug logging
RUST_LOG=mcpzip=debug mcpzip serve

# Trace logging (very verbose)
RUST_LOG=mcpzip=trace mcpzip serve

# Log specific modules
RUST_LOG=mcpzip::transport=debug,mcpzip::search=debug mcpzip serve
```

| Module | What You See |
|--------|-------------|
| `mcpzip::transport` | Connection lifecycle, reconnection attempts |
| `mcpzip::search` | Search queries, scores, cache hits/misses |
| `mcpzip::catalog` | Catalog refresh, tool count changes |
| `mcpzip::auth` | OAuth flow steps, token refresh |
| `mcpzip::proxy` | Meta-tool invocations, argument parsing |
| `mcpzip::mcp` | Raw MCP protocol messages (trace level) |

:::tip
When filing a bug report, include the output of `RUST_LOG=mcpzip=debug mcpzip serve`. Redact any API keys or tokens.
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
