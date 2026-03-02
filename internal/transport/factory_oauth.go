//go:build mcp_go_client_oauth

// https://hypercall.xyz

package transport

import (
	"context"
	"fmt"
	"os"

	"github.com/hypercall-public/mcpzip/internal/auth"
	"github.com/hypercall-public/mcpzip/internal/types"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// NewConnectFunc returns a ConnectFunc that creates OAuth-capable connections
// for HTTP upstreams. Each HTTP upstream gets its own OAuth handler with a
// local callback server for the browser-based authorization flow.
func NewConnectFunc(store *auth.TokenStore) ConnectFunc {
	return func(ctx context.Context, name string, cfg types.ServerConfig) (Upstream, error) {
		switch cfg.EffectiveType() {
		case "stdio":
			return NewStdioUpstream(ctx, name, cfg)
		case "http":
			handler, _, err := auth.NewOAuthHandler(cfg.URL, store)
			if err != nil {
				fmt.Fprintf(os.Stderr, "mcpzip: oauth setup for %s: %v (continuing without auth)\n", name, err)
				return NewHTTPUpstream(ctx, name, cfg)
			}
			transport := &mcp.StreamableClientTransport{
				Endpoint:     cfg.URL,
				OAuthHandler: handler,
			}
			return newHTTPUpstream(ctx, name, cfg, transport)
		case "sse":
			return NewSSEUpstream(ctx, name, cfg)
		default:
			return nil, fmt.Errorf("unknown transport type: %s", cfg.EffectiveType())
		}
	}
}
