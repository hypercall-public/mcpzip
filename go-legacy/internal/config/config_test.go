// https://hypercall.xyz

package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/jake/mcpzip/internal/types"
)

func writeConfig(t *testing.T, dir, content string) string {
	t.Helper()
	p := filepath.Join(dir, "config.json")
	if err := os.WriteFile(p, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
	return p
}

func TestLoad_ValidStdio(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"mcpServers": {
			"test": {"command": "echo", "args": ["hello"]}
		}
	}`)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.MCPServers) != 1 {
		t.Fatalf("expected 1 server, got %d", len(cfg.MCPServers))
	}
	s := cfg.MCPServers["test"]
	if s.Command != "echo" {
		t.Errorf("command = %q, want %q", s.Command, "echo")
	}
	if s.EffectiveType() != "stdio" {
		t.Errorf("type = %q, want stdio", s.EffectiveType())
	}
}

func TestLoad_ValidHTTP(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"mcpServers": {
			"remote": {"type": "http", "url": "http://localhost:8080/mcp"}
		}
	}`)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	s := cfg.MCPServers["remote"]
	if s.EffectiveType() != "http" {
		t.Errorf("type = %q, want http", s.EffectiveType())
	}
}

func TestLoad_WithGeminiKey(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"gemini_api_key": "test-key",
		"search": {"default_limit": 10, "model": "gemini-2.0-flash"},
		"mcpServers": {
			"test": {"command": "echo"}
		}
	}`)
	cfg, err := Load(p)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.GeminiAPIKey != "test-key" {
		t.Errorf("gemini key = %q, want %q", cfg.GeminiAPIKey, "test-key")
	}
	if cfg.Search.DefaultLimit != 10 {
		t.Errorf("limit = %d, want 10", cfg.Search.DefaultLimit)
	}
}

func TestLoad_MissingFile(t *testing.T) {
	_, err := Load("/nonexistent/config.json")
	if err == nil {
		t.Fatal("expected error for missing file")
	}
}

func TestLoad_InvalidJSON(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{invalid`)
	_, err := Load(p)
	if err == nil {
		t.Fatal("expected error for invalid JSON")
	}
}

func TestLoad_NoServers(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{"mcpServers": {}}`)
	_, err := Load(p)
	if err == nil {
		t.Fatal("expected error for empty servers")
	}
}

func TestLoad_StdioMissingCommand(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"mcpServers": {"bad": {"type": "stdio"}}
	}`)
	_, err := Load(p)
	if err == nil {
		t.Fatal("expected error for stdio without command")
	}
}

func TestLoad_HTTPMissingURL(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"mcpServers": {"bad": {"type": "http"}}
	}`)
	_, err := Load(p)
	if err == nil {
		t.Fatal("expected error for http without URL")
	}
}

func TestLoad_InvalidType(t *testing.T) {
	dir := t.TempDir()
	p := writeConfig(t, dir, `{
		"mcpServers": {"bad": {"type": "grpc", "command": "x"}}
	}`)
	_, err := Load(p)
	if err == nil {
		t.Fatal("expected error for invalid type")
	}
}

func TestDefaultPath(t *testing.T) {
	p := DefaultPath()
	if !filepath.IsAbs(p) {
		t.Error("DefaultPath should return absolute path")
	}
	if filepath.Base(p) != "config.json" {
		t.Errorf("DefaultPath base = %q, want config.json", filepath.Base(p))
	}
}

func TestCachePath(t *testing.T) {
	p := CachePath()
	if !filepath.IsAbs(p) {
		t.Error("CachePath should return absolute path")
	}
	if filepath.Base(p) != "tools.json" {
		t.Errorf("CachePath base = %q, want tools.json", filepath.Base(p))
	}
}

func TestValidate_ImplicitStdioWithCommand(t *testing.T) {
	cfg := &types.ProxyConfig{
		MCPServers: map[string]types.ServerConfig{
			"s": {Command: "test"},
		},
	}
	if err := validate(cfg); err != nil {
		t.Errorf("implicit stdio with command should be valid: %v", err)
	}
}

func TestLoadClaudeCodeConfig_NotFound(t *testing.T) {
	// Override HOME to empty dir so no config is found.
	dir := t.TempDir()
	t.Setenv("HOME", dir)
	_, err := LoadClaudeCodeConfig()
	if err == nil {
		t.Fatal("expected error when no Claude Code config exists")
	}
}

func TestLoadClaudeCodeConfig_Found(t *testing.T) {
	dir := t.TempDir()
	t.Setenv("HOME", dir)
	content := `{"mcpServers": {"slack": {"command": "slack-mcp"}}}`
	if err := os.WriteFile(filepath.Join(dir, ".claude.json"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
	cfg, err := LoadClaudeCodeConfig()
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := cfg.MCPServers["slack"]; !ok {
		t.Error("expected to find slack server")
	}
}
