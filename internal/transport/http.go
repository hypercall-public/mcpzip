// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// HTTPUpstream is a stub for HTTP MCP connections.
// The real implementation will use the go-sdk MCP client.
type HTTPUpstream struct{}

func (h *HTTPUpstream) ListTools(_ context.Context) ([]types.ToolEntry, error) {
	return nil, fmt.Errorf("http transport not yet connected")
}

func (h *HTTPUpstream) CallTool(_ context.Context, _ string, _ json.RawMessage) (json.RawMessage, error) {
	return nil, fmt.Errorf("http transport not yet connected")
}

func (h *HTTPUpstream) Close() error {
	return fmt.Errorf("http transport not yet connected")
}

func (h *HTTPUpstream) Alive() bool {
	return false
}
