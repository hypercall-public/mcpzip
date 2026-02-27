# mcpzip Architecture

## Component Overview

```
                      Claude Code
                          |
                      [stdio transport]
                          |
                    +-----v------+
                    |   Proxy    |  MCP Server: exposes 3 meta-tools
                    |  Server    |  + forwarded resources/prompts
                    +--+--+--+--+
                       |  |  |
          +------------+  |  +------------+
          |               |               |
    +-----v-----+  +-----v-----+  +------v------+
    |  Search   |  | Catalog   |  | Transport   |
    |  Engine   |  | Manager   |  | Manager     |
    +-----+-----+  +-----+-----+  +------+------+
          |               |               |
    +-----v-----+  +-----v-----+  +------v------+
    | LLM       |  | Disk      |  | Connection  |
    | Backend   |  | Cache     |  | Pool        |
    | (Gemini)  |  |           |  | (stdio+HTTP)|
    +-----------+  +-----------+  +-------------+
                                        |
                              +---------+---------+
                              |                   |
                        [stdio spawn]       [HTTP connect]
                              |                   |
                     Upstream Server A    Upstream Server B
```

## Foundation (Build First)

### Shared Types
- **File**: `internal/types/types.go`
- **Exports**:
  - `ToolEntry` - cached tool info (name, server, description, inputSchema as raw JSON)
  - `ServerConfig` - upstream server definition (command, args, env, type, url)
  - `ProxyConfig` - full config struct (servers, search settings, Gemini key, idle timeout)
  - `SearchResult` - search response item (name, description, compact params)
  - `ServerStatus` - health info (name, connected, tool count, last refresh)

### Config Package
- **File**: `internal/config/config.go`
- **Purpose**: Load, validate, and provide access to `~/.config/compressed-mcp-proxy/config.json`
- **Exports**:
  - `Load(path string) (*ProxyConfig, error)` - load config from disk
  - `DefaultPath() string` - returns `~/.config/compressed-mcp-proxy/config.json`
  - `ClaudeCodeConfig` struct - parsed Claude Code MCP config for migration

## Independent Components (Build in Parallel)

### Component 1: Catalog Manager
- **Purpose**: Maintain the tool catalog (in-memory + disk cache). Fetch tools from upstream servers, cache to disk, support background refresh.
- **Package**: `internal/catalog/`
- **Files**:
  - `catalog.go` - `Catalog` struct with `AllTools()`, `ToolsForServer()`, `ServerNames()`, `Refresh()`
  - `cache.go` - disk cache read/write (`~/.config/compressed-mcp-proxy/cache/tools.json`)
  - `fetch.go` - connect to upstream, call `tools/list`, collect all tools (handles pagination)
- **Dependencies**: `internal/types`, `internal/config`
- **Interface**:
  - Input: `ProxyConfig` (server definitions)
  - Output: `[]ToolEntry` (full tool catalog with server prefixes)
  - Methods:
    - `New(cfg *config.ProxyConfig) *Catalog`
    - `Load() error` - load from disk cache
    - `RefreshAll(ctx context.Context) error` - fetch from all upstreams, update cache
    - `RefreshServer(ctx context.Context, name string) error` - refresh single server
    - `AllTools() []ToolEntry`
    - `GetTool(prefixedName string) (*ToolEntry, error)`
    - `ServerStatus() []ServerStatus`
  - Events: catalog publishes refresh events for the proxy to forward `notifications/tools/list_changed`
- **Can parallel with**: Search Engine, Transport Manager, CLI

### Component 2: Search Engine
- **Purpose**: Match natural language queries to tools using LLM-powered search with semantic caching and keyword fallback.
- **Package**: `internal/search/`
- **Files**:
  - `search.go` - `Searcher` interface and orchestration (try cache -> LLM -> keyword fallback)
  - `llm.go` - `LLMSearcher` struct, Gemini Flash integration (full catalog prompt)
  - `keyword.go` - `KeywordSearcher` struct, fuzzy matching on tool names + descriptions
  - `cache.go` - `QueryCache` struct, keyword-overlap semantic matching
- **Dependencies**: `internal/types` (for `ToolEntry`, `SearchResult`)
- **Interface**:
  - Input: query string, tool catalog (`[]ToolEntry`), limit int
  - Output: `[]SearchResult`
  - Methods:
    - `NewSearcher(apiKey string, model string, catalog func() []ToolEntry) Searcher`
    - `Search(ctx context.Context, query string, limit int) ([]SearchResult, error)`
  - The `catalog` parameter is a function (not a direct reference) so Search is decoupled from Catalog
- **Searcher interface**:
  ```go
  type Searcher interface {
      Search(ctx context.Context, query string, limit int) ([]SearchResult, error)
  }
  ```
- **Can parallel with**: Catalog Manager, Transport Manager, CLI

### Component 3: Transport Manager
- **Purpose**: Manage connections to upstream MCP servers. Connection pooling with idle timeout. Support both stdio (spawn child process) and HTTP transports.
- **Package**: `internal/transport/`
- **Files**:
  - `manager.go` - `Manager` struct, connection pool, idle timeout reaping
  - `stdio.go` - spawn child process, create MCP client over stdio
  - `http.go` - connect to HTTP MCP endpoint, create MCP client
  - `upstream.go` - `Upstream` interface wrapping an MCP client connection
- **Dependencies**: `internal/types`, `internal/config`, `go-sdk` (Client, transports)
- **Interface**:
  - Input: `ServerConfig` (how to connect to an upstream)
  - Output: MCP client connection for making `callTool`, `listTools`, etc.
  - Methods:
    - `New(cfg *config.ProxyConfig) *Manager`
    - `GetConnection(ctx context.Context, serverName string) (*Upstream, error)` - get or create connection
    - `CallTool(ctx context.Context, serverName string, toolName string, args json.RawMessage) (*mcp.CallToolResult, error)`
    - `ListResources(ctx context.Context, serverName string) ([]mcp.Resource, error)`
    - `ReadResource(ctx context.Context, serverName string, uri string) (*mcp.ReadResourceResult, error)`
    - `ListPrompts(ctx context.Context, serverName string) ([]mcp.Prompt, error)`
    - `GetPrompt(ctx context.Context, serverName string, name string, args map[string]string) (*mcp.GetPromptResult, error)`
    - `Close()` - tear down all connections
  - Idle timeout: background goroutine reaps connections unused for N minutes
- **Can parallel with**: Catalog Manager, Search Engine, CLI

### Component 4: CLI
- **Purpose**: Command-line interface for serve, init, and migrate subcommands.
- **Package**: `internal/cli/`
- **Files**:
  - `root.go` - root cobra command
  - `serve.go` - `mcpzip serve` - starts the proxy (wires components, connects stdio)
  - `init.go` - `mcpzip init` - interactive setup wizard
  - `migrate.go` - `mcpzip migrate` - auto-migrate from Claude Code config
- **Dependencies**: `internal/config`, cobra
- **Interface**:
  - `serve` wires: config -> catalog -> search -> transport -> proxy server
  - `init` prompts user, generates config, calls migrate internally
  - `migrate` reads Claude Code config, generates proxy config, updates Claude Code config
- **Can parallel with**: Catalog Manager, Search Engine, Transport Manager

## Integration Layer (Build After Components)

### Proxy Server
- **Package**: `internal/proxy/`
- **Files**:
  - `server.go` - MCP server setup, register meta-tools, wire components
  - `handlers.go` - request handlers for search_tools, describe_tool, execute_tool
  - `resources.go` - aggregated resource/prompt forwarding
  - `instructions.go` - LLM-summarized server instructions
- **Purpose**: Wires Catalog + Search + Transport into an MCP server. Registers the 3 meta-tools. Handles resource/prompt forwarding. Manages lifecycle.
- **Wiring**:
  ```
  serve command
    -> load config
    -> create catalog (load from disk cache)
    -> create search engine (with catalog.AllTools as provider)
    -> create transport manager
    -> create proxy server (catalog + search + transport)
    -> start background catalog refresh
    -> connect to stdio transport
    -> serve until stdin closes
    -> cleanup (close transport manager)
  ```

### Integration Tests
- Full search -> execute loop against mock upstream MCP servers
- Catalog refresh while proxy is serving (no disruption)
- Upstream server failure -> graceful degradation
- Idle timeout -> connection teardown -> reconnection on next call
- Resource/prompt forwarding from multiple upstreams

## Interfaces

### Core Go Interfaces

```go
// internal/types/types.go

type ToolEntry struct {
    Name         string          // prefixed: "servername__toolname"
    ServerName   string          // which upstream server
    OriginalName string          // unprefixed tool name
    Description  string          // tool description
    InputSchema  json.RawMessage // full JSON Schema
    CompactParams string         // "chat_id:string*, message:string*" (for search results)
}

type SearchResult struct {
    Name          string // prefixed name
    Description   string // first sentence
    CompactParams string // param summary
}

type ServerConfig struct {
    Type    string            // "stdio" or "http"
    Command string            // for stdio
    Args    []string          // for stdio
    Env     map[string]string // for stdio
    URL     string            // for http
}

type ProxyConfig struct {
    GeminiAPIKey       string                   `json:"gemini_api_key"`
    Search             SearchConfig             `json:"search"`
    IdleTimeoutMinutes int                      `json:"idle_timeout_minutes"`
    MCPServers         map[string]ServerConfig   `json:"mcpServers"`
}

type SearchConfig struct {
    DefaultLimit int    `json:"default_limit"`
    Model        string `json:"model"`
}
```

```go
// internal/search/search.go

type Searcher interface {
    Search(ctx context.Context, query string, limit int) ([]types.SearchResult, error)
}
```

```go
// internal/transport/upstream.go

type Upstream interface {
    CallTool(ctx context.Context, name string, args json.RawMessage) (*mcp.CallToolResult, error)
    ListResources(ctx context.Context) ([]mcp.Resource, error)
    ReadResource(ctx context.Context, uri string) (*mcp.ReadResourceResult, error)
    ListPrompts(ctx context.Context) ([]mcp.Prompt, error)
    GetPrompt(ctx context.Context, name string, args map[string]string) (*mcp.GetPromptResult, error)
    Close() error
}
```

## Data Flow

### search_tools flow
1. Claude calls `search_tools(query="send telegram message", limit=5)`
2. Proxy handler receives the call
3. Check semantic cache (keyword overlap > 60%)
4. Cache miss: send query + full tool catalog to Gemini Flash
5. Gemini returns top N tool names
6. Map names to ToolEntry, build SearchResult with compact params
7. Store in cache, return results to Claude

### describe_tool flow
1. Claude calls `describe_tool(name="telegram-jakesyl__send_message")`
2. Proxy handler receives the call
3. Look up ToolEntry in catalog by prefixed name
4. Return full InputSchema JSON

### execute_tool flow
1. Claude calls `execute_tool(name="telegram-jakesyl__send_message", arguments={...})`
2. Proxy handler parses prefix: server="telegram-jakesyl", tool="send_message"
3. Transport manager: get or create connection to "telegram-jakesyl" server
4. Forward `tools/call` with original tool name and arguments
5. Return upstream response as-is

### Startup flow
1. Load config from `~/.config/compressed-mcp-proxy/config.json`
2. Load tool catalog from disk cache (`cache/tools.json`)
3. Create search engine with catalog provider
4. Create transport manager (no connections yet - lazy)
5. Create proxy server, register meta-tools
6. Connect to stdio transport (start serving)
7. Background: refresh catalog per-server with independent timeouts

## Files to Create/Modify

| File | Action | Purpose | Phase |
|------|--------|---------|-------|
| `go.mod` | Create | Go module definition | Foundation |
| `cmd/mcpzip/main.go` | Create | Entry point | Foundation |
| `internal/types/types.go` | Create | Shared types | Foundation |
| `internal/config/config.go` | Create | Config loading | Foundation |
| `internal/catalog/catalog.go` | Create | Tool catalog manager | Parallel |
| `internal/catalog/cache.go` | Create | Disk cache R/W | Parallel |
| `internal/catalog/fetch.go` | Create | Upstream tool fetching | Parallel |
| `internal/search/search.go` | Create | Search orchestration | Parallel |
| `internal/search/llm.go` | Create | Gemini Flash integration | Parallel |
| `internal/search/keyword.go` | Create | Keyword fallback | Parallel |
| `internal/search/cache.go` | Create | Semantic query cache | Parallel |
| `internal/transport/manager.go` | Create | Connection pool | Parallel |
| `internal/transport/stdio.go` | Create | Stdio upstream transport | Parallel |
| `internal/transport/http.go` | Create | HTTP upstream transport | Parallel |
| `internal/transport/upstream.go` | Create | Upstream interface | Parallel |
| `internal/cli/root.go` | Create | Cobra root command | Parallel |
| `internal/cli/serve.go` | Create | serve subcommand | Parallel |
| `internal/cli/init.go` | Create | init wizard | Parallel |
| `internal/cli/migrate.go` | Create | migrate command | Parallel |
| `internal/proxy/server.go` | Create | MCP server + wiring | Integration |
| `internal/proxy/handlers.go` | Create | Meta-tool handlers | Integration |
| `internal/proxy/resources.go` | Create | Resource/prompt forwarding | Integration |
| `internal/proxy/instructions.go` | Create | LLM instruction summary | Integration |

## Build Sequence

1. **Foundation Phase** (sequential):
   - `go.mod`, `cmd/mcpzip/main.go`
   - `internal/types/types.go` - shared types
   - `internal/config/config.go` - config loading

2. **Component Phase** (parallel):
   - Component 1: Catalog (catalog.go, cache.go, fetch.go)
   - Component 2: Search (search.go, llm.go, keyword.go, cache.go)
   - Component 3: Transport (manager.go, stdio.go, http.go, upstream.go)
   - Component 4: CLI (root.go, serve.go, init.go, migrate.go)

3. **Integration Phase** (sequential):
   - Proxy server (server.go, handlers.go, resources.go, instructions.go)
   - Wire serve command to proxy server
   - Integration tests
   - E2E tests

## External Integrations

| Service | Purpose | API Key Required |
|---------|---------|------------------|
| Gemini Flash | LLM-powered tool search | Yes (GEMINI_API_KEY or config) |
| Upstream MCP servers | Tool execution | Per-server (in config) |

## API Key Requirements

- **Gemini Flash**: Check `GEMINI_API_KEY` env var -> config `gemini_api_key` field -> fall back to keyword search
- **Upstream servers**: Keys stored in each server's `env` block in config.json. Passed through as environment variables when spawning stdio servers.

## Go Dependencies

| Package | Purpose |
|---------|---------|
| `github.com/modelcontextprotocol/go-sdk` | MCP Server + Client |
| `github.com/spf13/cobra` | CLI framework |
| `google.golang.org/genai` | Gemini Flash API (or REST) |
| `github.com/stretchr/testify` | Test assertions |
