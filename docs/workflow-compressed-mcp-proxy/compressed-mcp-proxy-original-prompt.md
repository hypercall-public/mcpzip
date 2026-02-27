# Original Prompt: compressed-mcp-proxy

## Feature Request
**Feature Name**: Compressed MCP Proxy
**Description**: An MCP server that proxies other MCP servers but presents tool definitions in a compressed, token-efficient format. MCP tools currently consume ~10% of context window (and up to 100% with multiple Google workspace servers enabled). This proxy would dramatically reduce that overhead while preserving full functionality.

## Timestamp
2026-02-27

## Context
The user observed that MCP tool definitions in Claude Code consume significant context:
- With current setup (Paper, Telegram, Slack, Notion, Linear, etc.): ~10% of 200k context
- With all 3 Google workspace MCP servers enabled: up to 100% of context
- Even after context compression, tools remain in the system prompt taking space
- The ToolSearch/deferred tools pattern helps but still requires verbose tool schemas

The goal is a proxy MCP server that:
1. Connects to upstream MCP servers as a client
2. Re-exposes their tools with compressed/minified schemas
3. Handles decompression/mapping when tools are actually called
4. Reduces the per-tool token overhead in Claude's context window
