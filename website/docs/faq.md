---
sidebar_position: 9
title: FAQ
description: Frequently asked questions about mcpzip
---

# FAQ

Frequently asked questions about mcpzip.

## General

<details>
<summary><strong>What is mcpzip?</strong></summary>

mcpzip is an MCP proxy that aggregates multiple upstream MCP servers and exposes them via a Search + Execute pattern. Instead of loading hundreds of tool schemas into Claude's context window, it gives the model just 3 meta-tools: `search_tools`, `describe_tool`, and `execute_tool`.

This reduces context window usage by 99%+ while maintaining full access to all your tools.

</details>

<details>
<summary><strong>What is MCP?</strong></summary>

The **Model Context Protocol (MCP)** is an open standard created by Anthropic that enables AI assistants to use external tools. It defines how AI clients (like Claude Code) communicate with tool servers.

Key concepts:
- **MCP Server** -- a program that exposes tools (functions the AI can call)
- **MCP Client** -- the AI application that connects to servers (e.g., Claude Code)
- **Tool** -- a function with a name, description, and JSON Schema parameters
- **JSON-RPC** -- the message format used for communication
- **`tools/list`** -- the method to discover available tools
- **`tools/call`** -- the method to invoke a specific tool

Learn more: [Model Context Protocol Specification](https://spec.modelcontextprotocol.io/)

</details>

<details>
<summary><strong>Does mcpzip work with ChatGPT / GPT-4?</strong></summary>

mcpzip implements the MCP protocol, which is currently supported by Claude Code and other MCP-compatible clients. It is not a ChatGPT plugin. However, any application that supports MCP can use mcpzip as a tool server.

</details>

<details>
<summary><strong>Is mcpzip open source?</strong></summary>

Yes. mcpzip is open source and available on [GitHub](https://github.com/hypercall-public/mcpzip).

</details>

<details>
<summary><strong>Who built mcpzip?</strong></summary>

mcpzip is built by [Hypercall](https://hypercall.xyz).

</details>

## Setup

<details>
<summary><strong>How do I install mcpzip?</strong></summary>

The recommended method is from source using Cargo:

```bash
cargo install --git https://github.com/hypercall-public/mcpzip
```

Or download a pre-built binary from [GitHub Releases](https://github.com/hypercall-public/mcpzip/releases).

See the [Getting Started](/docs/getting-started) guide for full instructions.

</details>

<details>
<summary><strong>Do I need to install Rust to use mcpzip?</strong></summary>

No. Pre-built binaries are available on the [Releases page](https://github.com/hypercall-public/mcpzip/releases). Download the binary for your platform and add it to your PATH.

If you want to build from source, you'll need the Rust toolchain (`rustup`).

</details>

<details>
<summary><strong>How do I add mcpzip to Claude Code?</strong></summary>

Add this to your `~/.claude.json`:

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

Then restart Claude Code. You should see 3 tools available: `search_tools`, `describe_tool`, and `execute_tool`.

</details>

<details>
<summary><strong>Can I keep some servers connected directly to Claude Code?</strong></summary>

Yes. mcpzip doesn't have to manage all your servers. You can have some connected directly and some through mcpzip. For example:

```json title="~/.claude.json"
{
  "mcpServers": {
    "mcpzip": {
      "command": "mcpzip",
      "args": ["serve"]
    },
    "my-custom-server": {
      "command": "my-server",
      "args": ["start"]
    }
  }
}
```

Servers connected directly to Claude will have their tools loaded normally. Only the servers in mcpzip's config benefit from context compression.

</details>

## How It Works

<details>
<summary><strong>Does mcpzip modify my tool calls?</strong></summary>

No. mcpzip passes tool arguments through to the upstream server **unchanged**. The only transformation is the tool name: `execute_tool("slack__send_message", args)` becomes `tools/call("send_message", args)` on the Slack server.

Arguments are forwarded exactly as-is. mcpzip does not validate, transform, or log the arguments.

</details>

<details>
<summary><strong>What happens if an upstream server is down?</strong></summary>

- **During startup**: mcpzip serves from its disk cache. The downed server's tools are still searchable and describable (from cache). `execute_tool` calls to that server will fail with a connection error.
- **During background refresh**: If a server fails to connect, its cached tools are preserved. Tools from servers that did connect are updated.
- **During a tool call**: mcpzip returns a clear error message. Other servers are unaffected.

</details>

<details>
<summary><strong>Does mcpzip cache tool results?</strong></summary>

mcpzip caches **search results** (query-to-results mapping) and **tool schemas** (the catalog). It does **not** cache tool execution results. Every `execute_tool` call goes to the upstream server in real-time.

</details>

<details>
<summary><strong>Can Claude skip the search step?</strong></summary>

Yes. If Claude already knows the tool name (e.g., from a previous search), it can call `execute_tool` directly without searching first. The search step is for discovery -- once you know the tool name, you can execute it directly.

</details>

<details>
<summary><strong>How does mcpzip handle tool name conflicts?</strong></summary>

Tools are namespaced by server using double underscore: `slack__send_message`, `telegram__send_message`. Even if two servers expose a tool with the same name, the prefixed names are unique.

</details>

## Semantic Search

<details>
<summary><strong>Do I need a Gemini API key?</strong></summary>

No. Gemini is optional. Without it, mcpzip uses keyword-based search, which works well for direct queries like "slack send message" or "todoist create task".

Gemini adds natural language understanding so you can search with phrases like "help me schedule a meeting" or "post something to the team". It costs ~200-500ms extra latency per search.

</details>

<details>
<summary><strong>Is my tool data sent to Gemini?</strong></summary>

When semantic search is used, mcpzip sends a **compact representation** of your tool catalog to Gemini along with the search query. This includes tool names, descriptions, and parameter summaries -- but **not** actual data, arguments, or execution results.

The compact format looks like:

```
slack__send_message: Send a Slack message [channel:string*, text:string*]
todoist__create_task: Create a new task [content:string*, project_id:string]
```

No credentials, tokens, or user data are sent to Gemini.

</details>

<details>
<summary><strong>Can I use a different LLM for search?</strong></summary>

Currently, mcpzip supports Gemini models for semantic search. The model is configurable:

```json
{
  "search": {
    "model": "gemini-2.0-flash"
  }
}
```

</details>

## Performance

<details>
<summary><strong>How much context does mcpzip save?</strong></summary>

With a typical setup of 10 servers and 50 tools per server (500 tools total):

- **Without mcpzip**: ~175,000 tokens (500 tools x ~350 tokens each)
- **With mcpzip**: ~1,200 tokens (3 meta-tools)
- **Savings**: 99.3%

Try the [interactive calculator](/docs/performance) for your specific setup.

</details>

<details>
<summary><strong>Does mcpzip add latency?</strong></summary>

The search step adds latency, but context compression reduces it. Net effect:

- **Search (keyword only)**: < 1ms added
- **Search (with Gemini)**: 200-500ms added
- **Context processing savings**: varies, but typically 100-500ms per message with large tool sets

For most users, the net latency change is **neutral or faster** because the model processes fewer tokens per message.

</details>

<details>
<summary><strong>How fast does mcpzip start?</strong></summary>

mcpzip starts serving in < 5 milliseconds by loading its disk cache. The background refresh to update tools from upstream servers takes 2-10 seconds but runs non-blocking. First-time startup (no cache) waits for at least one server to connect.

</details>

## Troubleshooting

<details>
<summary><strong>mcpzip hangs on startup</strong></summary>

mcpzip should never hang -- it serves from cache immediately. If it appears to hang:

1. Check the config is valid JSON: `python3 -m json.tool < ~/.config/compressed-mcp-proxy/config.json`
2. Try with debug logging: `RUST_LOG=mcpzip=debug mcpzip serve`
3. Ensure the config has at least one server defined

If the background refresh is slow, mcpzip is still serving. The "hang" might be a slow upstream server.

</details>

<details>
<summary><strong>Claude doesn't see mcpzip's tools</strong></summary>

1. Make sure mcpzip is in your PATH: `which mcpzip`
2. Check your Claude Code config has the mcpzip entry:
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
3. Restart Claude Code
4. Check for errors: `mcpzip serve` (run it manually to see errors)

</details>

<details>
<summary><strong>How do I update mcpzip?</strong></summary>

```bash
# From source
cargo install --git https://github.com/hypercall-public/mcpzip --force

# Or download the latest release binary
```

Your config, cache, and OAuth tokens are preserved across updates.

</details>

See the [Troubleshooting](/docs/troubleshooting) page for more detailed solutions.
