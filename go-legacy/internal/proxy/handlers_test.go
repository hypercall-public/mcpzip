// https://hypercall.xyz

package proxy

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/jake/mcpzip/internal/catalog"
	"github.com/jake/mcpzip/internal/search"
	"github.com/jake/mcpzip/internal/transport"
	"github.com/jake/mcpzip/internal/types"
)

func setupTestServer() *Server {
	tools := map[string][]types.ToolEntry{
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
	}
	lister := &mockToolLister{servers: tools}
	cat := catalog.New(lister, "")
	cat.RefreshAll(context.Background())

	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher("", "", catalogFn)

	configs := map[string]types.ServerConfig{
		"slack": {Command: "slack-mcp"},
	}
	mockConnect := func(ctx context.Context, name string, cfg types.ServerConfig) (transport.Upstream, error) {
		return &mockUpstream{name: name}, nil
	}
	tm := transport.NewManager(configs, 0, 0, mockConnect)

	return New(cat, searcher, tm)
}

type mockToolLister struct {
	servers map[string][]types.ToolEntry
}

func (m *mockToolLister) ListToolsAll(ctx context.Context) (map[string][]types.ToolEntry, error) {
	return m.servers, nil
}

type mockUpstream struct {
	name string
}

func (m *mockUpstream) ListTools(ctx context.Context) ([]types.ToolEntry, error) {
	return nil, nil
}

func (m *mockUpstream) CallTool(ctx context.Context, toolName string, args json.RawMessage) (json.RawMessage, error) {
	result := map[string]string{
		"server":   m.name,
		"tool":     toolName,
		"response": "ok",
	}
	data, _ := json.Marshal(result)
	return data, nil
}

func (m *mockUpstream) Close() error { return nil }
func (m *mockUpstream) Alive() bool  { return true }

func TestHandleSearchTools(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"query": "slack channels"}`)
	result, err := s.HandleSearchTools(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "slack__channels_list") {
		t.Errorf("expected channels_list in results, got: %s", result)
	}
}

func TestHandleSearchTools_NoResults(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"query": "nonexistent_xyz"}`)
	result, err := s.HandleSearchTools(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "No tools found") {
		t.Errorf("expected no results message, got: %s", result)
	}
}

func TestHandleSearchTools_EmptyQuery(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"query": ""}`)
	_, err := s.HandleSearchTools(context.Background(), args)
	if err == nil {
		t.Error("expected error for empty query")
	}
}

func TestHandleSearchTools_WithLimit(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"query": "slack", "limit": 1}`)
	result, err := s.HandleSearchTools(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	// With limit 1, should only have one tool entry.
	parts := strings.Split(result, "\n\n")
	if len(parts) != 1 {
		t.Errorf("expected 1 result with limit 1, got %d sections", len(parts))
	}
}

func TestHandleDescribeTool(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "slack__send_message"}`)
	result, err := s.HandleDescribeTool(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result, "slack__send_message") {
		t.Errorf("expected tool name in result, got: %s", result)
	}
	if !strings.Contains(result, "Input Schema") {
		t.Errorf("expected input schema in result, got: %s", result)
	}
	if !strings.Contains(result, "channel") {
		t.Errorf("expected channel param in schema, got: %s", result)
	}
}

func TestHandleDescribeTool_Unknown(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "unknown__tool"}`)
	_, err := s.HandleDescribeTool(context.Background(), args)
	if err == nil {
		t.Error("expected error for unknown tool")
	}
}

func TestHandleDescribeTool_EmptyName(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": ""}`)
	_, err := s.HandleDescribeTool(context.Background(), args)
	if err == nil {
		t.Error("expected error for empty name")
	}
}

func TestHandleExecuteTool(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "slack__send_message", "arguments": {"channel": "#general", "text": "hello"}}`)
	result, err := s.HandleExecuteTool(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}

	var resp map[string]string
	if err := json.Unmarshal(result, &resp); err != nil {
		t.Fatal(err)
	}
	if resp["server"] != "slack" {
		t.Errorf("expected server=slack, got %q", resp["server"])
	}
	if resp["tool"] != "send_message" {
		t.Errorf("expected tool=send_message, got %q", resp["tool"])
	}
}

func TestHandleExecuteTool_UnknownTool(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "unknown__tool", "arguments": {}}`)
	_, err := s.HandleExecuteTool(context.Background(), args)
	if err == nil {
		t.Error("expected error for unknown tool")
	}
}

func TestHandleExecuteTool_EmptyName(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "", "arguments": {}}`)
	_, err := s.HandleExecuteTool(context.Background(), args)
	if err == nil {
		t.Error("expected error for empty name")
	}
}

func TestHandleExecuteTool_StringArguments(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	// LLMs sometimes send arguments as a JSON string instead of an object.
	args := json.RawMessage(`{"name": "slack__send_message", "arguments": "{\"channel\": \"#general\", \"text\": \"hello\"}"}`)
	result, err := s.HandleExecuteTool(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}

	var resp map[string]string
	if err := json.Unmarshal(result, &resp); err != nil {
		t.Fatal(err)
	}
	if resp["server"] != "slack" {
		t.Errorf("expected server=slack, got %q", resp["server"])
	}
	if resp["tool"] != "send_message" {
		t.Errorf("expected tool=send_message, got %q", resp["tool"])
	}
}

func TestHandleExecuteTool_InvalidArgs(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	_, err := s.HandleExecuteTool(context.Background(), json.RawMessage(`invalid`))
	if err == nil {
		t.Error("expected error for invalid JSON args")
	}
}

func TestHandleExecuteTool_ProxyStatus(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	args := json.RawMessage(`{"name": "proxy_status", "arguments": {}}`)
	result, err := s.HandleExecuteTool(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	var resp struct {
		ToolCount   int      `json:"tool_count"`
		ServerNames []string `json:"server_names"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		t.Fatal(err)
	}
	if resp.ToolCount != 2 {
		t.Errorf("tool count = %d, want 2", resp.ToolCount)
	}
}

func TestHandleSearchTools_LimitCapped(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()

	// Request limit 9999 - should be capped to maxSearchLimit.
	args := json.RawMessage(`{"query": "slack", "limit": 9999}`)
	_, err := s.HandleSearchTools(context.Background(), args)
	if err != nil {
		t.Fatal(err)
	}
	// No panic/OOM is the test - it should work fine.
}
