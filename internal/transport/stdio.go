// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// StdioUpstream is a stub for stdio MCP connections.
// The real implementation will use the go-sdk MCP client.
type StdioUpstream struct{}

func (s *StdioUpstream) ListTools(_ context.Context) ([]types.ToolEntry, error) {
	return nil, fmt.Errorf("stdio transport not yet connected")
}

func (s *StdioUpstream) CallTool(_ context.Context, _ string, _ json.RawMessage) (json.RawMessage, error) {
	return nil, fmt.Errorf("stdio transport not yet connected")
}

func (s *StdioUpstream) Close() error {
	return fmt.Errorf("stdio transport not yet connected")
}

func (s *StdioUpstream) Alive() bool {
	return false
}
