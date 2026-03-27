// https://hypercall.xyz

package proxy

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/jake/mcpzip/internal/catalog"
	"github.com/jake/mcpzip/internal/search"
	"github.com/jake/mcpzip/internal/transport"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Server is the core MCP proxy that exposes 3 meta-tools.
type Server struct {
	catalog   *catalog.Catalog
	searcher  search.Searcher
	transport *transport.Manager
}

// New creates a new proxy server.
func New(cat *catalog.Catalog, searcher search.Searcher, tm *transport.Manager) *Server {
	return &Server{
		catalog:   cat,
		searcher:  searcher,
		transport: tm,
	}
}

// Run starts the MCP server over stdio and blocks until the context is
// cancelled or the client disconnects.
func (s *Server) Run(ctx context.Context) error {
	return s.RunWithTransport(ctx, &mcp.StdioTransport{})
}

// RunWithTransport starts the MCP server using the given transport.
// This is useful for testing with in-memory transports.
func (s *Server) RunWithTransport(ctx context.Context, t mcp.Transport) error {
	server := mcp.NewServer(
		&mcp.Implementation{Name: "mcpzip", Version: "0.1.0"},
		&mcp.ServerOptions{
			Instructions: s.Instructions(),
		},
	)

	s.registerTools(server)

	return server.Run(ctx, t)
}

func (s *Server) registerTools(server *mcp.Server) {
	// search_tools: discover tools by keyword query
	server.AddTool(
		&mcp.Tool{
			Name:        "search_tools",
			Description: "Search for available tools by keyword query. Returns matching tool names, descriptions, and parameter summaries.",
			InputSchema: json.RawMessage(`{
				"type": "object",
				"properties": {
					"query": {
						"type": "string",
						"description": "Search query to find tools (e.g. 'send message', 'list channels')"
					},
					"limit": {
						"type": "integer",
						"description": "Maximum number of results to return (default: 5, max: 50)"
					}
				},
				"required": ["query"]
			}`),
		},
		func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			result, err := s.HandleSearchTools(ctx, req.Params.Arguments)
			if err != nil {
				return &mcp.CallToolResult{
					Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("Error: %v", err)}},
					IsError: true,
				}, nil
			}
			return &mcp.CallToolResult{
				Content: []mcp.Content{&mcp.TextContent{Text: result}},
			}, nil
		},
	)

	// describe_tool: get full schema for a specific tool
	server.AddTool(
		&mcp.Tool{
			Name:        "describe_tool",
			Description: "Get the full description and input schema for a specific tool. Use the prefixed name from search_tools results.",
			InputSchema: json.RawMessage(`{
				"type": "object",
				"properties": {
					"name": {
						"type": "string",
						"description": "The prefixed tool name (e.g. 'slack__send_message')"
					}
				},
				"required": ["name"]
			}`),
		},
		func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			result, err := s.HandleDescribeTool(ctx, req.Params.Arguments)
			if err != nil {
				return &mcp.CallToolResult{
					Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("Error: %v", err)}},
					IsError: true,
				}, nil
			}
			return &mcp.CallToolResult{
				Content: []mcp.Content{&mcp.TextContent{Text: result}},
			}, nil
		},
	)

	// execute_tool: invoke a tool on its upstream server
	server.AddTool(
		&mcp.Tool{
			Name:        "execute_tool",
			Description: "Execute a tool on its upstream MCP server. Use the prefixed name from search_tools results and provide the required arguments.",
			InputSchema: json.RawMessage(`{
				"type": "object",
				"properties": {
					"name": {
						"type": "string",
						"description": "The prefixed tool name (e.g. 'slack__send_message')"
					},
					"arguments": {
						"type": "object",
						"description": "Arguments to pass to the tool"
					},
					"timeout": {
						"type": "integer",
						"description": "Timeout in seconds for this tool call (default: no per-call timeout)"
					}
				},
				"required": ["name"]
			}`),
		},
		func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			result, err := s.HandleExecuteTool(ctx, req.Params.Arguments)
			if err != nil {
				return &mcp.CallToolResult{
					Content: []mcp.Content{&mcp.TextContent{Text: fmt.Sprintf("Error: %v", err)}},
					IsError: true,
				}, nil
			}
			return &mcp.CallToolResult{
				Content: []mcp.Content{&mcp.TextContent{Text: string(result)}},
			}, nil
		},
	)
}
