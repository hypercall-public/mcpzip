// https://hypercall.xyz

package catalog

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// mockLister implements ToolLister for testing.
type mockLister struct {
	servers map[string][]types.ToolEntry
	err     error
}

func (m *mockLister) ListToolsAll(ctx context.Context) (map[string][]types.ToolEntry, error) {
	if m.err != nil {
		return nil, m.err
	}
	return m.servers, nil
}

func sampleTools() map[string][]types.ToolEntry {
	return map[string][]types.ToolEntry{
		"slack": {
			{
				Name:         "channels_list",
				OriginalName: "channels_list",
				Description:  "List Slack channels",
				InputSchema:  json.RawMessage(`{"type":"object","properties":{"limit":{"type":"integer"}}}`),
			},
			{
				Name:         "send_message",
				OriginalName: "send_message",
				Description:  "Send a Slack message",
				InputSchema:  json.RawMessage(`{"type":"object","properties":{"channel":{"type":"string"},"text":{"type":"string"}},"required":["channel","text"]}`),
			},
		},
		"telegram": {
			{
				Name:         "send_message",
				OriginalName: "send_message",
				Description:  "Send a Telegram message",
				InputSchema:  json.RawMessage(`{"type":"object","properties":{"chat_id":{"type":"string"},"text":{"type":"string"}},"required":["chat_id","text"]}`),
			},
		},
	}
}

func TestCatalog_RefreshAndAllTools(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")

	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	tools := c.AllTools()
	if len(tools) != 3 {
		t.Fatalf("expected 3 tools, got %d", len(tools))
	}

	// Verify prefixed names.
	names := make(map[string]bool)
	for _, tool := range tools {
		names[tool.Name] = true
	}
	for _, want := range []string{"slack__channels_list", "slack__send_message", "telegram__send_message"} {
		if !names[want] {
			t.Errorf("missing tool %q", want)
		}
	}
}

func TestCatalog_GetTool(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	tool, err := c.GetTool("slack__send_message")
	if err != nil {
		t.Fatal(err)
	}
	if tool.ServerName != "slack" {
		t.Errorf("server = %q, want slack", tool.ServerName)
	}
	if tool.OriginalName != "send_message" {
		t.Errorf("original = %q, want send_message", tool.OriginalName)
	}
}

func TestCatalog_GetTool_Unknown(t *testing.T) {
	c := New(nil, "")
	_, err := c.GetTool("nonexistent")
	if err == nil {
		t.Error("expected error for unknown tool")
	}
}

func TestCatalog_ServerTools(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	slackTools := c.ServerTools("slack")
	if len(slackTools) != 2 {
		t.Errorf("expected 2 slack tools, got %d", len(slackTools))
	}
	telegramTools := c.ServerTools("telegram")
	if len(telegramTools) != 1 {
		t.Errorf("expected 1 telegram tool, got %d", len(telegramTools))
	}
}

func TestCatalog_ToolCount(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")
	if c.ToolCount() != 0 {
		t.Error("empty catalog should have 0 tools")
	}
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}
	if c.ToolCount() != 3 {
		t.Errorf("expected 3 tools, got %d", c.ToolCount())
	}
}

func TestCatalog_ServerNames(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}
	names := c.ServerNames()
	if len(names) != 2 || names[0] != "slack" || names[1] != "telegram" {
		t.Errorf("expected [slack telegram], got %v", names)
	}
}

func TestCatalog_CompactParams(t *testing.T) {
	lister := &mockLister{servers: sampleTools()}
	c := New(lister, "")
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}
	tool, _ := c.GetTool("slack__send_message")
	if tool.CompactParams == "" {
		t.Error("compact params should not be empty")
	}
	// Should contain "channel:string*" and "text:string*"
	if tool.CompactParams != "channel:string*, text:string*" {
		t.Errorf("compact params = %q", tool.CompactParams)
	}
}

func TestCatalog_LoadFromCache(t *testing.T) {
	dir := t.TempDir()
	cachePath := filepath.Join(dir, "tools.json")

	// Write a cache file.
	entries := []types.ToolEntry{
		{Name: "test__tool", ServerName: "test", OriginalName: "tool", Description: "A test tool"},
	}
	data, _ := json.Marshal(entries)
	os.WriteFile(cachePath, data, 0644)

	c := New(nil, cachePath)
	if err := c.Load(); err != nil {
		t.Fatal(err)
	}
	if c.ToolCount() != 1 {
		t.Fatalf("expected 1 tool from cache, got %d", c.ToolCount())
	}
	tool, err := c.GetTool("test__tool")
	if err != nil {
		t.Fatal(err)
	}
	if tool.Description != "A test tool" {
		t.Errorf("description = %q", tool.Description)
	}
}

func TestCatalog_LoadMissingCache(t *testing.T) {
	c := New(nil, "/nonexistent/cache.json")
	if err := c.Load(); err != nil {
		t.Errorf("missing cache should not error: %v", err)
	}
	if c.ToolCount() != 0 {
		t.Error("expected 0 tools with missing cache")
	}
}

func TestCatalog_SaveAndLoadCache(t *testing.T) {
	dir := t.TempDir()
	cachePath := filepath.Join(dir, "cache", "tools.json")

	lister := &mockLister{servers: sampleTools()}
	c := New(lister, cachePath)
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	// Load into a new catalog from the cache.
	c2 := New(nil, cachePath)
	if err := c2.Load(); err != nil {
		t.Fatal(err)
	}
	if c2.ToolCount() != c.ToolCount() {
		t.Errorf("cached tool count = %d, want %d", c2.ToolCount(), c.ToolCount())
	}
}

func TestCatalog_RefreshAll_ListerError(t *testing.T) {
	lister := &mockLister{err: fmt.Errorf("connection failed")}
	c := New(lister, "")
	err := c.RefreshAll(context.Background())
	if err == nil {
		t.Error("expected error from lister")
	}
}

func TestCatalog_RefreshAll_NoLister(t *testing.T) {
	c := New(nil, "")
	err := c.RefreshAll(context.Background())
	if err == nil {
		t.Error("expected error with nil lister")
	}
}

func TestCatalog_RefreshAll_EmptyServers(t *testing.T) {
	lister := &mockLister{servers: map[string][]types.ToolEntry{}}
	c := New(lister, "")
	if err := c.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}
	if c.ToolCount() != 0 {
		t.Error("expected 0 tools with empty servers")
	}
}
