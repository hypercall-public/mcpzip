// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// Upstream represents a connection to an upstream MCP server.
type Upstream interface {
	// ListTools returns all tools from this upstream.
	ListTools(ctx context.Context) ([]types.ToolEntry, error)
	// CallTool invokes a tool on this upstream and returns the raw JSON result.
	CallTool(ctx context.Context, toolName string, args json.RawMessage) (json.RawMessage, error)
	// Close shuts down the connection.
	Close() error
	// Alive checks if the connection is still usable.
	Alive() bool
}
