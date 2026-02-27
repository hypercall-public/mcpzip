# Implementation Plan: mcpzip

## Architecture Reference
See docs/workflow-compressed-mcp-proxy/plans/compressed-mcp-proxy-architecture-plan.md

## Plan Review Changes (Phase 6)
- **Search**: Keyword/TF-IDF is now the primary search mechanism. LLM search (Gemini) is an optional enhancement, not the default.
- **Catalog/Transport unification**: Catalog uses Transport Manager for upstream connections (no separate temporary connections). Catalog depends on Transport - they cannot be built in parallel.
- **Migration safety**: migrate command auto-detects Claude Code config path and creates a timestamped backup before modifying.
- **Admin tools**: proxy_status and proxy_refresh are handled as special cases in execute_tool, NOT registered as MCP tools (avoids needing to filter tools/list).
- **Concurrent stdio calls**: Add per-connection mutex for serializing calls to the same stdio upstream.
- **Double-underscore separator**: Split on FIRST occurrence of "__". Document that upstream tool names containing "__" are supported (parsed as part of tool name, not server name).
- **Dry-run mode**: Add `mcpzip status` command that reports tool counts and estimated token savings without serving.
- **Gemini rate limiting**: Debounce rapid LLM search calls (max 1 per 500ms).

## Implementation Tasks

### Foundation Tasks (Sequential)

#### Task F1: Go Module and Entry Point
- **From Architecture**: Foundation
- **Files to create**:
  - `go.mod` - module `github.com/jake/mcpzip`
  - `cmd/mcpzip/main.go` - entry point, delegates to CLI root command
- **Implementation details**:
  - Initialize Go module with required dependencies (go-sdk, cobra, genai)
  - Main function calls `cli.Execute()`
  - Verify `go build` succeeds with empty root command
- **Tests**: Build verification only (no logic to test)

#### Task F2: Shared Types
- **From Architecture**: Shared Types
- **Files to create**: `internal/types/types.go`
- **Implementation details**:
  - `ToolEntry` struct: Name, ServerName, OriginalName, Description, InputSchema (json.RawMessage), CompactParams string
  - `SearchResult` struct: Name, Description, CompactParams
  - `ServerConfig` struct: Type, Command, Args, Env, URL (matches Claude Code JSON format)
  - `ProxyConfig` struct: GeminiAPIKey, Search (SearchConfig), IdleTimeoutMinutes, MCPServers map
  - `SearchConfig` struct: DefaultLimit, Model
  - `ServerStatus` struct: Name, Connected, ToolCount, LastRefresh, Error
  - Helper: `PrefixedName(server, tool string) string` - returns "server__tool"
  - Helper: `ParsePrefixedName(name string) (server, tool string, err error)` - splits on first "__"
  - Helper: `CompactParamsFromSchema(schema json.RawMessage) string` - generates "param1:type*, param2:type" format
- **Tests**:
  - PrefixedName produces correct format
  - ParsePrefixedName splits correctly, errors on no separator
  - CompactParamsFromSchema handles required/optional params, various types

#### Task F3: Config Package
- **From Architecture**: Config Package
- **Files to create**: `internal/config/config.go`
- **Implementation details**:
  - `Load(path string) (*types.ProxyConfig, error)` - read JSON, unmarshal, validate
  - `DefaultPath() string` - `~/.config/compressed-mcp-proxy/config.json`
  - `CachePath() string` - `~/.config/compressed-mcp-proxy/cache/tools.json`
  - Validation: at least one server defined, valid server types ("stdio" or "http"), stdio servers have command, http servers have URL
  - `LoadClaudeCodeConfig() (*ClaudeCodeConfig, error)` - parse ~/.claude.json mcpServers section
  - `ClaudeCodeConfig` struct for migration
- **Tests**:
  - Load valid config
  - Load config with missing file -> error
  - Load config with invalid JSON -> error
  - Validation: no servers -> error, invalid type -> error
  - DefaultPath expands ~ correctly
  - LoadClaudeCodeConfig parses real format

### Component Tasks (Parallel)

#### Task C1: Catalog Manager
- **From Architecture**: Component 1 - Catalog Manager
- **Files to create**:
  - `internal/catalog/catalog.go` - Catalog struct, AllTools, GetTool, ServerStatus
  - `internal/catalog/cache.go` - disk cache read/write
  - `internal/catalog/fetch.go` - connect to upstream, fetch tools via MCP client
- **Interface**:
  - Input: ProxyConfig
  - Output: []ToolEntry (all tools, prefixed)
- **Implementation details**:
  - `Catalog` struct holds `sync.RWMutex`-protected `[]ToolEntry` and `map[string][]ToolEntry` (by server)
  - `Load()` reads from disk cache, populates in-memory catalog
  - `RefreshAll(ctx)` spawns a goroutine per server, each with its own timeout (30s default), collects results, updates cache. Errors per-server don't fail the whole refresh.
  - `RefreshServer(ctx, name)` refreshes a single server
  - `SaveCache()` writes current catalog to disk as JSON
  - `fetch.go`: for each server config, use Transport Manager to get a connection, call `tools/list` (handle pagination via nextCursor). Connection stays in pool for later execution use. Catalog depends on Transport Manager via an `UpstreamLister` interface for testability.
  - Each fetched tool is converted to `ToolEntry` with `PrefixedName`, `CompactParams` computed
  - Background refresh: `StartBackgroundRefresh(ctx, interval)` runs periodic refresh in a goroutine
- **Tests**:
  - Load from valid cache file
  - Load from missing cache -> empty catalog, no error
  - AllTools returns prefixed names
  - GetTool finds by prefixed name, errors on unknown
  - RefreshAll with mock upstream (using in-memory MCP server)
  - RefreshAll handles one server timeout without affecting others
  - SaveCache writes valid JSON, LoadCache reads it back
  - CompactParams generation from real schemas
- **Estimated complexity**: M
- **Can run in parallel with**: C2, C3, C4

#### Task C2: Search Engine
- **From Architecture**: Component 2 - Search Engine
- **Files to create**:
  - `internal/search/search.go` - Searcher interface, OrchestratedSearcher (cache -> LLM -> keyword)
  - `internal/search/llm.go` - GeminiSearcher, builds prompt with full catalog, parses response
  - `internal/search/keyword.go` - KeywordSearcher, lowercased token matching on name + description
  - `internal/search/cache.go` - QueryCache, keyword overlap matching
- **Interface**:
  - Input: query string, limit int
  - Output: []SearchResult
  - Catalog provided via `func() []ToolEntry` callback
- **Implementation details**:
  - `Searcher` interface with `Search(ctx, query, limit) ([]SearchResult, error)`
  - `OrchestratedSearcher`: keyword search is primary. If LLM is configured, optionally re-ranks keyword results via LLM for better accuracy. Cache applies to LLM re-ranking only.
  - `GeminiSearcher`:
    - Builds prompt: "Given these tools: [name: description] for each tool, match to query: X. Return top N tool names as JSON array."
    - Calls Gemini Flash via google genai SDK
    - Parses JSON array response, maps to SearchResults
  - `KeywordSearcher`:
    - Tokenize query into words
    - Score each tool: count matching tokens in name + description (case-insensitive)
    - Return top N by score
  - `QueryCache`:
    - Store: map[string][]SearchResult (normalized query -> results)
    - Match: tokenize new query, check overlap with each cached query key
    - Overlap threshold: 60% of new query tokens found in cached query tokens
    - Thread-safe with sync.RWMutex
  - Constructor: `NewSearcher(apiKey, model string, catalogFn func() []types.ToolEntry) Searcher`
    - If apiKey is empty, returns KeywordSearcher directly (no LLM, no orchestration)
    - If apiKey is present, returns OrchestratedSearcher wrapping all three
- **Tests**:
  - KeywordSearcher: exact match, partial match, no match, ranking by relevance
  - QueryCache: store and retrieve, keyword overlap matching, miss on low overlap
  - GeminiSearcher: mock HTTP response from Gemini, parse results correctly
  - OrchestratedSearcher: cache hit skips LLM, LLM failure falls back to keyword
  - Empty catalog returns empty results
  - Limit parameter respected
- **Estimated complexity**: L
- **Can run in parallel with**: C1, C3, C4

#### Task C3: Transport Manager
- **From Architecture**: Component 3 - Transport Manager
- **Files to create**:
  - `internal/transport/upstream.go` - Upstream interface
  - `internal/transport/manager.go` - Manager struct, connection pool, idle reaping
  - `internal/transport/stdio.go` - StdioUpstream, spawn child process MCP client
  - `internal/transport/http.go` - HTTPUpstream, connect to HTTP MCP endpoint
- **Interface**:
  - Input: ServerConfig (how to connect)
  - Output: Upstream interface (CallTool, ListResources, ReadResource, ListPrompts, GetPrompt, Close)
- **Implementation details**:
  - `Upstream` interface as defined in architecture
  - `Manager` struct:
    - `pool map[string]*poolEntry` (server name -> connection + last used time)
    - `sync.RWMutex` for pool access
    - `GetConnection(ctx, serverName)` - if pooled and alive, return it; else create new
    - `CallTool(ctx, serverName, toolName, args)` - get connection, call tool
    - Same pattern for ListResources, ReadResource, ListPrompts, GetPrompt
    - `startReaper(interval)` - goroutine that checks pool every minute, closes connections idle > IdleTimeoutMinutes
    - `Close()` - close all connections, stop reaper
  - `StdioUpstream`:
    - Spawns child process using `exec.Cmd` with server's command/args/env
    - Creates go-sdk Client over stdio transport
    - Calls `client.Initialize()` during creation
    - `Close()` kills child process
  - `HTTPUpstream`:
    - Creates go-sdk Client with StreamableHTTP transport to server URL
    - Calls `client.Initialize()` during creation
    - `Close()` disconnects
  - Retry logic: on connection failure, attempt up to 3 retries with 1s, 2s, 4s backoff
- **Tests**:
  - Manager: GetConnection creates new, GetConnection reuses existing
  - Manager: idle reaper closes old connections
  - Manager: Close tears down everything
  - StdioUpstream: spawn mock server, call tool, get result
  - HTTPUpstream: connect to mock HTTP server, call tool, get result
  - Retry: connection failure triggers retry, succeeds on retry
  - Retry: max retries exceeded returns error
- **Estimated complexity**: L
- **Can run in parallel with**: C1, C2, C4

#### Task C4: CLI Commands
- **From Architecture**: Component 4 - CLI
- **Files to create**:
  - `internal/cli/root.go` - root cobra command with version flag
  - `internal/cli/serve.go` - serve subcommand (wires components, starts proxy)
  - `internal/cli/init.go` - init wizard (interactive setup)
  - `internal/cli/migrate.go` - migrate command (auto-import from Claude Code)
- **Interface**:
  - `Execute() error` called from main
  - `serve`: loads config, creates catalog/search/transport/proxy, connects stdio
  - `init`: prompts user, generates config, optionally runs migrate
  - `migrate`: reads Claude Code config, generates proxy config, updates Claude Code config
- **Implementation details**:
  - `root.go`: cobra root cmd "mcpzip", version subcommand
  - `serve.go`:
    - Load config (--config flag or DefaultPath)
    - Create Catalog, load from cache
    - Create Searcher (with Gemini key resolution: env -> config -> keyword fallback)
    - Create Transport Manager
    - Create Proxy Server (wires all components)
    - Start background catalog refresh
    - Connect proxy to stdio transport
    - Wait for stdin close, then cleanup
  - `init.go`:
    - Detect existing Claude Code config
    - List found MCP servers, ask which to import (or all)
    - Ask for Gemini API key (or detect from env)
    - Generate config.json at DefaultPath
    - Optionally run migrate to update Claude Code config
    - Run initial catalog fetch
  - `migrate.go`:
    - Read Claude Code config (--claude-config flag or auto-detect)
    - Copy server entries to proxy config
    - Remove original entries from Claude Code config
    - Add single mcpzip entry to Claude Code config
    - Write both configs
    - Print summary of what changed
- **Tests**:
  - serve: verify component wiring (unit test with mocks)
  - migrate: read test Claude Code config, verify proxy config output
  - migrate: verify Claude Code config updated correctly
  - init: difficult to unit test (interactive); test config generation logic separately
- **Estimated complexity**: M
- **Can run in parallel with**: C1, C2, C3

### Integration Tasks (Sequential, after components)

#### Task I1: Proxy Server - Meta-Tool Handlers
- **From Architecture**: Integration Layer - Proxy Server
- **Files to create**:
  - `internal/proxy/server.go` - NewProxyServer, registers 3 meta-tools + resources/prompts
  - `internal/proxy/handlers.go` - handleSearchTools, handleDescribeTool, handleExecuteTool
- **Implementation details**:
  - `ProxyServer` struct holds Catalog, Searcher, Transport Manager
  - Register tool "search_tools":
    - inputSchema: {query: string (required), limit: integer (optional)}
    - handler: calls Searcher.Search, formats results as text content
  - Register tool "describe_tool":
    - inputSchema: {name: string (required)}
    - handler: calls Catalog.GetTool, returns full InputSchema as text content
  - Register tool "execute_tool":
    - inputSchema: {name: string (required), arguments: object (required)}
    - handler: parse prefix, call Transport.CallTool, return upstream result as-is
  - Admin tools (proxy_status, proxy_refresh) registered as regular tools but NOT returned in tools/list. Only discoverable via search_tools (include them in the catalog that search indexes).
- **Tests**:
  - search_tools handler: returns formatted results
  - describe_tool handler: returns full schema for valid tool, error for unknown
  - execute_tool handler: forwards to correct upstream, returns response unchanged
  - execute_tool handler: invalid prefix -> clear error
  - Admin tools discoverable via search but not in tools/list

#### Task I2: Proxy Server - Resource/Prompt Forwarding
- **From Architecture**: Integration Layer - Resources
- **Files to create**:
  - `internal/proxy/resources.go` - aggregated resource and prompt forwarding
- **Implementation details**:
  - On `resources/list`: aggregate from all connected upstreams, prefix URIs with server name
  - On `resources/read`: parse server from prefixed URI, forward to correct upstream
  - On `prompts/list`: aggregate from all upstreams, prefix names
  - On `prompts/get`: parse server from prefixed name, forward to correct upstream
  - Handle server failures gracefully (skip failed servers, return partial results)
- **Tests**:
  - Aggregate resources from 2 mock upstreams
  - Read resource routes to correct upstream
  - Failed upstream excluded from list, no crash

#### Task I3: Proxy Server - Instructions Summary
- **From Architecture**: Integration Layer - Instructions
- **Files to create**:
  - `internal/proxy/instructions.go` - collect and summarize upstream instructions
- **Implementation details**:
  - During catalog refresh, also collect `instructions` from each upstream's initialize response
  - If Gemini available: send all instructions to Gemini with "summarize these MCP server instructions into one concise paragraph" prompt
  - If no Gemini: concatenate with server name headers
  - Return summarized instructions in proxy's initialize response
- **Tests**:
  - Summarize with mock Gemini response
  - Fallback concatenation without Gemini
  - Handle empty instructions from some servers

#### Task I4: Full Integration Wiring
- **From Architecture**: Integration Layer
- **Files to modify**: `internal/cli/serve.go` (wire proxy server creation)
- **Implementation details**:
  - Wire the serve command to create ProxyServer with real components
  - Verify full startup -> serve -> shutdown lifecycle
  - Test with a real mock upstream MCP server
- **Integration tests**:
  - Full loop: start proxy, search for tool, execute tool, verify result
  - Multiple upstream servers aggregated correctly
  - Upstream failure during serve -> graceful degradation
  - Catalog refresh doesn't disrupt active serving

## Build Order

1. **Foundation** (must complete first):
   - Task F1: Go module and entry point
   - Task F2: Shared types
   - Task F3: Config package

2. **Parallel Group A** (can implement simultaneously):
   - Task C2: Search Engine (L)
   - Task C3: Transport Manager (L)
   - Task C4: CLI Commands (M)

3. **Sequential after Transport** (C3 must complete first):
   - Task C1: Catalog Manager (M) - depends on Transport Manager

4. **Integration Layer** (after all components):
   - Task I1: Meta-tool handlers
   - Task I2: Resource/prompt forwarding
   - Task I3: Instructions summary
   - Task I4: Full integration wiring

## External Integrations

| Service | Purpose | API Key | Real or Mock |
|---------|---------|---------|--------------|
| Gemini Flash | Tool search | GEMINI_API_KEY env or config | Real (with keyword fallback) |
| Upstream MCP servers | Tool execution | Per-server in config | Real in E2E, mock in unit/integration |

## Dependency Graph

```
F1 (module) -> F2 (types) -> F3 (config)
                                  |
                    +-------------+------+
                    |             |      |
                   C2            C3     C4
                (search)   (transport) (cli)
                    |             |      |
                    |            C1      |
                    |         (catalog)  |
                    |             |      |
                    +------+------+------+
                                  |
                         I1 (meta-tools)
                         I2 (resources)
                         I3 (instructions)
                         I4 (wiring)
```
