// https://hypercall.xyz

package proxy

import (
	"fmt"
	"strings"
)

// Instructions returns summarized instructions for the proxy.
// If server instructions are available, they are concatenated with headers.
// LLM summarization will be added when Gemini integration is complete.
func (s *Server) Instructions() string {
	serverNames := s.catalog.ServerNames()
	if len(serverNames) == 0 {
		return "mcpzip proxy - use search_tools to discover available tools."
	}

	var sb strings.Builder
	sb.WriteString("mcpzip proxy aggregates tools from the following servers:\n")
	for _, name := range serverNames {
		tools := s.catalog.ServerTools(name)
		sb.WriteString(fmt.Sprintf("- %s (%d tools)\n", name, len(tools)))
	}
	sb.WriteString("\nUse search_tools to discover tools, describe_tool for details, execute_tool to invoke them.")
	return sb.String()
}
