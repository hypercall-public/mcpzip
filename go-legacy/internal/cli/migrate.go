// https://hypercall.xyz

package cli

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/jake/mcpzip/internal/config"
	"github.com/jake/mcpzip/internal/types"
)

func runMigrate(args []string) error {
	fs := flag.NewFlagSet("migrate", flag.ContinueOnError)
	outputPath := fs.String("config", config.DefaultPath(), "output config file path")
	claudeConfigPath := fs.String("claude-config", "", "path to Claude Code config (auto-detected if empty)")
	dryRun := fs.Bool("dry-run", false, "show what would happen without writing files")
	if err := fs.Parse(args); err != nil {
		return err
	}

	// Find the Claude Code config path.
	claudePath := *claudeConfigPath
	if claudePath == "" {
		var err error
		claudePath, err = config.FindClaudeCodeConfigPath()
		if err != nil {
			return fmt.Errorf("finding Claude Code config: %w", err)
		}
	}

	// Load Claude Code config.
	claudeCfg, err := config.LoadClaudeCodeConfigFrom(claudePath)
	if err != nil {
		return fmt.Errorf("loading Claude Code config: %w", err)
	}

	// Find the mcpzip binary path for the Claude Code config entry.
	mcpzipBin, err := os.Executable()
	if err != nil {
		mcpzipBin = "mcpzip"
	}

	if *dryRun {
		fmt.Printf("Dry run: would migrate %d server(s) from %s\n\n", len(claudeCfg.MCPServers), claudePath)
		fmt.Printf("1. Write mcpzip config to %s:\n", *outputPath)
		for name, sc := range claudeCfg.MCPServers {
			fmt.Printf("   - %s (%s)\n", name, sc.EffectiveType())
		}
		fmt.Printf("\n2. Update %s:\n", claudePath)
		fmt.Printf("   - Remove %d individual server entries\n", len(claudeCfg.MCPServers))
		fmt.Printf("   - Add single \"mcpzip\" entry pointing to %s\n", mcpzipBin)
		return nil
	}

	// Step 1: Write the mcpzip proxy config.
	if err := writeProxyConfig(claudeCfg, *outputPath); err != nil {
		return err
	}

	// Step 2: Update the Claude Code config.
	if err := updateClaudeConfig(claudePath, claudeCfg, mcpzipBin); err != nil {
		return err
	}

	fmt.Printf("\nDone! Restart Claude Code to use mcpzip.\n")
	return nil
}

// writeProxyConfig writes the mcpzip config file with all the migrated servers.
func writeProxyConfig(claudeCfg *config.ClaudeCodeConfig, outputPath string) error {
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

	dir := filepath.Dir(outputPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("creating config directory: %w", err)
	}

	if err := os.WriteFile(outputPath, data, 0600); err != nil {
		return fmt.Errorf("writing config: %w", err)
	}

	fmt.Printf("Wrote mcpzip config to %s (%d servers)\n", outputPath, len(proxyCfg.MCPServers))
	for name, sc := range proxyCfg.MCPServers {
		fmt.Printf("  - %s (%s)\n", name, sc.EffectiveType())
	}
	return nil
}

// updateClaudeConfig replaces all mcpServers in the Claude Code config with
// a single mcpzip entry. Preserves all other fields in the config.
func updateClaudeConfig(claudePath string, claudeCfg *config.ClaudeCodeConfig, mcpzipBin string) error {
	// Read the raw JSON to preserve all non-mcpServers fields.
	data, err := os.ReadFile(claudePath)
	if err != nil {
		return fmt.Errorf("reading %s: %w", claudePath, err)
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		return fmt.Errorf("parsing %s: %w", claudePath, err)
	}

	// Replace mcpServers with single mcpzip entry.
	newServers := map[string]types.ServerConfig{
		"mcpzip": {
			Type:    "stdio",
			Command: mcpzipBin,
			Args:    []string{"serve"},
		},
	}
	serversJSON, err := json.Marshal(newServers)
	if err != nil {
		return fmt.Errorf("marshaling new mcpServers: %w", err)
	}
	raw["mcpServers"] = serversJSON

	out, err := json.MarshalIndent(raw, "", "  ")
	if err != nil {
		return fmt.Errorf("marshaling updated config: %w", err)
	}
	out = append(out, '\n')

	if err := os.WriteFile(claudePath, out, 0600); err != nil {
		return fmt.Errorf("writing %s: %w", claudePath, err)
	}

	fmt.Printf("\nUpdated %s:\n", claudePath)
	fmt.Printf("  Replaced %d servers with single mcpzip entry\n", len(claudeCfg.MCPServers))
	return nil
}
