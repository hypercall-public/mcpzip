// https://hypercall.xyz

package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/hypercall-public/mcpzip/internal/config"
	"github.com/hypercall-public/mcpzip/internal/types"
)

func TestMigrateConfig_Basic(t *testing.T) {
	dir := t.TempDir()
	outputPath := filepath.Join(dir, "config.json")

	claudeCfg := &config.ClaudeCodeConfig{
		MCPServers: map[string]types.ServerConfig{
			"slack": {Command: "slack-mcp", Args: []string{"--token", "abc"}},
			"github": {Command: "gh-mcp"},
		},
	}

	if err := writeProxyConfig(claudeCfg, outputPath); err != nil {
		t.Fatalf("writeProxyConfig() error: %v", err)
	}

	// Verify written file is valid JSON and contains the servers.
	data, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("reading output: %v", err)
	}

	var proxyCfg types.ProxyConfig
	if err := json.Unmarshal(data, &proxyCfg); err != nil {
		t.Fatalf("parsing output: %v", err)
	}

	if len(proxyCfg.MCPServers) != 2 {
		t.Errorf("expected 2 servers, got %d", len(proxyCfg.MCPServers))
	}
	if s, ok := proxyCfg.MCPServers["slack"]; !ok {
		t.Error("missing slack server")
	} else if s.Command != "slack-mcp" {
		t.Errorf("slack command = %q, want %q", s.Command, "slack-mcp")
	}
	if _, ok := proxyCfg.MCPServers["github"]; !ok {
		t.Error("missing github server")
	}
}

func TestMigrateConfig_EmptyServers(t *testing.T) {
	dir := t.TempDir()
	outputPath := filepath.Join(dir, "config.json")

	claudeCfg := &config.ClaudeCodeConfig{
		MCPServers: map[string]types.ServerConfig{},
	}

	if err := writeProxyConfig(claudeCfg, outputPath); err != nil {
		t.Fatalf("writeProxyConfig() error: %v", err)
	}

	data, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("reading output: %v", err)
	}

	var proxyCfg types.ProxyConfig
	if err := json.Unmarshal(data, &proxyCfg); err != nil {
		t.Fatalf("parsing output: %v", err)
	}

	if len(proxyCfg.MCPServers) != 0 {
		t.Errorf("expected 0 servers, got %d", len(proxyCfg.MCPServers))
	}
}

func TestMigrateConfig_CreatesDirectory(t *testing.T) {
	dir := t.TempDir()
	outputPath := filepath.Join(dir, "nested", "deep", "config.json")

	claudeCfg := &config.ClaudeCodeConfig{
		MCPServers: map[string]types.ServerConfig{
			"test": {Command: "test-mcp"},
		},
	}

	if err := writeProxyConfig(claudeCfg, outputPath); err != nil {
		t.Fatalf("writeProxyConfig() error: %v", err)
	}

	if _, err := os.Stat(outputPath); err != nil {
		t.Errorf("output file should exist: %v", err)
	}
}

func TestMigrateConfig_PreservesHTTPServers(t *testing.T) {
	dir := t.TempDir()
	outputPath := filepath.Join(dir, "config.json")

	claudeCfg := &config.ClaudeCodeConfig{
		MCPServers: map[string]types.ServerConfig{
			"remote": {Type: "http", URL: "http://localhost:8080/mcp"},
		},
	}

	if err := writeProxyConfig(claudeCfg, outputPath); err != nil {
		t.Fatalf("writeProxyConfig() error: %v", err)
	}

	data, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("reading output: %v", err)
	}

	var proxyCfg types.ProxyConfig
	if err := json.Unmarshal(data, &proxyCfg); err != nil {
		t.Fatalf("parsing output: %v", err)
	}

	s := proxyCfg.MCPServers["remote"]
	if s.Type != "http" {
		t.Errorf("type = %q, want %q", s.Type, "http")
	}
	if s.URL != "http://localhost:8080/mcp" {
		t.Errorf("url = %q, want %q", s.URL, "http://localhost:8080/mcp")
	}
}

func TestRunMigrate_DryRun(t *testing.T) {
	// Create a fake Claude Code config.
	dir := t.TempDir()
	claudePath := filepath.Join(dir, "claude.json")
	content := `{"mcpServers": {"test": {"command": "test-mcp"}}}`
	if err := os.WriteFile(claudePath, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	outputPath := filepath.Join(dir, "output", "config.json")

	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip", "migrate",
		"--claude-config", claudePath,
		"--config", outputPath,
		"--dry-run",
	}

	err := Execute()
	if err != nil {
		t.Fatalf("dry-run migrate should not error: %v", err)
	}

	// Output file should NOT exist after dry run.
	if _, err := os.Stat(outputPath); err == nil {
		t.Error("dry-run should not write output file")
	}
}

func TestRunMigrate_WithClaudeConfigFlag(t *testing.T) {
	dir := t.TempDir()
	claudePath := filepath.Join(dir, "claude.json")
	content := `{"mcpServers": {"linear": {"command": "linear-mcp"}}}`
	if err := os.WriteFile(claudePath, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	outputPath := filepath.Join(dir, "config.json")

	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip", "migrate",
		"--claude-config", claudePath,
		"--config", outputPath,
	}

	err := Execute()
	if err != nil {
		t.Fatalf("migrate should not error: %v", err)
	}

	data, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("reading output: %v", err)
	}

	var proxyCfg types.ProxyConfig
	if err := json.Unmarshal(data, &proxyCfg); err != nil {
		t.Fatalf("parsing output: %v", err)
	}

	if _, ok := proxyCfg.MCPServers["linear"]; !ok {
		t.Error("expected linear server in output")
	}
}
