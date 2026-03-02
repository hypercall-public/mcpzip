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
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// connectMCPProxy creates a proxy server and connects an MCP client to it
// via in-memory transport, returning the client session.
func connectMCPProxy(t *testing.T) *mcp.ClientSession {
	t.Helper()

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

	upstreams := map[string]*mockUpstreamForIntegration{
		"slack": {serverName: "slack", tools: slackTools},
	}

	configs := map[string]types.ServerConfig{
		"slack": {Command: "slack-mcp"},
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

	srv := proxy.New(cat, searcher, tm)

	// Connect via in-memory transport.
	ctx := context.Background()
	serverTransport, clientTransport := mcp.NewInMemoryTransports()

	go func() {
		_ = srv.RunWithTransport(ctx, serverTransport)
	}()

	client := mcp.NewClient(
		&mcp.Implementation{Name: "test-client", Version: "0.1.0"},
		nil,
	)
	session, err := client.Connect(ctx, clientTransport, nil)
	if err != nil {
		t.Fatalf("connecting MCP client: %v", err)
	}
	t.Cleanup(func() { session.Close() })

	return session
}

// TestMCP_ListTools verifies the proxy exposes exactly 3 meta-tools over MCP.
func TestMCP_ListTools(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	var tools []*mcp.Tool
	for tool, err := range session.Tools(ctx, nil) {
		if err != nil {
			t.Fatal(err)
		}
		tools = append(tools, tool)
	}

	if len(tools) != 3 {
		t.Fatalf("expected 3 meta-tools, got %d", len(tools))
	}

	names := map[string]bool{}
	for _, tool := range tools {
		names[tool.Name] = true
	}
	for _, expected := range []string{"search_tools", "describe_tool", "execute_tool"} {
		if !names[expected] {
			t.Errorf("missing meta-tool: %s", expected)
		}
	}
}

// TestMCP_SearchTools exercises search_tools over the real MCP protocol.
func TestMCP_SearchTools(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	result, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "search_tools",
		Arguments: map[string]any{"query": "slack channels"},
	})
	if err != nil {
		t.Fatal(err)
	}

	if result.IsError {
		t.Fatalf("search_tools returned error: %v", result.Content)
	}
	text := result.Content[0].(*mcp.TextContent).Text
	if !strings.Contains(text, "slack__channels_list") {
		t.Errorf("expected channels_list in results, got: %s", text)
	}
}

// TestMCP_DescribeTool exercises describe_tool over the real MCP protocol.
func TestMCP_DescribeTool(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	result, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "describe_tool",
		Arguments: map[string]any{"name": "slack__send_message"},
	})
	if err != nil {
		t.Fatal(err)
	}

	if result.IsError {
		t.Fatalf("describe_tool returned error: %v", result.Content)
	}
	text := result.Content[0].(*mcp.TextContent).Text
	if !strings.Contains(text, "Input Schema") {
		t.Errorf("expected Input Schema in result, got: %s", text)
	}
	if !strings.Contains(text, "channel") {
		t.Errorf("expected channel param in schema, got: %s", text)
	}
}

// TestMCP_ExecuteTool exercises execute_tool over the real MCP protocol.
func TestMCP_ExecuteTool(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	result, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name: "execute_tool",
		Arguments: map[string]any{
			"name":      "slack__send_message",
			"arguments": map[string]any{"channel": "#general", "text": "hello"},
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	if result.IsError {
		t.Fatalf("execute_tool returned error: %v", result.Content)
	}

	text := result.Content[0].(*mcp.TextContent).Text
	var resp map[string]any
	if err := json.Unmarshal([]byte(text), &resp); err != nil {
		t.Fatalf("unmarshal result: %v (text: %s)", err, text)
	}
	if resp["server"] != "slack" {
		t.Errorf("expected server=slack, got %v", resp["server"])
	}
	if resp["tool"] != "send_message" {
		t.Errorf("expected tool=send_message, got %v", resp["tool"])
	}
}

// TestMCP_FullFlow exercises the complete search -> describe -> execute flow
// over the real MCP protocol.
func TestMCP_FullFlow(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	// Step 1: Search for tools.
	searchResult, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "search_tools",
		Arguments: map[string]any{"query": "send message"},
	})
	if err != nil {
		t.Fatal(err)
	}
	searchText := searchResult.Content[0].(*mcp.TextContent).Text
	if !strings.Contains(searchText, "slack__send_message") {
		t.Fatalf("search did not find send_message: %s", searchText)
	}

	// Step 2: Describe the tool.
	describeResult, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "describe_tool",
		Arguments: map[string]any{"name": "slack__send_message"},
	})
	if err != nil {
		t.Fatal(err)
	}
	describeText := describeResult.Content[0].(*mcp.TextContent).Text
	if !strings.Contains(describeText, "channel") {
		t.Fatalf("describe missing channel param: %s", describeText)
	}

	// Step 3: Execute the tool.
	execResult, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name: "execute_tool",
		Arguments: map[string]any{
			"name":      "slack__send_message",
			"arguments": map[string]any{"channel": "#test", "text": "from mcpzip"},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	execText := execResult.Content[0].(*mcp.TextContent).Text
	var resp map[string]any
	if err := json.Unmarshal([]byte(execText), &resp); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if resp["status"] != "success" {
		t.Errorf("expected success, got %v", resp["status"])
	}
}

// TestMCP_ServerInstructions verifies the server sends instructions.
func TestMCP_ServerInstructions(t *testing.T) {
	session := connectMCPProxy(t)

	initResult := session.InitializeResult()
	if initResult == nil {
		t.Fatal("no initialize result")
	}
	if initResult.Instructions == "" {
		t.Error("expected non-empty instructions")
	}
	if !strings.Contains(initResult.Instructions, "mcpzip") {
		t.Errorf("expected instructions to mention mcpzip, got: %s", initResult.Instructions)
	}
}

// TestMCP_SearchToolsError verifies error handling over MCP protocol.
func TestMCP_SearchToolsError(t *testing.T) {
	session := connectMCPProxy(t)
	ctx := context.Background()

	// Empty query should return an error (via IsError, not protocol error).
	result, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      "search_tools",
		Arguments: map[string]any{"query": ""},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !result.IsError {
		t.Error("expected IsError=true for empty query")
	}
}
