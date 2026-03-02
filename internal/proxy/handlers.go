// https://hypercall.xyz

package proxy

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// SearchToolsArgs is the input for the search_tools meta-tool.
type SearchToolsArgs struct {
	Query string `json:"query"`
	Limit int    `json:"limit,omitempty"`
}

// DescribeToolArgs is the input for the describe_tool meta-tool.
type DescribeToolArgs struct {
	Name string `json:"name"`
}

// ExecuteToolArgs is the input for the execute_tool meta-tool.
type ExecuteToolArgs struct {
	Name      string          `json:"name"`
	Arguments json.RawMessage `json:"arguments"`
	Timeout   int             `json:"timeout,omitempty"` // seconds
}

const (
	defaultSearchLimit = 5
	maxSearchLimit     = 50
)

// HandleSearchTools implements the search_tools meta-tool.
func (s *Server) HandleSearchTools(ctx context.Context, rawArgs json.RawMessage) (string, error) {
	var args SearchToolsArgs
	if err := json.Unmarshal(rawArgs, &args); err != nil {
		return "", fmt.Errorf("invalid search_tools arguments: %w", err)
	}
	if args.Query == "" {
		return "", fmt.Errorf("query is required")
	}
	limit := args.Limit
	if limit <= 0 {
		limit = defaultSearchLimit
	} else if limit > maxSearchLimit {
		limit = maxSearchLimit
	}

	results, err := s.searcher.Search(ctx, args.Query, limit)
	if err != nil {
		return "", fmt.Errorf("search failed: %w", err)
	}

	if len(results) == 0 {
		return "No tools found matching your query.", nil
	}

	var sb strings.Builder
	for i, r := range results {
		if i > 0 {
			sb.WriteString("\n\n")
		}
		sb.WriteString(fmt.Sprintf("**%s**", r.Name))
		if r.Description != "" {
			sb.WriteString(fmt.Sprintf("\n%s", r.Description))
		}
		if r.CompactParams != "" {
			sb.WriteString(fmt.Sprintf("\nParams: %s", r.CompactParams))
		}
	}
	return sb.String(), nil
}

// HandleDescribeTool implements the describe_tool meta-tool.
func (s *Server) HandleDescribeTool(ctx context.Context, rawArgs json.RawMessage) (string, error) {
	var args DescribeToolArgs
	if err := json.Unmarshal(rawArgs, &args); err != nil {
		return "", fmt.Errorf("invalid describe_tool arguments: %w", err)
	}
	if args.Name == "" {
		return "", fmt.Errorf("name is required")
	}

	tool, err := s.catalog.GetTool(args.Name)
	if err != nil {
		return "", err
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("**%s**\n", tool.Name))
	sb.WriteString(fmt.Sprintf("Server: %s\n", tool.ServerName))
	sb.WriteString(fmt.Sprintf("Original name: %s\n", tool.OriginalName))
	if tool.Description != "" {
		sb.WriteString(fmt.Sprintf("\n%s\n", tool.Description))
	}
	if len(tool.InputSchema) > 0 {
		sb.WriteString("\nInput Schema:\n```json\n")
		// Pretty-print the schema.
		var pretty json.RawMessage
		if err := json.Unmarshal(tool.InputSchema, &pretty); err == nil {
			formatted, err := json.MarshalIndent(pretty, "", "  ")
			if err == nil {
				sb.Write(formatted)
			} else {
				sb.Write(tool.InputSchema)
			}
		} else {
			sb.Write(tool.InputSchema)
		}
		sb.WriteString("\n```")
	}
	return sb.String(), nil
}

// HandleExecuteTool implements the execute_tool meta-tool.
func (s *Server) HandleExecuteTool(ctx context.Context, rawArgs json.RawMessage) (json.RawMessage, error) {
	var args ExecuteToolArgs
	if err := json.Unmarshal(rawArgs, &args); err != nil {
		return nil, fmt.Errorf("invalid execute_tool arguments: %w", err)
	}
	if args.Name == "" {
		return nil, fmt.Errorf("name is required")
	}

	// LLMs sometimes double-encode arguments as a JSON string instead of an object.
	// Unwrap one level: "{\"key\":\"val\"}" → {"key":"val"}
	if len(args.Arguments) > 0 && args.Arguments[0] == '"' {
		var s string
		if err := json.Unmarshal(args.Arguments, &s); err == nil {
			args.Arguments = json.RawMessage(s)
		}
	}

	// Handle admin tools before catalog lookup (they may not be in the catalog).
	switch args.Name {
	case "proxy_status":
		return s.handleProxyStatus()
	case "proxy_refresh":
		return s.handleProxyRefresh(ctx)
	}

	// Verify tool exists in catalog.
	if _, err := s.catalog.GetTool(args.Name); err != nil {
		return nil, err
	}

	// Parse the prefixed name to get server and original tool name.
	serverName, toolName, err := types.ParsePrefixedName(args.Name)
	if err != nil {
		return nil, fmt.Errorf("invalid tool name %q: %w", args.Name, err)
	}

	callCtx := ctx
	if args.Timeout > 0 {
		var cancel context.CancelFunc
		callCtx, cancel = context.WithTimeout(ctx, time.Duration(args.Timeout)*time.Second)
		defer cancel()
	}

	result, err := s.transport.CallTool(callCtx, serverName, toolName, args.Arguments)
	if err != nil {
		return nil, fmt.Errorf("executing %q on %q: %w", toolName, serverName, err)
	}
	return result, nil
}

func (s *Server) handleProxyStatus() (json.RawMessage, error) {
	status := struct {
		ToolCount   int      `json:"tool_count"`
		ServerNames []string `json:"server_names"`
	}{
		ToolCount:   s.catalog.ToolCount(),
		ServerNames: s.catalog.ServerNames(),
	}
	data, err := json.Marshal(status)
	if err != nil {
		return nil, err
	}
	return data, nil
}

func (s *Server) handleProxyRefresh(ctx context.Context) (json.RawMessage, error) {
	if err := s.catalog.RefreshAll(ctx); err != nil {
		return nil, fmt.Errorf("refresh failed: %w", err)
	}
	result := struct {
		Status    string `json:"status"`
		ToolCount int    `json:"tool_count"`
	}{
		Status:    "refreshed",
		ToolCount: s.catalog.ToolCount(),
	}
	data, err := json.Marshal(result)
	if err != nil {
		return nil, err
	}
	return data, nil
}
