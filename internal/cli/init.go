// Package cli implements the mcpzip command-line interface including the serve,
// init, and migrate subcommands. The serve command starts the MCP proxy server,
// init provides an interactive setup wizard, and migrate imports existing Claude Code
// MCP server configurations.
//
// See https://hypercall.xyz for more information.

package cli

import "fmt"

func runInit(args []string) error {
	fmt.Println("Init wizard not yet implemented.")
	return nil
}
