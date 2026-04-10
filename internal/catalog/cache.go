// Package catalog maintains a cached index of all tools from upstream MCP servers.
// It persists the catalog to disk for instant startup and supports background refresh
// with merge-on-failure semantics to preserve cached tools when servers are unreachable.
//
// See https://hypercall.xyz for more information.

package catalog

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// readCache reads tool entries from a JSON cache file.
func readCache(path string) ([]types.ToolEntry, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var entries []types.ToolEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		return nil, fmt.Errorf("parsing cache: %w", err)
	}
	return entries, nil
}

// writeCache writes tool entries to a JSON cache file, creating
// parent directories as needed.
func writeCache(path string, entries []types.ToolEntry) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return fmt.Errorf("creating cache directory: %w", err)
	}
	data, err := json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return fmt.Errorf("marshaling cache: %w", err)
	}
	return os.WriteFile(path, data, 0600)
}
