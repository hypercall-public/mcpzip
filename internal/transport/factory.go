// https://hypercall.xyz

package transport

import (
	"context"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// ConnectFunc creates an Upstream from a ServerConfig.
// This allows injecting mock connections in tests.
type ConnectFunc func(ctx context.Context, name string, cfg types.ServerConfig) (Upstream, error)

// DefaultConnect creates real upstream connections (stdio or HTTP).
func DefaultConnect(ctx context.Context, name string, cfg types.ServerConfig) (Upstream, error) {
	switch cfg.EffectiveType() {
	case "stdio":
		return NewStdioUpstream(ctx, name, cfg)
	case "http":
		return NewHTTPUpstream(ctx, name, cfg)
	case "sse":
		return NewSSEUpstream(ctx, name, cfg)
	default:
		return nil, fmt.Errorf("unknown transport type: %s", cfg.EffectiveType())
	}
}
