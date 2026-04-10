---
slug: benchmarks
title: "mcpzip Performance Deep Dive"
authors: [hypercall]
tags: [performance, benchmarks, engineering]
---

# mcpzip Performance Deep Dive

We built mcpzip to be fast, lightweight, and invisible. Here is a detailed breakdown of its performance characteristics -- startup time, search latency, memory footprint, and binary size.

<!-- truncate -->

## Startup Time: Instant

The most important performance metric for a CLI tool is startup time. mcpzip starts serving in **under 5 milliseconds**.

How? Disk caching. On startup, mcpzip:

1. Reads `~/.config/compressed-mcp-proxy/cache/tools.json` (typically 100KB-2MB)
2. Deserializes the tool catalog into memory
3. Starts the MCP server on stdio

No network connections. No process spawning. Just read a file and serve.

The background refresh runs concurrently -- it connects to all upstream servers in parallel, calls `tools/list` on each, and merges the results into the catalog. This takes 2-10 seconds depending on your servers, but it never blocks a request.

| Phase | Duration | Blocks Requests? |
|-------|----------|-----------------|
| Load disk cache | under 5ms | No (this IS serving) |
| Start MCP server | under 1ms | No |
| Background: connect to servers | 1-5s per server | No |
| Background: list tools | 0.5-2s per server | No |
| Background: merge + persist | under 100ms | No |

## Search Latency

mcpzip's search engine is the heart of the proxy. It needs to be fast because Claude calls it on nearly every tool interaction.

### Keyword Search: under 1ms

The keyword search engine tokenizes the query and scores it against tool metadata. It runs entirely in memory with no I/O.

| Operation | Time |
|-----------|------|
| Query tokenization | under 0.01ms |
| Score all tools (500) | under 0.5ms |
| Sort and take top N | under 0.1ms |
| **Total** | **under 1ms** |

### LLM Search: 200-500ms

When Gemini is configured, mcpzip sends the query and a compact tool catalog to the Gemini API. Latency depends on:
- Network round-trip to Gemini (~50-100ms)
- Gemini inference time (~100-300ms)
- Response parsing (~1ms)

### Query Cache: under 0.1ms

Repeated or similar queries hit the cache. Cache keys are normalized (lowercased, tokenized, sorted) so "slack send message" and "send message slack" are the same cache entry.

### Combined Flow

On a cache miss with Gemini enabled:
1. Keyword search runs immediately: under 1ms
2. LLM search runs in parallel: 200-500ms
3. Results are merged and cached: under 1ms

**Total: 200-500ms** (dominated by LLM network latency)

On a cache hit: **under 0.1ms**

## Memory Footprint

mcpzip is designed to be lightweight. It runs as a child process of Claude Code, so every megabyte matters.

| State | RSS |
|-------|-----|
| Startup (no cache) | ~10 MB |
| Idle (500 tools cached) | ~15 MB |
| Active (5 stdio connections) | ~20 MB* |
| Active (5 HTTP connections) | ~18 MB |
| Peak (catalog refresh, 10 servers) | ~25 MB |

*stdio connections spawn child processes (e.g., `npx @anthropic/slack-mcp`). Those processes have their own memory, typically 30-100MB each. The ~20MB figure is mcpzip itself, not including the child processes.

### Why So Small?

1. **No garbage collector** -- Rust uses compile-time memory management. No GC pauses, no overhead.
2. **No runtime** -- Rust compiles to native code. No JVM, no V8, no interpreter.
3. **Compact data structures** -- Tool entries use `String` and `serde_json::Value`, not heavyweight ORM objects.
4. **Lazy connections** -- Upstream connections are created on first use, not at startup.

## Binary Size: 5.8MB

| Version | Size |
|---------|------|
| mcpzip (Rust, release, stripped) | **5.8 MB** |
| mcpzip (Go, release, stripped) | 11 MB |
| Typical Node.js MCP server | 50-200 MB |
| mcp-remote (Node.js) | ~80 MB |

The Rust binary is statically linked with musl libc on Linux, meaning it runs on any Linux distribution without installing runtime dependencies. On macOS, it links against system libraries.

### What is in the binary?

- Tokio async runtime (~1.5MB)
- HTTP client (reqwest) (~1MB)
- JSON parser (serde_json) (~0.3MB)
- MCP protocol implementation (~0.5MB)
- CLI framework (clap) (~0.3MB)
- Business logic (~0.5MB)
- Other dependencies (~1.7MB)

## Connection Pooling

mcpzip maintains a connection pool with lazy initialization and idle reaping.

### Lazy Connects

Connections are created on first use. If you have 10 servers configured but only use 3, only 3 connections are created.

### Idle Timeout

Idle connections are closed after 5 minutes (configurable). This is important for stdio connections, which are full OS processes consuming resources.

### Concurrent Startup

During catalog refresh, all servers are connected in parallel with a 30-second per-server timeout. A slow server does not block the others.

### Reconnection

If a connection drops (process crashes, network error), it is automatically re-established on the next `execute_tool` call. No manual intervention needed.

## Tool Call Latency

The overhead mcpzip adds to a tool call is minimal:

| Phase | Time |
|-------|------|
| Parse prefixed name | under 0.01ms |
| Connection pool lookup | under 0.01ms |
| Argument serialization | under 0.1ms |
| **mcpzip overhead** | **under 0.15ms** |
| Upstream tool execution | varies (1ms - 30s) |
| Result deserialization | under 0.1ms |

The total overhead is under 0.3ms. Tool call latency is entirely dominated by the upstream server's execution time, not mcpzip.

## Summary

| Metric | Value |
|--------|-------|
| Time to first request | under 5ms |
| Keyword search latency | under 1ms |
| LLM search latency | 200-500ms |
| Cache hit latency | under 0.1ms |
| Tool call overhead | under 0.3ms |
| Memory (idle) | ~15MB |
| Memory (active) | ~20MB |
| Binary size | 5.8MB |
| Test count | 240+ |

mcpzip is designed to be invisible. It starts instantly, searches in milliseconds, and adds negligible overhead to tool calls. The only thing you notice is that your context window is 99% larger.

---

mcpzip is open source at [github.com/hypercall-public/mcpzip](https://github.com/hypercall-public/mcpzip). Built by [Hypercall](https://hypercall.xyz).
