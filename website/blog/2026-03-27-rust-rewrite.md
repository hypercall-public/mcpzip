---
slug: rust-rewrite
title: Why We Rewrote mcpzip from Go to Rust
authors: [hypercall]
tags: [engineering, rust, go, rewrite]
---

# Why We Rewrote mcpzip from Go to Rust

mcpzip started as a Go project. It worked well -- the Go SDK for MCP was solid, the binary was reasonable, and the codebase was clean. But as we pushed the proxy harder with more servers, more tools, and tighter performance requirements, we hit Go's limits.

So we rewrote the entire thing in Rust. Here is why, and how it went.

<!-- truncate -->

## The Numbers

| Metric | Go Version | Rust Version | Change |
|--------|-----------|-------------|--------|
| Files | 45 | 35 | -22% |
| Lines of code | 5,703 | 8,147 | +43% (more tests) |
| Binary size | 11 MB | 5.8 MB | **-47%** |
| Tests | ~60 | 240+ | **4x more** |
| Dependencies | ~30 | ~25 | -17% |

The Rust version is 47% smaller as a binary, has 4x more tests, and uses fewer files despite being more code (because test code is colocated with source in Rust).

## Why Rewrite?

### 1. Binary Size

Go's runtime and garbage collector add a fixed overhead. Our Go binary was 11MB -- not huge, but unnecessary for what is essentially a proxy. The Rust binary is 5.8MB with zero runtime overhead.

For a tool that gets installed on developer machines and runs as a child process of Claude Code, every megabyte matters.

### 2. Memory Usage

Go's garbage collector means unpredictable memory spikes during catalog refresh. With 500+ tools and 10+ upstream connections, these spikes were noticeable. Rust's ownership model gives us predictable, low memory usage.

### 3. Concurrency Model

mcpzip needs careful concurrency: multiple upstream connections, parallel search strategies, background catalog refresh, and real-time tool calls -- all at once.

Go's goroutines are lightweight, but sharing mutable state requires careful channel choreography or mutex discipline. Rust's type system enforces correct concurrency at compile time:

**Go version** -- runtime race detection:

```go
type Catalog struct {
    mu    sync.RWMutex
    tools map[string]*ToolEntry
    path  string
}

func (c *Catalog) GetTool(name string) (*ToolEntry, bool) {
    c.mu.RLock()
    defer c.mu.RUnlock()
    t, ok := c.tools[name]
    return t, ok
}
```

**Rust version** -- compile-time safety:

```rust
pub struct Catalog {
    tools: RwLock<HashMap<String, ToolEntry>>,
    cache_path: PathBuf,
}

impl Catalog {
    pub fn get_tool(&self, name: &str) -> Option<ToolEntry> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }
}
```

Both use a `RwLock`, but Rust's borrow checker ensures you **cannot** forget to acquire the lock or accidentally access the data without it. In Go, nothing stops you from accessing `c.tools` directly -- you rely on discipline and race detectors to catch mistakes.

### 4. Error Handling

Go's `if err != nil` pattern is verbose but workable. Rust's `Result` and `?` operator give us the same explicit error handling with less boilerplate:

**Go**:

```go
func (c *Catalog) Load() error {
    data, err := os.ReadFile(c.path)
    if err != nil {
        return fmt.Errorf("load catalog: %w", err)
    }
    var tools map[string]*ToolEntry
    if err := json.Unmarshal(data, &tools); err != nil {
        return fmt.Errorf("parse catalog: %w", err)
    }
    c.mu.Lock()
    c.tools = tools
    c.mu.Unlock()
    return nil
}
```

**Rust**:

```rust
pub fn load(&self) -> Result<(), McpzipError> {
    let data = std::fs::read_to_string(&self.cache_path)?;
    let tools: HashMap<String, ToolEntry> = serde_json::from_str(&data)?;
    *self.tools.write().unwrap() = tools;
    Ok(())
}
```

Same logic, half the code. The `?` operator propagates errors automatically, and the `From` trait handles error type conversion.

## What Was Preserved

The Rust rewrite is a faithful port. Everything that worked in Go works the same way in Rust:

- **Same config format** -- your `config.json` works without changes
- **Same 3 meta-tools** -- `search_tools`, `describe_tool`, `execute_tool`
- **Same architecture** -- ProxyServer, Catalog, Manager, Searcher
- **Same search algorithm** -- keyword + Gemini orchestrated search
- **Same OAuth flow** -- PKCE, browser callback, token persistence
- **Same CLI** -- `serve`, `init`, `migrate` with the same flags
- **Same file locations** -- `~/.config/compressed-mcp-proxy/`

If you were using the Go version, the Rust version is a drop-in replacement.

## What Improved

Beyond the raw performance numbers, several things got better:

### Background Catalog Refresh

The Go version refreshed the catalog synchronously on startup. The Rust version loads from disk cache immediately and refreshes in the background. First request is served in under 5ms instead of waiting for all servers to connect.

### SSE Parsing

The Rust version uses a more robust SSE parser that handles edge cases in multi-line `data:` fields and reconnection.

### OAuth Token Reuse

The Rust version checks for mcp-remote tokens in addition to its own, so you do not have to re-authenticate if you have already used mcp-remote with the same server.

### Test Coverage

We went from ~60 tests to 240+. Every module has comprehensive unit tests, including edge cases like:
- Nullable JSON types (`"type": ["string", "null"]`)
- `anyOf` schemas
- Double-underscore tool name parsing
- Config validation (missing commands, invalid types)
- Token refresh and expiration

## The Claude Code Experience

Both the Go and Rust versions were written with [Claude Code](https://claude.ai/claude-code). The rewrite was a fascinating exercise in using an AI coding assistant for a major port:

1. We fed Claude the Go codebase and the MCP specification
2. Claude generated Rust modules matching the Go architecture
3. We iteratively refined the code with Claude, adding tests and fixing edge cases
4. The entire rewrite took about a day of wall-clock time

The tight feedback loop -- write code, run tests, fix issues -- works exceptionally well with Claude Code. Having 240+ tests pass gives high confidence that the port is correct.

## Should You Rewrite Your Project?

Probably not. Rewrites are risky and usually not worth it. We did it because:

1. mcpzip is relatively small (~6K LOC)
2. The architecture was clean and well-understood
3. We had specific, measurable goals (binary size, memory, test coverage)
4. We had 60+ tests as a safety net for the port

If your project is larger, consider incremental improvements instead. But if you are starting fresh and care about binary size, memory usage, and compile-time safety, Rust is an excellent choice for CLI tools and proxies.

---

mcpzip is open source at [github.com/hypercall-public/mcpzip](https://github.com/hypercall-public/mcpzip). Built by [Hypercall](https://hypercall.xyz).
