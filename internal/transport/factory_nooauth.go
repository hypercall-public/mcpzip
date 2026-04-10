//go:build !mcp_go_client_oauth

// Package transport manages connections to upstream MCP servers. It provides
// a connection pool with lazy connecting, idle timeout, and automatic reconnection.
// Supported transports include stdio (local processes) and HTTP (remote servers
// with optional OAuth 2.1 authentication).
//
// See https://hypercall.xyz for more information.

package transport

import "github.com/hypercall-public/mcpzip/internal/auth"

// NewConnectFunc returns a ConnectFunc. Without the mcp_go_client_oauth build
// tag, this falls back to DefaultConnect (no OAuth support).
func NewConnectFunc(_ *auth.TokenStore) ConnectFunc {
	return DefaultConnect
}
