---
sidebar_position: 3.5
title: CLI Reference
description: Complete reference for all mcpzip commands, flags, and options
---

# CLI Reference

mcpzip provides three commands: `serve`, `init`, and `migrate`.

## `mcpzip serve`

Start the proxy server. This is the main command -- it starts mcpzip as an MCP server over stdio, ready for Claude Code to connect.

```bash
mcpzip serve [OPTIONS]
```

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--config <PATH>` | `~/.config/compressed-mcp-proxy/config.json` | Path to config file |

### Examples

```bash
# Start with default config
mcpzip serve

# Start with a custom config file
mcpzip serve --config /path/to/my-config.json
```

### Behavior

1. Loads configuration from disk
2. Loads cached tool catalog (instant -- no network required)
3. Starts serving MCP over stdio immediately
4. Spawns a background task to refresh the catalog by connecting to all upstream servers
5. Handles `Ctrl+C` for graceful shutdown

:::info Instant Startup
mcpzip serves from its disk cache on startup, so it is available immediately. The background refresh connects to upstream servers concurrently and merges any new or changed tools into the catalog. If a server fails to connect, its cached tools are preserved.
:::

### Logging

mcpzip logs to stderr (stdout is reserved for MCP protocol). Control log verbosity with `RUST_LOG`:

```bash
# Normal output (default)
mcpzip serve

# Debug logging
RUST_LOG=mcpzip=debug mcpzip serve

# Trace logging (very verbose)
RUST_LOG=mcpzip=trace mcpzip serve
```

---

## `mcpzip init`

Interactive setup wizard. Guides you through creating a config file.

```bash
mcpzip init
```

This command has no flags. It will:

1. Ask for your Gemini API key (optional, for semantic search)
2. Walk you through adding MCP servers
3. Write the config to `~/.config/compressed-mcp-proxy/config.json`

:::tip
If you already have MCP servers configured in Claude Code, use `mcpzip migrate` instead -- it is faster and preserves your existing setup.
:::

---

## `mcpzip migrate`

Automatically migrate your Claude Code MCP configuration to mcpzip.

```bash
mcpzip migrate [OPTIONS]
```

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--config <PATH>` | `~/.config/compressed-mcp-proxy/config.json` | Output config file path |
| `--claude-config <PATH>` | Auto-detected | Path to Claude Code config |
| `--dry-run` | `false` | Show what would happen without writing files |

### Claude Code Config Detection

If `--claude-config` is not specified, mcpzip searches these paths in order:

1. `~/.claude.json`
2. `~/.claude/config.json`

### Examples

```bash
# Preview what will happen
mcpzip migrate --dry-run

# Migrate with defaults
mcpzip migrate

# Custom paths
mcpzip migrate --claude-config ~/.claude.json --config ~/my-mcpzip.json
```

:::warning
`mcpzip migrate` modifies your Claude Code config file. Use `--dry-run` first to verify what will change. See the [Migration Guide](/docs/migration-guide) for a detailed walkthrough.
:::

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GEMINI_API_KEY` | Gemini API key for semantic search (overrides config file value) |
| `RUST_LOG` | Log level control (e.g., `mcpzip=debug`) |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (config invalid, server unreachable, etc.) |

---

## File Locations

| Path | Purpose |
|------|---------|
| `~/.config/compressed-mcp-proxy/config.json` | Main configuration |
| `~/.config/compressed-mcp-proxy/cache/tools.json` | Cached tool catalog |
| `~/.config/compressed-mcp-proxy/auth/` | Persisted OAuth tokens |
| `~/.claude.json` | Claude Code config (primary) |
| `~/.claude/config.json` | Claude Code config (fallback) |
