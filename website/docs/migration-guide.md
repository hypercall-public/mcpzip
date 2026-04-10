---
sidebar_position: 3.7
title: Migration Guide
description: Step-by-step guide to migrating from raw Claude Code config to mcpzip
---

# Migration Guide

Already have MCP servers configured in Claude Code? mcpzip can import them in one command.

## Overview

```mermaid
flowchart LR
    subgraph Before["Before Migration"]
        CC1[Claude Code] --> S1[Slack MCP]
        CC1 --> S2[GitHub MCP]
        CC1 --> S3[Todoist MCP]
        CC1 --> S4[Gmail MCP]
        CC1 --> S5[Linear MCP]
    end

    subgraph After["After Migration"]
        CC2[Claude Code] --> MZ[mcpzip]
        MZ --> S6[Slack MCP]
        MZ --> S7[GitHub MCP]
        MZ --> S8[Todoist MCP]
        MZ --> S9[Gmail MCP]
        MZ --> S10[Linear MCP]
    end

    style MZ fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style CC1 fill:#1a1a2e,stroke:#60A5FA,color:#60A5FA
    style CC2 fill:#1a1a2e,stroke:#60A5FA,color:#60A5FA
```

## Prerequisites

- mcpzip binary installed and in your PATH
- Existing Claude Code config with MCP servers

## Step 1: Preview the Migration

Always start with a dry run:

```bash
mcpzip migrate --dry-run
```

Example output:

```
Dry run: would migrate 5 server(s) from /Users/you/.claude.json

1. Write mcpzip config to /Users/you/.config/compressed-mcp-proxy/config.json:
   - slack (stdio)
   - github (stdio)
   - todoist (http)
   - gmail (http)
   - linear (stdio)

2. Update /Users/you/.claude.json:
   - Remove 5 individual server entries
   - Add single "mcpzip" entry pointing to /usr/local/bin/mcpzip
```

:::tip
Review this output carefully. Make sure all your servers are listed and the transport types look correct.
:::

## Step 2: Run the Migration

```bash
mcpzip migrate
```

This does two things:

### 2a. Creates mcpzip config

A new file is written at `~/.config/compressed-mcp-proxy/config.json` containing all your upstream servers:

```json title="~/.config/compressed-mcp-proxy/config.json"
{
  "mcpServers": {
    "slack": {
      "command": "npx",
      "args": ["-y", "@anthropic/slack-mcp"],
      "env": {
        "SLACK_TOKEN": "xoxb-your-token"
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
      "url": "https://gmail.mcp.run/sse"
    },
    "linear": {
      "command": "npx",
      "args": ["-y", "@anthropic/linear-mcp"],
      "env": {
        "LINEAR_API_KEY": "lin_api_..."
      }
    }
  }
}
```

### 2b. Updates Claude Code config

Your Claude Code config is updated to replace all individual servers with a single mcpzip entry:

```json title="~/.claude.json (after migration)"
{
  "mcpServers": {
    "mcpzip": {
      "type": "stdio",
      "command": "/usr/local/bin/mcpzip",
      "args": ["serve"]
    }
  }
}
```

## Step 3: Restart Claude Code

Restart Claude Code to pick up the new config. You should see 3 tools available:
- `search_tools`
- `describe_tool`
- `execute_tool`

## Step 4: Add Semantic Search (Optional)

For better tool discovery, add a Gemini API key:

```bash
export GEMINI_API_KEY=your-key-here
```

Or add it to your mcpzip config:

```json title="~/.config/compressed-mcp-proxy/config.json"
{
  "gemini_api_key": "AIza...",
  "mcpServers": { ... }
}
```

---

## What Gets Migrated

| Item | Migrated? | Notes |
|------|-----------|-------|
| Server names | Yes | Preserved exactly |
| `command` + `args` | Yes | For stdio servers |
| `env` variables | Yes | Including secrets |
| `type: "http"` + `url` | Yes | HTTP/SSE servers |
| `headers` | Yes | Custom auth headers |
| Non-MCP settings | No | Only `mcpServers` is touched |

:::warning Secrets in Config
Your environment variables (API keys, tokens) are copied to the mcpzip config file as-is. Make sure `~/.config/compressed-mcp-proxy/config.json` has appropriate permissions:

```bash
chmod 600 ~/.config/compressed-mcp-proxy/config.json
```
:::

## What Doesn't Get Migrated

- **Gemini API key** -- you'll need to add this separately
- **Search settings** -- defaults are used (model: `gemini-2.0-flash`, limit: 5)
- **Timeout settings** -- defaults are used (idle: 5 min, call: 120s)
- **Non-MCP Claude Code settings** -- untouched

---

## Migration Flow

```mermaid
flowchart TD
    START([Start]) --> FIND[Find Claude Code config]
    FIND --> CHECK{Config found?}
    CHECK -->|No| ERR1[Error: no config found]
    CHECK -->|Yes| READ[Read mcpServers]
    READ --> EMPTY{Any servers?}
    EMPTY -->|No| ERR2[Error: no MCP servers]
    EMPTY -->|Yes| DRY{--dry-run?}
    DRY -->|Yes| PREVIEW[Print preview]
    DRY -->|No| WRITE[Write mcpzip config]
    WRITE --> UPDATE[Update Claude Code config]
    UPDATE --> DONE([Done - restart Claude Code])

    style START fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style DONE fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style ERR1 fill:#1a1a2e,stroke:#F87171,color:#F87171
    style ERR2 fill:#1a1a2e,stroke:#F87171,color:#F87171
```

---

## Rollback

If you need to undo the migration:

### 1. Check if you have a backup

If you use version control for your dotfiles, restore from there.

### 2. Manual rollback

Re-add your servers to your Claude Code config:

```bash
# Open Claude Code config
$EDITOR ~/.claude.json
```

Paste your original `mcpServers` block back in, and remove the `mcpzip` entry.

### 3. Optional: Remove mcpzip config

```bash
rm ~/.config/compressed-mcp-proxy/config.json
rm -rf ~/.config/compressed-mcp-proxy/cache/
rm -rf ~/.config/compressed-mcp-proxy/auth/
```

:::danger Back Up First
Before running `mcpzip migrate`, consider backing up your Claude Code config:

```bash
cp ~/.claude.json ~/.claude.json.bak
```
:::

---

## Custom Paths

If your config files are in non-standard locations:

```bash
# Specify Claude Code config location
mcpzip migrate --claude-config /path/to/claude-config.json

# Specify output location
mcpzip migrate --config /path/to/mcpzip-config.json

# Both
mcpzip migrate \
  --claude-config /path/to/claude-config.json \
  --config /path/to/mcpzip-config.json
```
