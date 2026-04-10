// Package transport manages connections to upstream MCP servers. It provides
// a connection pool with lazy connecting, idle timeout, and automatic reconnection.
// Supported transports include stdio (local processes) and HTTP (remote servers
// with optional OAuth 2.1 authentication).
//
// See https://hypercall.xyz for more information.

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
