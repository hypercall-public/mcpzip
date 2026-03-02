//go:build !mcp_go_client_oauth

// https://hypercall.xyz

package transport

import "github.com/hypercall-public/mcpzip/internal/auth"

// NewConnectFunc returns a ConnectFunc. Without the mcp_go_client_oauth build
// tag, this falls back to DefaultConnect (no OAuth support).
func NewConnectFunc(_ *auth.TokenStore) ConnectFunc {
	return DefaultConnect
}
