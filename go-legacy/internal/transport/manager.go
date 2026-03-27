// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/jake/mcpzip/internal/types"
)

// Manager manages connections to upstream MCP servers.
// It maintains a pool of active connections and reaps idle ones.
type Manager struct {
	configs     map[string]types.ServerConfig
	pool        map[string]*poolEntry
	mu          sync.RWMutex
	idleTimeout time.Duration
	callTimeout time.Duration
	stopReaper  chan struct{}
	closeOnce   sync.Once
	connect     ConnectFunc
}

type poolEntry struct {
	upstream Upstream
	lastUsed time.Time
}

// NewManager creates a new transport manager and starts the idle reaper.
// callTimeout is optional — if zero, tool calls use the caller's context deadline only.
func NewManager(configs map[string]types.ServerConfig, idleTimeout, callTimeout time.Duration, connect ConnectFunc) *Manager {
	if connect == nil {
		connect = DefaultConnect
	}
	m := &Manager{
		configs:     configs,
		pool:        make(map[string]*poolEntry),
		idleTimeout: idleTimeout,
		callTimeout: callTimeout,
		stopReaper:  make(chan struct{}),
		connect:     connect,
	}
	go m.reaper()
	return m
}

// GetUpstream returns a pooled connection for the named server, creating one if
// necessary or if the existing connection is no longer alive.
func (m *Manager) GetUpstream(ctx context.Context, serverName string) (Upstream, error) {
	cfg, ok := m.configs[serverName]
	if !ok {
		return nil, fmt.Errorf("unknown server: %s", serverName)
	}

	// Try to return a live pooled connection.
	m.mu.Lock()
	defer m.mu.Unlock()

	if entry, exists := m.pool[serverName]; exists && entry.upstream.Alive() {
		entry.lastUsed = time.Now()
		return entry.upstream, nil
	}

	// Close stale connection if present.
	if entry, exists := m.pool[serverName]; exists {
		_ = entry.upstream.Close()
		delete(m.pool, serverName)
	}

	upstream, err := m.connect(ctx, serverName, cfg)
	if err != nil {
		return nil, fmt.Errorf("connecting to %s: %w", serverName, err)
	}

	m.pool[serverName] = &poolEntry{
		upstream: upstream,
		lastUsed: time.Now(),
	}
	return upstream, nil
}

// CallTool is a convenience method that gets the upstream for a server and
// invokes the named tool. If the call fails, it evicts the stale connection
// and retries once with a fresh connection.
func (m *Manager) CallTool(ctx context.Context, serverName, toolName string, args json.RawMessage) (json.RawMessage, error) {
	upstream, err := m.GetUpstream(ctx, serverName)
	if err != nil {
		return nil, err
	}

	callCtx := ctx
	if m.callTimeout > 0 {
		var cancel context.CancelFunc
		callCtx, cancel = context.WithTimeout(ctx, m.callTimeout)
		defer cancel()
	}

	result, err := upstream.CallTool(callCtx, toolName, args)
	if err == nil {
		return result, nil
	}

	// Call failed — connection may be stale (e.g. upstream server restarted).
	// Evict from pool and retry once with a fresh connection.
	m.evict(serverName)

	upstream, err2 := m.GetUpstream(ctx, serverName)
	if err2 != nil {
		return nil, err // return original error
	}

	// Fresh context for retry — the original callCtx may have been cancelled.
	retryCtx := ctx
	if m.callTimeout > 0 {
		var cancel context.CancelFunc
		retryCtx, cancel = context.WithTimeout(ctx, m.callTimeout)
		defer cancel()
	}

	result, err2 = upstream.CallTool(retryCtx, toolName, args)
	if err2 != nil {
		return nil, err2
	}
	return result, nil
}

// evict closes and removes a connection from the pool.
func (m *Manager) evict(serverName string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if entry, exists := m.pool[serverName]; exists {
		_ = entry.upstream.Close()
		delete(m.pool, serverName)
	}
}

// ListToolsAll lists tools from all configured servers concurrently.
// A failure from one server does not prevent results from other servers.
// Each server gets a 30-second timeout.
func (m *Manager) ListToolsAll(ctx context.Context) (map[string][]types.ToolEntry, error) {
	if len(m.configs) == 0 {
		return make(map[string][]types.ToolEntry), nil
	}

	type result struct {
		name  string
		tools []types.ToolEntry
		err   error
	}

	results := make(chan result, len(m.configs))
	var wg sync.WaitGroup

	for name := range m.configs {
		wg.Add(1)
		go func(serverName string) {
			defer wg.Done()
			serverCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
			defer cancel()

			upstream, err := m.GetUpstream(serverCtx, serverName)
			if err != nil {
				results <- result{name: serverName, err: err}
				return
			}
			tools, err := upstream.ListTools(serverCtx)
			results <- result{name: serverName, tools: tools, err: err}
		}(name)
	}

	// Close the results channel once all goroutines finish.
	go func() {
		wg.Wait()
		close(results)
	}()

	allTools := make(map[string][]types.ToolEntry, len(m.configs))
	for r := range results {
		if r.err != nil {
			// Log or skip -- one failure doesn't fail others.
			continue
		}
		allTools[r.name] = r.tools
	}

	return allTools, nil
}

// Close shuts down all pooled connections and stops the reaper.
// Safe to call multiple times.
func (m *Manager) Close() error {
	m.closeOnce.Do(func() { close(m.stopReaper) })

	m.mu.Lock()
	defer m.mu.Unlock()

	var firstErr error
	for name, entry := range m.pool {
		if err := entry.upstream.Close(); err != nil && firstErr == nil {
			firstErr = err
		}
		delete(m.pool, name)
	}
	return firstErr
}

// reaper periodically closes connections that have been idle longer than
// idleTimeout. It runs every 60 seconds (or every idleTimeout/2 if that's
// shorter, to make tests fast).
func (m *Manager) reaper() {
	interval := 60 * time.Second
	if half := m.idleTimeout / 2; half < interval && half > 0 {
		interval = half
	}

	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for {
		select {
		case <-m.stopReaper:
			return
		case <-ticker.C:
			m.reapIdle()
		}
	}
}

// reapIdle closes any pooled connections that have been idle past the timeout.
func (m *Manager) reapIdle() {
	m.mu.Lock()
	defer m.mu.Unlock()

	now := time.Now()
	for name, entry := range m.pool {
		if now.Sub(entry.lastUsed) > m.idleTimeout {
			_ = entry.upstream.Close()
			delete(m.pool, name)
		}
	}
}
