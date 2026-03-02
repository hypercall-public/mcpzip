// https://hypercall.xyz

package proxy_test

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/hypercall-public/mcpzip/internal/catalog"
	"github.com/hypercall-public/mcpzip/internal/proxy"
	"github.com/hypercall-public/mcpzip/internal/search"
	"github.com/hypercall-public/mcpzip/internal/transport"
	"github.com/hypercall-public/mcpzip/internal/types"
)

// mockUpstreamForIntegration simulates a real upstream MCP server.
type mockUpstreamForIntegration struct {
	serverName string
	tools      []types.ToolEntry
}

func (m *mockUpstreamForIntegration) ListTools(ctx context.Context) ([]types.ToolEntry, error) {
	return m.tools, nil
}

func (m *mockUpstreamForIntegration) CallTool(ctx context.Context, toolName string, args json.RawMessage) (json.RawMessage, error) {
	result := map[string]any{
		"status":  "success",
		"server":  m.serverName,
		"tool":    toolName,
		"echo_in": json.RawMessage(args),
	}
	data, _ := json.Marshal(result)
	return data, nil
}

func (m *mockUpstreamForIntegration) Close() error { return nil }
func (m *mockUpstreamForIntegration) Alive() bool  { return true }

func buildIntegrationProxy(t *testing.T) *proxy.Server {
	t.Helper()

	// Define two mock upstream servers.
	slackTools := []types.ToolEntry{
		{
			Name:         "channels_list",
			OriginalName: "channels_list",
			Description:  "List all Slack channels",
			InputSchema:  json.RawMessage(`{"type":"object","properties":{"limit":{"type":"integer"}}}`),
		},
		{
			Name:         "send_message",
			OriginalName: "send_message",
			Description:  "Send a message to a Slack channel",
			InputSchema:  json.RawMessage(`{"type":"object","properties":{"channel":{"type":"string"},"text":{"type":"string"}},"required":["channel","text"]}`),
		},
	}
	telegramTools := []types.ToolEntry{
		{
			Name:         "send_message",
			OriginalName: "send_message",
			Description:  "Send a Telegram message to a chat",
			InputSchema:  json.RawMessage(`{"type":"object","properties":{"chat_id":{"type":"string"},"text":{"type":"string"}},"required":["chat_id","text"]}`),
		},
		{
			Name:         "get_history",
			OriginalName: "get_history",
			Description:  "Get message history from a Telegram chat",
			InputSchema:  json.RawMessage(`{"type":"object","properties":{"chat_id":{"type":"string"},"limit":{"type":"integer"}},"required":["chat_id"]}`),
		},
	}

	upstreams := map[string]*mockUpstreamForIntegration{
		"slack":    {serverName: "slack", tools: slackTools},
		"telegram": {serverName: "telegram", tools: telegramTools},
	}

	configs := map[string]types.ServerConfig{
		"slack":    {Command: "slack-mcp"},
		"telegram": {Command: "telegram-mcp"},
	}
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		return upstreams[name], nil
	}

	tm := transport.NewManager(configs, 10*time.Minute, 0, mockConnect)
	t.Cleanup(func() { tm.Close() })

	cat := catalog.New(tm, "")
	if err := cat.RefreshAll(context.Background()); err != nil {
		t.Fatal(err)
	}

	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher("", "", catalogFn)

	return proxy.New(cat, searcher, tm)
}

// TestIntegration_FullLoop exercises: search -> describe -> execute.
func TestIntegration_FullLoop(t *testing.T) {
	s := buildIntegrationProxy(t)
	ctx := context.Background()

	// Step 1: Search for "slack channels".
	searchArgs := json.RawMessage(`{"query": "slack channels", "limit": 5}`)
	searchResult, err := s.HandleSearchTools(ctx, searchArgs)
	if err != nil {
		t.Fatalf("search: %v", err)
	}
	if !strings.Contains(searchResult, "slack__channels_list") {
		t.Fatalf("search did not find channels_list: %s", searchResult)
	}

	// Step 2: Describe the found tool.
	descArgs := json.RawMessage(`{"name": "slack__channels_list"}`)
	descResult, err := s.HandleDescribeTool(ctx, descArgs)
	if err != nil {
		t.Fatalf("describe: %v", err)
	}
	if !strings.Contains(descResult, "Input Schema") {
		t.Fatalf("describe missing schema: %s", descResult)
	}

	// Step 3: Execute the tool.
	execArgs := json.RawMessage(`{"name": "slack__channels_list", "arguments": {"limit": 10}}`)
	execResult, err := s.HandleExecuteTool(ctx, execArgs)
	if err != nil {
		t.Fatalf("execute: %v", err)
	}
	var resp map[string]any
	if err := json.Unmarshal(execResult, &resp); err != nil {
		t.Fatalf("unmarshal exec result: %v", err)
	}
	if resp["server"] != "slack" {
		t.Errorf("expected server=slack, got %v", resp["server"])
	}
	if resp["tool"] != "channels_list" {
		t.Errorf("expected tool=channels_list, got %v", resp["tool"])
	}
}

// TestIntegration_MultiServer verifies tools from multiple servers are aggregated.
func TestIntegration_MultiServer(t *testing.T) {
	s := buildIntegrationProxy(t)
	ctx := context.Background()

	// Search for "send_message" - should find both slack and telegram versions.
	args := json.RawMessage(`{"query": "send message", "limit": 10}`)
	result, err := s.HandleSearchTools(ctx, args)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "slack__send_message") {
		t.Error("missing slack__send_message")
	}
	if !strings.Contains(result, "telegram__send_message") {
		t.Error("missing telegram__send_message")
	}
}

// TestIntegration_ExecuteRouting verifies tool calls route to the correct upstream.
func TestIntegration_ExecuteRouting(t *testing.T) {
	s := buildIntegrationProxy(t)
	ctx := context.Background()

	tests := []struct {
		toolName   string
		wantServer string
		wantTool   string
	}{
		{"slack__send_message", "slack", "send_message"},
		{"telegram__send_message", "telegram", "send_message"},
		{"telegram__get_history", "telegram", "get_history"},
	}

	for _, tt := range tests {
		args := json.RawMessage(`{"name": "` + tt.toolName + `", "arguments": {"test": true}}`)
		result, err := s.HandleExecuteTool(ctx, args)
		if err != nil {
			t.Errorf("%s: %v", tt.toolName, err)
			continue
		}
		var resp map[string]any
		json.Unmarshal(result, &resp)
		if resp["server"] != tt.wantServer {
			t.Errorf("%s: server = %v, want %s", tt.toolName, resp["server"], tt.wantServer)
		}
		if resp["tool"] != tt.wantTool {
			t.Errorf("%s: tool = %v, want %s", tt.toolName, resp["tool"], tt.wantTool)
		}
	}
}

// TestIntegration_SearchDescribeExecute tests the complete user flow.
func TestIntegration_SearchDescribeExecute(t *testing.T) {
	s := buildIntegrationProxy(t)
	ctx := context.Background()

	// User wants to send a telegram message.
	searchResult, err := s.HandleSearchTools(ctx, json.RawMessage(`{"query": "telegram send"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(searchResult, "telegram__send_message") {
		t.Fatalf("could not find telegram send tool")
	}

	// Describe to see full schema.
	descResult, err := s.HandleDescribeTool(ctx, json.RawMessage(`{"name": "telegram__send_message"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(descResult, "chat_id") {
		t.Fatalf("schema missing chat_id")
	}

	// Execute with proper args.
	execResult, err := s.HandleExecuteTool(ctx, json.RawMessage(`{
		"name": "telegram__send_message",
		"arguments": {"chat_id": "123", "text": "hello from mcpzip"}
	}`))
	if err != nil {
		t.Fatal(err)
	}
	var resp map[string]any
	json.Unmarshal(execResult, &resp)
	if resp["status"] != "success" {
		t.Errorf("expected success, got %v", resp["status"])
	}
}
