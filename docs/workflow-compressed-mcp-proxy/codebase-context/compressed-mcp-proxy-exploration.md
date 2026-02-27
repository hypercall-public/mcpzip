# Exploration: Compressed MCP Proxy

## Feature Description
An MCP server that proxies other MCP servers but presents tool definitions in a compressed, token-efficient format. MCP tools currently consume ~10% of context window (and up to 100% with multiple servers enabled). This proxy would dramatically reduce that overhead while preserving full functionality.

## Architecture

### MCP Protocol Fundamentals
- Built on JSON-RPC 2.0 over stdio, SSE, or Streamable HTTP transports
- Stateful sessions: initialize -> capability negotiation -> normal operation
- Key methods: `tools/list` (returns tool schemas), `tools/call` (executes a tool)
- Tool definitions include: `name` (required), `description` (optional but critical for LLM), `inputSchema` (required JSON Schema object)

### Proxy Pattern
```
Claude Code (client)
     |  stdio or HTTP
     v
[Compressed MCP Proxy] -- acts as server downstream, client upstream
     |         |         |
     +--- stdio --> Upstream Server A
     +--- stdio --> Upstream Server B
     +--- HTTP  --> Upstream Server C
```

The proxy is simultaneously:
1. An MCP **server** to the downstream client (Claude Code)
2. An MCP **client** to one or more upstream MCP servers

For `tools/list`: proxy fetches from all upstreams, compresses/transforms definitions, returns merged list
For `tools/call`: proxy maps the tool name to the correct upstream, forwards the call, returns the result

### Data Flow
1. Claude Code spawns proxy via stdio (or connects via HTTP)
2. Proxy initializes, connects to configured upstream servers
3. On `tools/list`: proxy fetches all upstream tools, applies compression, returns compressed definitions
4. On `tools/call`: proxy resolves tool name -> upstream server, forwards call with original arguments, returns result
5. On `notifications/tools/list_changed` from upstream: proxy re-fetches and notifies downstream

## Token Overhead Analysis (Quantitative)

### Measured Impact
| Scenario | Tokens | Context % (200k) |
|----------|--------|-------------------|
| Minimal MCP (~1-2 servers) | 20,307 | 10.2% |
| Current session (deferred tools) | 41,662 | 20.8% |
| Several servers fully loaded | 72,926 | 36.5% |
| Maximum observed (all servers) | 115,246 | 57.6% |
| MCP overhead at maximum | ~104,887 | 52.4% |

### Per-Tool Token Cost
| Metric | Tokens |
|--------|--------|
| Minimum tool (0 params) | 41 |
| Median tool | 80 |
| Mean tool | 93 |
| Maximum tool (11 params) | 306 |
| Average real FastMCP tool | 150-200 |

### Token Distribution Within Schemas
| Component | % of Total |
|-----------|-----------|
| JSON structural syntax (`{}[]",:` + keys) | 33.7% |
| Property schemas (param names + types) | 40.1% |
| Descriptions (docstrings) | 14.8% |
| Tool names (`mcp__server__tool`) | 11.4% |

**Key finding: 71.9% of all characters in a tool schema are JSON syntax, not semantic content.**

### Compression Strategy Comparison (91 Telegram tools)
| Strategy | Tokens | Reduction |
|----------|--------|-----------|
| Full JSON + full prefix (baseline) | 8,751 | 0% |
| Minified JSON + short prefix | 4,930 | -44% |
| Prose format `name(params): desc` | 2,154 | -75% |
| Names only (current deferred mechanism) | 993 | -89% |
| Short names only (theoretical minimum) | 473 | -95% |

### What Claude Actually Needs
**Critical fields to preserve:**
- `name` - exact string for routing
- `description` - 1-2 sentences for tool selection
- `inputSchema.properties` - parameter names and types
- `inputSchema.required` - which params are mandatory

**Safe to strip:**
- `title` on tools and properties (display-only)
- `annotations` (behavioral hints, untrusted)
- `outputSchema` (client-side validation only)
- `_meta` (protocol metadata)
- `examples`, `default` values in properties
- `additionalProperties: false` (validator metadata)
- `$schema`, `$defs` (JSON Schema meta)
- Pydantic-generated `title` fields and verbose `anyOf` for Union types

## Existing Prior Art

### Transport-Bridging Proxies
- **punkpeye/mcp-proxy** (TypeScript): SSE/HTTP proxy over stdio, session management
- **sparfenyuk/mcp-proxy** (Python): Bidirectional transport bridging, OAuth2
- **supercorp-ai/supergateway**: stdio to SSE/HTTP

### Aggregator/Multiplexer Proxies
- **metatool-ai/metamcp**: Multi-server namespacing, tool overrides, rate limiting
- **dwillitzer/mcp-aggregator**: Universal aggregator, auto-discovery, hot reload
- **nazar256/combine-mcp**: Multiple servers with tool filtering

### Filtering Proxies
- **pro-vi/mcp-filter** (Python): Allowlist/deny filtering. Supabase: 50k -> 1.9k tokens (91% reduction)
- **igrigorik/MCProxy** (Rust): Aggregation + search + regex filters

### Compression/Transformation Proxies
- **samteezy/mcp-context-proxy**: LLM-based response compression, description override
- **vivekhaldar/mcpblox** (Node.js): LLM-generated JS transforms on tool defs
- **IBM/mcp-context-forge**: Multi-algorithm compression (Brotli, Zstd, GZip), TOON support

### Token Reduction Approaches
- **Claude Code ToolSearch/Deferred**: 85-95% reduction via lazy loading (built-in)
- **Speakeasy Dynamic Toolsets**: 3-tool search/describe/execute pattern, 96.7% reduction
- **KiloCode XML proposal**: JSON -> XML schema encoding, 47% reduction
- **SEP-1576**: $ref deduplication + embedding selection (protocol-level proposal)
- **TOON format**: Token-Oriented Object Notation, 30-60% reduction vs JSON

## Claude Code's Current Deferred/ToolSearch Mechanism
- Activates when MCP tools exceed 10% of context (configurable via `ENABLE_TOOL_SEARCH`)
- Deferred tools listed as bare names in system prompt (~993 tokens for 91 tools)
- Single `ToolSearch` tool added (~494 tokens) for on-demand schema loading
- Result: 85-95% reduction in upfront token cost
- Tradeoff: +1 API round-trip per new tool used in a session

## Language/Framework Recommendation

### Primary: TypeScript (`@modelcontextprotocol/sdk`)
- Reference SDK (v1.27.1), highest spec compliance
- Richest community: most MCP proxy examples are in TypeScript
- Working proxy reference code (punkpeye, samteezy)
- Low-level `Server` class enables full request interception for proxy
- `Client` class for upstream connections with `listTools()`, `callTool()`
- Vitest for testing
- Cold start: 200-400ms (acceptable for stdio)

### Secondary: Go (`modelcontextprotocol/go-sdk`)
- Best performance (0.855ms latency, 18MB memory, sub-100ms cold start)
- Single binary deployment
- Official SDK v1.4.0 with Google collaboration
- Existing Go proxy: TBXark/mcp-proxy

### Recommendation
**TypeScript** for this project because:
1. The MCP ecosystem is TypeScript-first; all reference implementations are TS
2. Fastest iteration speed for a new project
3. User likely already has Node.js in their workflow (Claude Code is Node-based)
4. Performance is adequate for a local proxy (single user, not multi-tenant)
5. Easiest to read/modify upstream MCP server source code

## SDK Patterns for Building the Proxy

### TypeScript Low-Level Server (for proxy)
```typescript
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { ListToolsRequestSchema, CallToolRequestSchema } from "@modelcontextprotocol/sdk/types.js";

// Downstream: Server to Claude Code
const server = new Server(
  { name: "compressed-proxy", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

// Upstream: Client to real MCP server
const client = new Client({ name: "proxy-client", version: "1.0.0" });
await client.connect(new StdioClientTransport({ command: "node", args: ["server.js"] }));

// Intercept and compress tools/list
server.setRequestHandler(ListToolsRequestSchema, async () => {
  const { tools } = await client.listTools();
  return { tools: tools.map(compressTool) };
});

// Forward tools/call unchanged
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  return await client.callTool(request.params);
});

await server.connect(new StdioServerTransport());
```

### Claude Code Configuration
```json
{
  "mcpServers": {
    "compressed-proxy": {
      "type": "stdio",
      "command": "node",
      "args": ["./dist/index.js"],
      "env": {
        "UPSTREAM_CONFIG": "./upstream-servers.json"
      }
    }
  }
}
```

## API Keys Status
| Service | Status | Notes |
|---------|--------|-------|
| No external APIs needed | N/A | Proxy only relays to upstream MCP servers |

## Key Files for This Feature
| File | Purpose | Relevance |
|------|---------|-----------|
| `src/index.ts` | Main entry point | Proxy server setup, transport init |
| `src/proxy.ts` | Core proxy logic | Client/server dual pattern, tool routing |
| `src/compress.ts` | Compression engine | Tool schema transformation |
| `src/config.ts` | Configuration | Upstream server definitions |
| `upstream-servers.json` | Upstream config | Which MCP servers to proxy |
| `package.json` | Dependencies | @modelcontextprotocol/sdk, zod |

## Patterns to Follow
1. **Low-level `Server` class**: Use `setRequestHandler()` for full control over request/response
2. **Multi-upstream aggregation**: Namespace tools with short prefixes (e.g., `gh__`, `sl__`, `tg__`)
3. **Schema stripping**: Remove `title`, `annotations`, `outputSchema`, `_meta`, `examples`, `default`
4. **Description truncation**: Keep first sentence only for each tool and parameter description
5. **Pydantic cleanup**: Remove auto-generated `title` fields, simplify `anyOf` union types to base type

## Concerns and Risks
- **Accuracy vs compression tradeoff**: Aggressive compression (removing descriptions) may reduce Claude's tool selection accuracy. Mitigation: tiered approach - simple tools get minimal schemas, complex tools keep full descriptions.
- **Upstream server lifecycle**: Must handle upstream server crashes, restarts, and reconnections gracefully. Mitigation: health checks, automatic reconnection.
- **Tool name collisions**: Multiple upstreams may have identically-named tools. Mitigation: mandatory short prefix namespacing.
- **Schema validation bypass**: Stripping schema fields means Claude might send invalid arguments. Mitigation: proxy validates against full schema before forwarding to upstream.
- **Startup latency**: Spawning multiple upstream stdio servers adds to cold start time. Mitigation: lazy upstream connections (connect on first tool call, not at startup).

## Recommendations
- Start with TypeScript and the @modelcontextprotocol/sdk
- Focus on schema stripping + description truncation as the primary compression strategy (-44% to -75%)
- Support both stdio and HTTP upstream transports
- Make compression level configurable (none/minimal/aggressive)
- Consider lazy upstream connections to minimize startup time
- Add a `--dry-run` mode that shows token savings without proxying
