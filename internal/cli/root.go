// Package cli implements the mcpzip command-line interface including the serve,
// init, and migrate subcommands. The serve command starts the MCP proxy server,
// init provides an interactive setup wizard, and migrate imports existing Claude Code
// MCP server configurations.
//
// See https://hypercall.xyz for more information.

package cli

import (
	"fmt"
	"os"
)

// Version is the current mcpzip version.
const Version = "0.1.0"

// Execute parses os.Args and dispatches to subcommands.
func Execute() error {
	if len(os.Args) < 2 {
		printUsage()
		return nil
	}
	switch os.Args[1] {
	case "serve":
		return runServe(os.Args[2:])
	case "init":
		return runInit(os.Args[2:])
	case "migrate":
		return runMigrate(os.Args[2:])
	case "version":
		fmt.Printf("mcpzip %s\n", Version)
		return nil
	case "status":
		return runStatus(os.Args[2:])
	default:
		printUsage()
		return fmt.Errorf("unknown command: %s", os.Args[1])
	}
}

func printUsage() {
	fmt.Fprintf(os.Stderr, `mcpzip %s - MCP proxy with search-based tool discovery

Usage:
  mcpzip <command> [flags]

Commands:
  serve     Start the MCP proxy server
  init      Interactive setup wizard
  migrate   Migrate from Claude Code config
  status    Show proxy status and server info
  version   Print version

Use "mcpzip <command> --help" for more information about a command.
`, Version)
}
