// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"

	"github.com/jake/mcpzip/internal/types"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// HTTPUpstream connects to an upstream MCP server via HTTP (streamable HTTP or SSE)
// using the go-sdk.
type HTTPUpstream struct {
	mu      sync.Mutex
	session *mcp.ClientSession
	client  *mcp.Client
	alive   bool
}

// NewHTTPUpstream creates and connects to an upstream HTTP MCP server (streamable HTTP).
func NewHTTPUpstream(ctx context.Context, name string, cfg types.ServerConfig) (*HTTPUpstream, error) {
	return newHTTPUpstream(ctx, name, cfg, &mcp.StreamableClientTransport{Endpoint: cfg.URL})
}

// NewSSEUpstream creates and connects to an upstream SSE MCP server.
func NewSSEUpstream(ctx context.Context, name string, cfg types.ServerConfig) (*HTTPUpstream, error) {
	return newHTTPUpstream(ctx, name, cfg, &mcp.SSEClientTransport{Endpoint: cfg.URL})
}

func newHTTPUpstream(ctx context.Context, name string, cfg types.ServerConfig, transport mcp.Transport) (*HTTPUpstream, error) {
	client := mcp.NewClient(
		&mcp.Implementation{Name: "mcpzip", Version: "0.1.0"},
		nil,
	)

	session, err := client.Connect(ctx, transport, nil)
	if err != nil {
		return nil, fmt.Errorf("connecting to %q at %s: %w", name, cfg.URL, err)
	}

	return &HTTPUpstream{
		session: session,
		client:  client,
		alive:   true,
	}, nil
}

func (h *HTTPUpstream) ListTools(ctx context.Context) ([]types.ToolEntry, error) {
	h.mu.Lock()
	session := h.session
	h.mu.Unlock()
	if session == nil {
		return nil, fmt.Errorf("connection closed")
	}

	var allTools []*mcp.Tool
	for tool, err := range session.Tools(ctx, nil) {
		if err != nil {
			return nil, fmt.Errorf("listing tools: %w", err)
		}
		allTools = append(allTools, tool)
	}

	entries := make([]types.ToolEntry, 0, len(allTools))
	for _, t := range allTools {
		schemaBytes, err := json.Marshal(t.InputSchema)
		if err != nil {
			schemaBytes = []byte(`{"type":"object"}`)
		}
		entries = append(entries, types.ToolEntry{
			OriginalName:  t.Name,
			Description:   t.Description,
			InputSchema:   schemaBytes,
			CompactParams: types.CompactParamsFromSchema(schemaBytes),
		})
	}
	return entries, nil
}

func (h *HTTPUpstream) CallTool(ctx context.Context, toolName string, args json.RawMessage) (json.RawMessage, error) {
	h.mu.Lock()
	session := h.session
	h.mu.Unlock()
	if session == nil {
		return nil, fmt.Errorf("connection closed")
	}

	var argsMap map[string]any
	if len(args) > 0 {
		if err := json.Unmarshal(args, &argsMap); err != nil {
			return nil, fmt.Errorf("unmarshaling tool arguments: %w", err)
		}
	}

	result, err := session.CallTool(ctx, &mcp.CallToolParams{
		Name:      toolName,
		Arguments: argsMap,
	})
	if err != nil {
		return nil, err
	}

	if result.StructuredContent != nil {
		return json.Marshal(result.StructuredContent)
	}

	if len(result.Content) == 1 {
		if tc, ok := result.Content[0].(*mcp.TextContent); ok {
			if json.Valid([]byte(tc.Text)) {
				return json.RawMessage(tc.Text), nil
			}
			return json.Marshal(tc.Text)
		}
	}

	return json.Marshal(result.Content)
}

func (h *HTTPUpstream) Close() error {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.alive = false
	if h.session != nil {
		err := h.session.Close()
		h.session = nil
		return err
	}
	return nil
}

func (h *HTTPUpstream) Alive() bool {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.alive && h.session != nil
}
