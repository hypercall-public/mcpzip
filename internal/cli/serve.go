// https://hypercall.xyz

package cli

import (
	"flag"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/config"
)

func runServe(args []string) error {
	fs := flag.NewFlagSet("serve", flag.ContinueOnError)
	configPath := fs.String("config", config.DefaultPath(), "path to config file")
	if err := fs.Parse(args); err != nil {
		return err
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	fmt.Printf("Starting mcpzip proxy...\n")
	fmt.Printf("  Config: %s\n", *configPath)
	fmt.Printf("  Servers: %d\n", len(cfg.MCPServers))
	for name, sc := range cfg.MCPServers {
		fmt.Printf("    - %s (%s)\n", name, sc.EffectiveType())
	}

	// Actual proxy wiring comes in integration phase (Task I4).
	return nil
}
