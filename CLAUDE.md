# Compressed MCP Proxy

## Project Overview
An MCP proxy that aggregates multiple upstream MCP servers and exposes them via a Search + Execute pattern. Instead of loading hundreds of tool schemas into context, Claude uses 3 meta-tools (search_tools, describe_tool, execute_tool) to discover and invoke upstream tools on demand.

## Tech Stack
- Go with github.com/modelcontextprotocol/go-sdk (official MCP SDK)
- Gemini Flash for LLM-powered tool search (configurable backend)
- Single binary deployment via goreleaser

## Key Architecture
- Dual client/server proxy: Server (stdio downstream to Claude) + Client (stdio/HTTP upstream to real MCP servers)
- 3 meta-tools: search_tools, describe_tool (optional), execute_tool
- Disk-cached tool catalog with async background refresh
- Connection pool with idle timeout for upstream servers
- Full MCP proxy (tools via search pattern, resources + prompts forwarded directly)

## Commands
- `go build ./...` - Build
- `go test ./...` - Run tests
- `go run . serve` - Run proxy (stdio mode)
- `go run . init` - Interactive setup wizard
- `go run . migrate` - Auto-migrate from Claude Code config

## Project Structure
```
cmd/
  root.go         - CLI entry point (cobra)
  serve.go        - Proxy server command
  init.go         - Interactive setup wizard
  migrate.go      - Claude Code config migration
internal/
  proxy/          - Core proxy logic, upstream management
  search/         - LLM-powered tool search + semantic cache
  catalog/        - Tool catalog, disk caching, background refresh
  config/         - Configuration loading
  transport/      - Upstream connection management (stdio + HTTP)
```

## Config Location
~/.config/compressed-mcp-proxy/config.json
