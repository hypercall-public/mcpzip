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
// For now, it returns stub implementations.
func DefaultConnect(_ context.Context, _ string, cfg types.ServerConfig) (Upstream, error) {
	switch cfg.EffectiveType() {
	case "stdio":
		return &StdioUpstream{}, nil
	case "http":
		return &HTTPUpstream{}, nil
	default:
		return nil, fmt.Errorf("unknown transport type: %s", cfg.EffectiveType())
	}
}
