---
sidebar_position: 1
slug: /
---

# Introduction

**mcpzip** is an MCP proxy that aggregates multiple upstream MCP servers and exposes them through a Search + Execute pattern. Instead of loading hundreds of tool schemas into your AI's context window, mcpzip gives the model just 3 meta-tools to discover and invoke any tool on demand.

## The Problem

The Model Context Protocol (MCP) lets AI assistants use external tools. But each MCP server you add dumps all its tool schemas into the context window. With 5+ servers, you're looking at hundreds of tool definitions loaded on every single message:

- **Context bloat**: Tool schemas eat up precious context tokens
- **Slower responses**: More context = higher latency
- **Model confusion**: Too many irrelevant tools degrade tool selection accuracy
- **Hard limits**: Some models cap the number of tools they can handle

## The Solution

mcpzip sits between your AI assistant and your MCP servers. It replaces all those tool definitions with exactly 3:

| Meta-Tool | What It Does |
|-----------|-------------|
| `search_tools` | Find tools by keyword or natural language |
| `describe_tool` | Get the full schema for a specific tool |
| `execute_tool` | Run a tool on its upstream server |

The model searches for what it needs, inspects the schema, and executes — loading only the tools it actually uses.

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

## Key Features

- **Context compression** — 3 tools instead of hundreds
- **Smart search** — keyword matching + optional Gemini semantic search
- **Instant startup** — serves from disk cache, refreshes in background
- **All transports** — stdio, HTTP (Streamable HTTP), SSE
- **OAuth 2.1** — browser-based PKCE flow, reuses mcp-remote tokens
- **Connection pooling** — idle timeout, automatic reconnection
- **Per-call timeout** — override defaults for slow tools
- **Auto-migration** — import Claude Code MCP config in one command
- **~5MB binary** — single static binary, no runtime dependencies

## Built by

[Hypercall](https://hypercall.xyz)
