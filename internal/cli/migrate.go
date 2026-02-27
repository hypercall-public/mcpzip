// https://hypercall.xyz

package cli

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/hypercall-public/mcpzip/internal/config"
	"github.com/hypercall-public/mcpzip/internal/types"
)

func runMigrate(args []string) error {
	fs := flag.NewFlagSet("migrate", flag.ContinueOnError)
	outputPath := fs.String("config", config.DefaultPath(), "output config file path")
	claudeConfigPath := fs.String("claude-config", "", "path to Claude Code config (auto-detected if empty)")
	dryRun := fs.Bool("dry-run", false, "show what would happen without writing files")
	if err := fs.Parse(args); err != nil {
		return err
	}

	// Load Claude Code config.
	var claudeCfg *config.ClaudeCodeConfig
	var err error
	if *claudeConfigPath != "" {
		claudeCfg, err = config.LoadClaudeCodeConfigFrom(*claudeConfigPath)
	} else {
		claudeCfg, err = config.LoadClaudeCodeConfig()
	}
	if err != nil {
		return fmt.Errorf("loading Claude Code config: %w", err)
	}

	if *dryRun {
		fmt.Printf("Dry run: would migrate %d server(s) to %s\n", len(claudeCfg.MCPServers), *outputPath)
		for name, sc := range claudeCfg.MCPServers {
			fmt.Printf("  - %s (%s)\n", name, sc.EffectiveType())
		}
		return nil
	}

	return migrateConfig(claudeCfg, *outputPath)
}

// migrateConfig converts a ClaudeCodeConfig into a ProxyConfig and writes it to outputPath.
func migrateConfig(claudeCfg *config.ClaudeCodeConfig, outputPath string) error {
	proxyCfg := types.ProxyConfig{
		MCPServers: make(map[string]types.ServerConfig, len(claudeCfg.MCPServers)),
	}
	for name, sc := range claudeCfg.MCPServers {
		proxyCfg.MCPServers[name] = sc
	}

	data, err := json.MarshalIndent(proxyCfg, "", "  ")
	if err != nil {
		return fmt.Errorf("marshaling config: %w", err)
	}
	data = append(data, '\n')

	// Create config directory if needed.
	dir := filepath.Dir(outputPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("creating config directory: %w", err)
	}

	if err := os.WriteFile(outputPath, data, 0600); err != nil {
		return fmt.Errorf("writing config: %w", err)
	}

	fmt.Printf("Migrated %d server(s) to %s\n", len(proxyCfg.MCPServers), outputPath)
	for name, sc := range proxyCfg.MCPServers {
		fmt.Printf("  - %s (%s)\n", name, sc.EffectiveType())
	}
	return nil
}
