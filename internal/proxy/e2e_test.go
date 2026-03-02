// https://hypercall.xyz

package proxy_test

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/hypercall-public/mcpzip/internal/catalog"
	"github.com/hypercall-public/mcpzip/internal/proxy"
	"github.com/hypercall-public/mcpzip/internal/search"
	"github.com/hypercall-public/mcpzip/internal/transport"
	"github.com/hypercall-public/mcpzip/internal/types"
)

// TestE2E_CachePersistence verifies the catalog cache survives restarts.
func TestE2E_CachePersistence(t *testing.T) {
	dir := t.TempDir()
	cachePath := filepath.Join(dir, "cache", "tools.json")

	tools := map[string][]types.ToolEntry{
		"slack": {{
			Name: "send_message", OriginalName: "send_message",
			Description: "Send a message",
			InputSchema: json.RawMessage(`{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}`),
		}},
	}

	// First "run": build proxy, refresh catalog (writes cache).
	configs := map[string]types.ServerConfig{"slack": {Command: "slack-mcp"}}
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		return &mockUpstreamForIntegration{serverName: name, tools: tools[name]}, nil
	}

	tm1 := transport.NewManager(configs, 10*time.Minute, 0, mockConnect)
	cat1 := catalog.New(tm1, cachePath)
	if err := cat1.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}
	tm1.Close()

	// Verify cache file exists.
	if _, err := os.Stat(cachePath); err != nil {
		t.Fatalf("cache file not created: %v", err)
	}

	// Second "run": load from cache only (no lister connected).
	cat2 := catalog.New(nil, cachePath)
	if err := cat2.Load(); err != nil {
		t.Fatal(err)
	}
	if cat2.ToolCount() != 1 {
		t.Fatalf("expected 1 tool from cache, got %d", cat2.ToolCount())
	}
	tool, err := cat2.GetTool("slack__send_message")
	if err != nil {
		t.Fatal(err)
	}
	if tool.Description != "Send a message" {
		t.Errorf("cached description = %q", tool.Description)
	}
}

// TestE2E_ProxyLifecycle tests full startup -> use -> shutdown.
func TestE2E_ProxyLifecycle(t *testing.T) {
	dir := t.TempDir()
	cachePath := filepath.Join(dir, "tools.json")

	tools := map[string][]types.ToolEntry{
		"notion": {{
			Name: "search", OriginalName: "search",
			Description: "Search Notion pages",
			InputSchema: json.RawMessage(`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}`),
		}},
		"linear": {{
			Name: "create_issue", OriginalName: "create_issue",
			Description: "Create a Linear issue",
			InputSchema: json.RawMessage(`{"type":"object","properties":{"title":{"type":"string"},"team":{"type":"string"}},"required":["title","team"]}`),
		}},
	}

	configs := map[string]types.ServerConfig{
		"notion": {Command: "notion-mcp"},
		"linear": {Command: "linear-mcp"},
	}
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		return &mockUpstreamForIntegration{serverName: name, tools: tools[name]}, nil
	}

	// Startup.
	tm := transport.NewManager(configs, 10*time.Minute, 0, mockConnect)
	defer tm.Close()

	cat := catalog.New(tm, cachePath)
	if err := cat.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher("", "", catalogFn)
	srv := proxy.New(cat, searcher, tm)

	ctx := context.Background()

	// Use: search for notion.
	result, err := srv.HandleSearchTools(ctx, json.RawMessage(`{"query": "notion search"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "notion__search") {
		t.Error("search should find notion__search")
	}

	// Use: execute linear tool.
	execResult, err := srv.HandleExecuteTool(ctx, json.RawMessage(`{
		"name": "linear__create_issue",
		"arguments": {"title": "Bug fix", "team": "ENG"}
	}`))
	if err != nil {
		t.Fatal(err)
	}
	var resp map[string]any
	json.Unmarshal(execResult, &resp)
	if resp["server"] != "linear" {
		t.Errorf("expected server=linear, got %v", resp["server"])
	}

	// Verify instructions.
	instructions := srv.Instructions()
	if !strings.Contains(instructions, "notion") || !strings.Contains(instructions, "linear") {
		t.Error("instructions should mention both servers")
	}
}

// TestE2E_UpstreamFailure tests graceful degradation when one upstream fails.
func TestE2E_UpstreamFailure(t *testing.T) {
	tools := map[string][]types.ToolEntry{
		"working": {{
			Name: "tool1", OriginalName: "tool1",
			Description: "Working tool",
			InputSchema: json.RawMessage(`{"type":"object"}`),
		}},
	}

	configs := map[string]types.ServerConfig{
		"working": {Command: "working-mcp"},
		"broken":  {Command: "broken-mcp"},
	}

	callCount := 0
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		callCount++
		if name == "broken" {
			return &failingUpstream{}, nil
		}
		return &mockUpstreamForIntegration{serverName: name, tools: tools[name]}, nil
	}

	tm := transport.NewManager(configs, 10*time.Minute, 0, mockConnect)
	defer tm.Close()

	cat := catalog.New(tm, "")
	if err := cat.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	// Should still have the working tool despite broken upstream.
	if cat.ToolCount() != 1 {
		t.Fatalf("expected 1 tool (from working server), got %d", cat.ToolCount())
	}
	tool, err := cat.GetTool("working__tool1")
	if err != nil {
		t.Fatal(err)
	}
	if tool.ServerName != "working" {
		t.Error("tool should be from working server")
	}
}

// TestE2E_EmptyCatalogSearch verifies search works on empty catalog.
func TestE2E_EmptyCatalogSearch(t *testing.T) {
	cat := catalog.New(nil, "")
	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher("", "", catalogFn)
	tm := transport.NewManager(nil, 10*time.Minute, 0, nil)
	defer tm.Close()

	srv := proxy.New(cat, searcher, tm)
	result, err := srv.HandleSearchTools(context.Background(), json.RawMessage(`{"query": "anything"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "No tools found") {
		t.Error("empty catalog search should return no tools message")
	}
}

// TestE2E_LargeToolCatalog tests search performance with many tools.
func TestE2E_LargeToolCatalog(t *testing.T) {
	tools := make(map[string][]types.ToolEntry)
	var serverTools []types.ToolEntry
	for i := range 100 {
		name := "tool_" + string(rune('a'+i%26)) + "_" + json.Number(json.Number(string(rune('0'+i/10)))).String() + json.Number(json.Number(string(rune('0'+i%10)))).String()
		serverTools = append(serverTools, types.ToolEntry{
			Name:         name,
			OriginalName: name,
			Description:  "Tool number " + name,
			InputSchema:  json.RawMessage(`{"type":"object"}`),
		})
	}
	tools["bigserver"] = serverTools

	configs := map[string]types.ServerConfig{"bigserver": {Command: "big-mcp"}}
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		return &mockUpstreamForIntegration{serverName: name, tools: tools[name]}, nil
	}

	tm := transport.NewManager(configs, 10*time.Minute, 0, mockConnect)
	defer tm.Close()

	cat := catalog.New(tm, "")
	if err := cat.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher("", "", catalogFn)
	srv := proxy.New(cat, searcher, tm)

	// Search should respect limit.
	result, err := srv.HandleSearchTools(context.Background(), json.RawMessage(`{"query": "tool", "limit": 3}`))
	if err != nil {
		t.Fatal(err)
	}
	// Count tool entries in result (separated by double newlines).
	parts := strings.Split(result, "\n\n")
	if len(parts) > 3 {
		t.Errorf("expected at most 3 results, got %d", len(parts))
	}
}

// failingUpstream simulates an upstream that fails on ListTools.
type failingUpstream struct{}

func (f *failingUpstream) ListTools(ctx context.Context) ([]types.ToolEntry, error) {
	return nil, context.DeadlineExceeded
}
func (f *failingUpstream) CallTool(ctx context.Context, name string, args json.RawMessage) (json.RawMessage, error) {
	return nil, context.DeadlineExceeded
}
func (f *failingUpstream) Close() error { return nil }
func (f *failingUpstream) Alive() bool  { return true }
