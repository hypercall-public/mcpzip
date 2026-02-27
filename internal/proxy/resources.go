// https://hypercall.xyz

package proxy

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// Resource represents an MCP resource from an upstream server.
type Resource struct {
	URI         string `json:"uri"`
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	MimeType    string `json:"mimeType,omitempty"`
	ServerName  string `json:"server_name"`
}

// Prompt represents an MCP prompt from an upstream server.
type Prompt struct {
	Name        string          `json:"name"`
	Description string          `json:"description,omitempty"`
	Arguments   json.RawMessage `json:"arguments,omitempty"`
	ServerName  string          `json:"server_name"`
}

// PrefixURI returns a server-prefixed URI.
func PrefixURI(server, uri string) string {
	return server + types.NameSeparator + uri
}

// ParsePrefixedURI splits a prefixed URI back into server and original URI.
func ParsePrefixedURI(prefixed string) (server, uri string, err error) {
	return types.ParsePrefixedName(prefixed)
}

// ListResources returns all resources aggregated from upstream servers.
// Currently returns empty since upstream connections don't support
// resource listing yet (pending MCP SDK integration).
func (s *Server) ListResources(ctx context.Context) ([]Resource, error) {
	// Will be populated when real MCP SDK connections support resources.
	return nil, nil
}

// ReadResource reads a resource by its prefixed URI,
// routing to the correct upstream server.
func (s *Server) ReadResource(ctx context.Context, prefixedURI string) (json.RawMessage, error) {
	server, _, err := ParsePrefixedURI(prefixedURI)
	if err != nil {
		return nil, fmt.Errorf("invalid resource URI %q: %w", prefixedURI, err)
	}
	// Verify server exists.
	_ = server
	return nil, fmt.Errorf("resource reading not yet implemented (pending MCP SDK integration)")
}

// ListPrompts returns all prompts aggregated from upstream servers.
func (s *Server) ListPrompts(ctx context.Context) ([]Prompt, error) {
	return nil, nil
}

// GetPrompt gets a prompt by its prefixed name.
func (s *Server) GetPrompt(ctx context.Context, prefixedName string) (json.RawMessage, error) {
	server, _, err := types.ParsePrefixedName(prefixedName)
	if err != nil {
		return nil, fmt.Errorf("invalid prompt name %q: %w", prefixedName, err)
	}
	_ = server
	return nil, fmt.Errorf("prompt retrieval not yet implemented (pending MCP SDK integration)")
}
