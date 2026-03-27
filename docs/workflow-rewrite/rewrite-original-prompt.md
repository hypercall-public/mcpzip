# Original Prompt: rewrite

## Feature Request
**Feature Name**: Rust Rewrite
**Description**: Rewrite the entire mcpzip MCP proxy server in Rust. The Go implementation aggregates multiple upstream MCP servers and exposes them via 3 meta-tools (search_tools, describe_tool, execute_tool). The Rust rewrite should be a faithful port with equivalent functionality, targeting significantly lower memory footprint (5-10MB vs 50MB in Go).

## Timestamp
2026-03-02

## Context
The proxy currently runs as a Go binary using the go-sdk for MCP protocol handling. Key motivations for the Rust rewrite:
- Memory footprint: Go runtime overhead causes 25-50MB RSS for ~1MB of actual data
- The proxy runs as a long-lived sidecar process (one per Claude Code session), so memory matters
- Rust eliminates GC overhead, runtime scheduler, and goroutine machinery
- The codebase is well-defined (~1600 lines of Go) with clear boundaries, making it a good rewrite target

## Current Repository
https://github.com/protochainresearch/mcpzip
