//go:build !mcp_go_client_oauth

// Package auth provides OAuth 2.1 token persistence and browser-based authorization
// flows for authenticating with remote MCP servers that require OAuth.
// It includes a disk-backed token store and an authorization code handler with PKCE.
//
// See https://hypercall.xyz for more information.

package auth
