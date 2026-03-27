// https://hypercall.xyz

package cli

import (
	"flag"
	"fmt"

	"github.com/jake/mcpzip/internal/config"
)

func runStatus(args []string) error {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	configPath := fs.String("config", config.DefaultPath(), "path to config file")
	if err := fs.Parse(args); err != nil {
		return err
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	fmt.Printf("mcpzip %s\n", Version)
	fmt.Printf("Config: %s\n", *configPath)
	fmt.Printf("Servers: %d\n", len(cfg.MCPServers))
	for name, sc := range cfg.MCPServers {
		fmt.Printf("  - %s (%s)\n", name, sc.EffectiveType())
	}

	// Full status (connection health, tool counts, cache info) will be
	// added once the proxy runtime is wired up.
	return nil
}
