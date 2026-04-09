---
sidebar_position: 2
---

# Getting Started

## Installation

### From source (requires Rust toolchain)

```bash
cargo install --git https://github.com/hypercall-public/mcpzip
```

### From GitHub releases

Download the latest binary from [Releases](https://github.com/hypercall-public/mcpzip/releases) and place it in your PATH.

## Quick Setup

### Option 1: Migrate from Claude Code (recommended)

If you already have MCP servers configured in Claude Code, mcpzip can import them automatically:

```bash
# Preview what will happen
mcpzip migrate --dry-run

# Do the migration
mcpzip migrate
```

This will:
1. Read your Claude Code config (`~/.claude.json` or `~/.claude/config.json`)
2. Create a mcpzip config at `~/.config/compressed-mcp-proxy/config.json` with all your servers
3. Replace the individual server entries in Claude Code with a single `mcpzip` entry

### Option 2: Interactive setup

```bash
mcpzip init
```

### Option 3: Manual configuration

Create `~/.config/compressed-mcp-proxy/config.json`:

```json
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
    }
  }
}
```

Then add mcpzip to your Claude Code config (`~/.claude.json`):

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

## Verify It Works

Restart Claude Code. You should see mcpzip's 3 tools available:
- `search_tools`
- `describe_tool`
- `execute_tool`

Try asking Claude to search for a tool:

> "Search for tools related to sending messages"

Claude will use `search_tools` to find matching tools across all your configured servers.

## Optional: Enable Semantic Search

By default, mcpzip uses keyword-based search. For better natural language understanding, add a Gemini API key:

```bash
export GEMINI_API_KEY=your-key
```

Or add it to your config:

```json
{
  "gemini_api_key": "your-key",
  "mcpServers": { ... }
}
```

This enables Gemini-powered semantic search that understands queries like "help me manage my calendar" or "find something to track tasks".
