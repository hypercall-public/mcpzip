// https://hypercall.xyz

package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/hypercall-public/mcpzip/internal/types"
)

const (
	configDir  = "compressed-mcp-proxy"
	configFile = "config.json"
	cacheDir   = "cache"
	cacheFile  = "tools.json"
)

// DefaultPath returns the default config file path:
// ~/.config/compressed-mcp-proxy/config.json
func DefaultPath() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", configDir, configFile)
}

// CachePath returns the default cache file path:
// ~/.config/compressed-mcp-proxy/cache/tools.json
func CachePath() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", configDir, cacheDir, cacheFile)
}

// Load reads and validates a proxy config from the given path.
func Load(path string) (*types.ProxyConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading config: %w", err)
	}

	var cfg types.ProxyConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parsing config: %w", err)
	}

	if err := validate(&cfg); err != nil {
		return nil, fmt.Errorf("invalid config: %w", err)
	}

	return &cfg, nil
}

func validate(cfg *types.ProxyConfig) error {
	if len(cfg.MCPServers) == 0 {
		return fmt.Errorf("at least one MCP server must be defined")
	}
	for name, sc := range cfg.MCPServers {
		switch sc.EffectiveType() {
		case "stdio":
			if sc.Command == "" {
				return fmt.Errorf("server %q: stdio server must have a command", name)
			}
		case "http":
			if sc.URL == "" {
				return fmt.Errorf("server %q: http server must have a url", name)
			}
		default:
			return fmt.Errorf("server %q: unsupported type %q (must be \"stdio\" or \"http\")", name, sc.Type)
		}
	}
	return nil
}

// ClaudeCodeConfig represents the relevant portion of Claude Code's config.
type ClaudeCodeConfig struct {
	MCPServers map[string]types.ServerConfig `json:"mcpServers"`
}

// LoadClaudeCodeConfig reads and parses Claude Code's MCP server configuration.
// It checks common config locations in order.
func LoadClaudeCodeConfig() (*ClaudeCodeConfig, error) {
	paths := claudeCodeConfigPaths()
	for _, p := range paths {
		cfg, err := LoadClaudeCodeConfigFrom(p)
		if err != nil {
			continue
		}
		return cfg, nil
	}
	return nil, fmt.Errorf("no Claude Code config found with MCP servers")
}

// LoadClaudeCodeConfigFrom reads and parses Claude Code config from a specific path.
func LoadClaudeCodeConfigFrom(path string) (*ClaudeCodeConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading claude config: %w", err)
	}
	var cfg ClaudeCodeConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parsing claude config: %w", err)
	}
	if len(cfg.MCPServers) == 0 {
		return nil, fmt.Errorf("no MCP servers found in %s", path)
	}
	return &cfg, nil
}

func claudeCodeConfigPaths() []string {
	home, _ := os.UserHomeDir()
	return []string{
		filepath.Join(home, ".claude.json"),
		filepath.Join(home, ".claude", "config.json"),
	}
}
