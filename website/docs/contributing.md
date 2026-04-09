---
sidebar_position: 7
---

# Contributing

Contributions are welcome! Here's how to get started.

## Development Setup

```bash
git clone https://github.com/hypercall-public/mcpzip.git
cd mcpzip
cargo build
cargo test
```

## Running Tests

```bash
# Run all tests (150+)
cargo test

# Run a specific test module
cargo test proxy::handlers

# Run with output
cargo test -- --nocapture
```

## Project Structure

```
src/
  main.rs              # Entry point
  lib.rs               # Module declarations
  config.rs            # Config loading + validation
  error.rs             # Error types
  types.rs             # Core types (ToolEntry, ServerConfig, ProxyConfig)
  cli/                 # CLI commands (serve, init, migrate)
  auth/                # OAuth 2.1 + token persistence
  proxy/               # ProxyServer + meta-tool handlers
  catalog/             # Tool catalog + disk cache
  search/              # Keyword + LLM search
  transport/           # Connection pool + stdio/http/sse
  mcp/                 # MCP protocol types + server/client
```

## Guidelines

- Run `cargo test` before submitting a PR
- Follow existing code patterns and naming conventions
- Add tests for new functionality
- Keep the binary size small — be conservative with dependencies

## Reporting Issues

Open an issue at [github.com/hypercall-public/mcpzip/issues](https://github.com/hypercall-public/mcpzip/issues).

Include:
- mcpzip version (`mcpzip --version`)
- Your config (redact secrets)
- Steps to reproduce
- Expected vs actual behavior
