// Package catalog maintains a cached index of all tools from upstream MCP servers.
// It persists the catalog to disk for instant startup and supports background refresh
// with merge-on-failure semantics to preserve cached tools when servers are unreachable.
//
// See https://hypercall.xyz for more information.

package catalog

import (
	"context"
	"fmt"
	"sort"
	"sync"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// ToolLister fetches tools from upstream servers.
// Implemented by transport.Manager.
type ToolLister interface {
	ListToolsAll(ctx context.Context) (map[string][]types.ToolEntry, error)
}

// Catalog manages the aggregated tool catalog from all upstream servers.
type Catalog struct {
	mu       sync.RWMutex
	tools    []types.ToolEntry
	byName   map[string]*types.ToolEntry
	byServer map[string][]types.ToolEntry

	lister    ToolLister
	cachePath string
}

// New creates a new Catalog.
func New(lister ToolLister, cachePath string) *Catalog {
	return &Catalog{
		lister:    lister,
		cachePath: cachePath,
		byName:    make(map[string]*types.ToolEntry),
		byServer:  make(map[string][]types.ToolEntry),
	}
}

// Load reads the catalog from the disk cache. If the cache doesn't exist
// or is invalid, the catalog starts empty (no error).
func (c *Catalog) Load() error {
	entries, err := readCache(c.cachePath)
	if err != nil {
		// Missing or corrupt cache is not fatal; start empty.
		return nil
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	c.setTools(entries)
	return nil
}

// RefreshAll fetches tools from all upstream servers via the ToolLister,
// updates the in-memory catalog, and saves to disk cache.
func (c *Catalog) RefreshAll(ctx context.Context) error {
	if c.lister == nil {
		return fmt.Errorf("no tool lister configured")
	}

	serverTools, err := c.lister.ListToolsAll(ctx)
	if err != nil {
		return fmt.Errorf("fetching tools: %w", err)
	}

	var all []types.ToolEntry
	for serverName, rawTools := range serverTools {
		for _, t := range rawTools {
			entry := types.ToolEntry{
				Name:          types.PrefixedName(serverName, t.OriginalName),
				ServerName:    serverName,
				OriginalName:  t.OriginalName,
				Description:   t.Description,
				InputSchema:   t.InputSchema,
				CompactParams: types.CompactParamsFromSchema(t.InputSchema),
			}
			// If upstream already set OriginalName, use it; otherwise use Name as original.
			if entry.OriginalName == "" {
				entry.OriginalName = t.Name
				entry.Name = types.PrefixedName(serverName, t.Name)
			}
			all = append(all, entry)
		}
	}

	// Sort for deterministic ordering.
	sort.Slice(all, func(i, j int) bool {
		return all[i].Name < all[j].Name
	})

	c.mu.Lock()
	c.setTools(all)
	c.mu.Unlock()

	if c.cachePath != "" {
		if err := writeCache(c.cachePath, all); err != nil {
			return fmt.Errorf("saving cache: %w", err)
		}
	}

	return nil
}

// AllTools returns a copy of all tools in the catalog.
func (c *Catalog) AllTools() []types.ToolEntry {
	c.mu.RLock()
	defer c.mu.RUnlock()
	result := make([]types.ToolEntry, len(c.tools))
	copy(result, c.tools)
	return result
}

// GetTool returns a tool by its prefixed name.
func (c *Catalog) GetTool(prefixedName string) (*types.ToolEntry, error) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	t, ok := c.byName[prefixedName]
	if !ok {
		return nil, fmt.Errorf("unknown tool: %s", prefixedName)
	}
	copied := *t
	return &copied, nil
}

// ServerTools returns all tools for a given server.
func (c *Catalog) ServerTools(serverName string) []types.ToolEntry {
	c.mu.RLock()
	defer c.mu.RUnlock()
	result := make([]types.ToolEntry, len(c.byServer[serverName]))
	copy(result, c.byServer[serverName])
	return result
}

// ToolCount returns the total number of tools.
func (c *Catalog) ToolCount() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.tools)
}

// ServerNames returns sorted list of server names that have tools.
func (c *Catalog) ServerNames() []string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	names := make([]string, 0, len(c.byServer))
	for name := range c.byServer {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// setTools replaces the catalog contents. Caller must hold write lock.
func (c *Catalog) setTools(tools []types.ToolEntry) {
	c.tools = tools
	c.byName = make(map[string]*types.ToolEntry, len(tools))
	c.byServer = make(map[string][]types.ToolEntry)
	for i := range tools {
		c.byName[tools[i].Name] = &tools[i]
		c.byServer[tools[i].ServerName] = append(c.byServer[tools[i].ServerName], tools[i])
	}
}
