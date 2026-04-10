---
sidebar_position: 9.5
title: Glossary
description: Key terms and concepts used in mcpzip and the MCP ecosystem
---

# Glossary

Key terms and concepts used throughout the mcpzip documentation.

---

### Catalog

mcpzip's in-memory index of all tools from all upstream servers. The catalog is persisted to disk as a cache file and refreshed in the background on startup.

**File**: `~/.config/compressed-mcp-proxy/cache/tools.json`

---

### Claude Code

Anthropic's official CLI for Claude. Acts as an MCP client that connects to MCP servers (including mcpzip) to give Claude access to external tools.

---

### Compact Representation

A compressed format for tool schemas used in search results. Instead of the full JSON Schema, tools are represented as:

```
server__tool: description [param1:type*, param2:type]
```

Where `*` marks required parameters. This is ~7x smaller than full JSON schemas while preserving the information Claude needs to select tools.

---

### Connection Pool

mcpzip's system for managing upstream server connections. Features lazy initialization (connect on first use), idle timeout (disconnect after inactivity), and automatic reconnection.

---

### Context Window

The maximum amount of text (measured in tokens) that an AI model can process in a single conversation turn. Tool schemas consume context window space on every message. mcpzip reduces this by replacing all tool schemas with 3 meta-tools.

---

### Downstream

The direction from mcpzip to Claude Code. mcpzip acts as an MCP **server** on its downstream side, exposing the 3 meta-tools over stdio.

---

### execute_tool

One of mcpzip's 3 meta-tools. Routes a tool call to the correct upstream server. Takes a prefixed tool name and arguments, strips the prefix, and calls `tools/call` on the upstream server.

---

### Gemini

Google's LLM family, used by mcpzip for optional semantic search. The default model is `gemini-2.0-flash`. Requires a `GEMINI_API_KEY` to enable.

---

### Idle Timeout

The duration after which an inactive upstream connection is closed. Default: 5 minutes. Configurable via `idle_timeout_minutes`.

---

### JSON-RPC

The message protocol used by MCP. All MCP communication uses JSON-RPC 2.0 format with `method`, `params`, `id`, and `result`/`error` fields.

---

### Keyword Search

mcpzip's built-in search engine that tokenizes queries and scores them against tool metadata (names, descriptions, parameter names). Always active, runs in < 1ms.

---

### Manager

The internal component that manages the connection pool. Handles lazy connects, idle reaping, concurrent `list_tools` calls, and call routing.

---

### MCP (Model Context Protocol)

An open standard by Anthropic that defines how AI assistants communicate with external tool servers. Specifies `tools/list` for discovery and `tools/call` for invocation.

**Specification**: [spec.modelcontextprotocol.io](https://spec.modelcontextprotocol.io/)

---

### mcp-remote

The reference MCP OAuth client. mcpzip can reuse OAuth tokens previously obtained by mcp-remote.

---

### Meta-Tool

One of the 3 tools mcpzip exposes to Claude: `search_tools`, `describe_tool`, `execute_tool`. These replace hundreds of individual tool definitions.

---

### NDJSON

Newline-Delimited JSON. The wire format for stdio MCP communication. Each line is a complete JSON-RPC message.

---

### OAuth 2.1

The authorization framework used by mcpzip for HTTP MCP servers. Implements the Authorization Code flow with PKCE for security.

---

### PKCE (Proof Key for Code Exchange)

A security extension to OAuth that prevents authorization code interception attacks. mcpzip generates a random code verifier and sends its SHA-256 hash (code challenge) to the authorization server.

Pronounced "pixy".

---

### Prefixed Name

A tool name that includes the server name as a prefix, separated by double underscore: `slack__send_message`. Used to route `execute_tool` calls to the correct upstream server.

---

### ProxyServer

mcpzip's core component that implements the 3 meta-tools. Coordinates between the Catalog, Searcher, and Manager to handle search, describe, and execute requests.

---

### Query Cache

A cache of search results keyed by normalized queries. Queries are normalized by lowercasing, tokenizing, and sorting tokens, so "slack send message" and "send message slack" hit the same cache entry.

---

### search_tools

One of mcpzip's 3 meta-tools. Accepts a natural language query and returns matching tools from the catalog using keyword search and (optionally) LLM semantic search.

---

### Semantic Search

LLM-powered search that understands natural language intent. Sends the query and compact tool catalog to Gemini for ranking. Adds ~200-500ms latency but dramatically improves result quality for natural language queries.

---

### Streamable HTTP

The HTTP-based MCP transport where the client sends HTTP POST requests and receives SSE (Server-Sent Events) responses. Used for remote MCP servers.

---

### stdio

The default MCP transport. The client spawns a child process and communicates via NDJSON over stdin/stdout pipes. Used for local MCP servers.

---

### SSE (Server-Sent Events)

A legacy MCP transport where the client connects to an SSE endpoint for server-to-client messages and sends HTTP POST requests for client-to-server messages.

---

### Token (Authentication)

An OAuth access token or API key used to authenticate with a remote MCP server. Stored at `~/.config/compressed-mcp-proxy/auth/`.

---

### Token (LLM)

A unit of text in the AI model's vocabulary. Tool schemas consume tokens in the context window. mcpzip reduces tool token usage by 99%+.

---

### ToolEntry

The internal data structure representing a cached tool:

| Field | Description |
|-------|-------------|
| `name` | Prefixed name (e.g., `slack__send_message`) |
| `server_name` | Server name (e.g., `slack`) |
| `original_name` | Unprefixed name (e.g., `send_message`) |
| `description` | Tool description |
| `input_schema` | Full JSON Schema for parameters |
| `compact_params` | Compact parameter summary |

---

### Upstream

The direction from mcpzip to real MCP servers. mcpzip acts as an MCP **client** on its upstream side, connecting to servers via stdio, HTTP, or SSE.
