# mcpzip

[![Go Reference](https://pkg.go.dev/badge/github.com/hypercall-public/mcpzip.svg)](https://pkg.go.dev/github.com/hypercall-public/mcpzip)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Aggregate hundreds of MCP tools behind 3 meta-tools.**

mcpzip is an MCP proxy that sits between Claude and your MCP servers. Instead of loading hundreds of tool schemas into the context window, it exposes just 3 tools:

| Tool | Purpose |
|------|---------|
| `search_tools` | Find tools by keyword or natural language query |
| `describe_tool` | Get the full schema for a specific tool |
| `execute_tool` | Run a tool on its upstream server |

> **Note:** This is the Go implementation. The project has since been [rewritten in Rust](https://github.com/hypercall-public/mcpzip) for better performance and smaller binary size.

## Install

```bash
go install github.com/hypercall-public/mcpzip/cmd/mcpzip@latest
```

## Quick Start

```bash
# Migrate from existing Claude Code config
mcpzip migrate

# Or run directly
mcpzip serve
```

## Configuration

Config lives at `~/.config/compressed-mcp-proxy/config.json`:

```json
{
  "mcpServers": {
    "slack": {
      "command": "npx",
      "args": ["-y", "@anthropic/slack-mcp"],
      "env": { "SLACK_TOKEN": "xoxb-..." }
    },
    "github": {
      "command": "gh-mcp"
    },
    "todoist": {
      "type": "http",
      "url": "https://todoist.com/mcp"
    }
  },
  "gemini_api_key": "...",
  "idle_timeout_minutes": 5
}
```

## Features

- **Context compression** — 3 tools instead of hundreds
- **Smart search** — keyword matching + optional Gemini semantic search
- **Instant startup** — serves from disk-cached tool catalog
- **All transports** — stdio and HTTP with OAuth 2.1
- **Auto-migration** — import Claude Code MCP config in one command

## Architecture

```
Claude ──stdio──> mcpzip ──stdio/http──> MCP Servers
                    │
              search_tools
              describe_tool
              execute_tool
```

## Built by

[Hypercall](https://hypercall.xyz)

## License

[MIT](LICENSE)
