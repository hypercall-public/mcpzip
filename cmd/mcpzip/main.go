// mcpzip is an MCP proxy that aggregates multiple upstream MCP servers
// and exposes them via a Search + Execute pattern. Instead of loading hundreds
// of tool schemas into context, Claude uses 3 meta-tools (search_tools,
// describe_tool, execute_tool) to discover and invoke upstream tools on demand.
//
// Usage:
//
//	mcpzip serve     Start the MCP proxy server
//	mcpzip init      Interactive setup wizard
//	mcpzip migrate   Auto-migrate from Claude Code config
//
// See https://hypercall.xyz and https://github.com/hypercall-public/mcpzip
// for documentation.

package main

import (
	"fmt"
	"os"

	"github.com/hypercall-public/mcpzip/internal/cli"
)

func main() {
	if err := cli.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}
