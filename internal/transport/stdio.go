// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"sync"

	"github.com/hypercall-public/mcpzip/internal/types"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// StdioUpstream connects to an upstream MCP server via stdio using the go-sdk.
type StdioUpstream struct {
	mu      sync.Mutex
	session *mcp.ClientSession
	client  *mcp.Client
	alive   bool
}

// NewStdioUpstream creates and connects to an upstream stdio MCP server.
func NewStdioUpstream(ctx context.Context, name string, cfg types.ServerConfig) (*StdioUpstream, error) {
	client := mcp.NewClient(
		&mcp.Implementation{Name: "mcpzip", Version: "0.1.0"},
		nil,
	)

	cmd := exec.CommandContext(ctx, cfg.Command, cfg.Args...)
	if len(cfg.Env) > 0 {
		cmd.Env = os.Environ()
		for k, v := range cfg.Env {
			cmd.Env = append(cmd.Env, k+"="+v)
		}
	}

	session, err := client.Connect(ctx, &mcp.CommandTransport{Command: cmd}, nil)
	if err != nil {
		return nil, fmt.Errorf("connecting to %q: %w", name, err)
	}

	return &StdioUpstream{
		session: session,
		client:  client,
		alive:   true,
	}, nil
}

func (s *StdioUpstream) ListTools(ctx context.Context) ([]types.ToolEntry, error) {
	s.mu.Lock()
	session := s.session
	s.mu.Unlock()
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
			OriginalName: t.Name,
			Description:  t.Description,
			InputSchema:  schemaBytes,
			CompactParams: types.CompactParamsFromSchema(schemaBytes),
		})
	}
	return entries, nil
}

func (s *StdioUpstream) CallTool(ctx context.Context, toolName string, args json.RawMessage) (json.RawMessage, error) {
	s.mu.Lock()
	session := s.session
	s.mu.Unlock()
	if session == nil {
		return nil, fmt.Errorf("connection closed")
	}

	// Convert json.RawMessage args to map[string]any for the SDK.
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

	// Convert CallToolResult back to JSON.
	// If the result has structured content, use that.
	if result.StructuredContent != nil {
		return json.Marshal(result.StructuredContent)
	}

	// Otherwise, extract text content.
	if len(result.Content) == 1 {
		if tc, ok := result.Content[0].(*mcp.TextContent); ok {
			// Try to parse as JSON first; if it's already JSON, return as-is.
			if json.Valid([]byte(tc.Text)) {
				return json.RawMessage(tc.Text), nil
			}
			return json.Marshal(tc.Text)
		}
	}

	// Fallback: marshal the entire content array.
	return json.Marshal(result.Content)
}

func (s *StdioUpstream) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.alive = false
	if s.session != nil {
		err := s.session.Close()
		s.session = nil
		return err
	}
	return nil
}

func (s *StdioUpstream) Alive() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.alive && s.session != nil
}
