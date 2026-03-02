// https://hypercall.xyz

package transport

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"testing"
	"time"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// mockUpstream implements Upstream for testing.
type mockUpstream struct {
	mu     sync.Mutex
	tools  []types.ToolEntry
	alive  bool
	closed bool
	// callResult is returned by CallTool.
	callResult json.RawMessage
	// callErr is returned by CallTool if non-nil.
	callErr error
}

func (m *mockUpstream) ListTools(_ context.Context) ([]types.ToolEntry, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.closed {
		return nil, fmt.Errorf("upstream closed")
	}
	return m.tools, nil
}

func (m *mockUpstream) CallTool(_ context.Context, toolName string, args json.RawMessage) (json.RawMessage, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.closed {
		return nil, fmt.Errorf("upstream closed")
	}
	if m.callErr != nil {
		return nil, m.callErr
	}
	if m.callResult != nil {
		return m.callResult, nil
	}
	return json.RawMessage(fmt.Sprintf(`{"tool":%q}`, toolName)), nil
}

func (m *mockUpstream) Close() error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.closed = true
	return nil
}

func (m *mockUpstream) Alive() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.alive && !m.closed
}

func (m *mockUpstream) isClosed() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.closed
}

// mockConnectFunc returns a factory that creates mockUpstreams from the provided map.
func mockConnectFunc(upstreams map[string]*mockUpstream) ConnectFunc {
	return func(_ context.Context, name string, _ types.ServerConfig) (Upstream, error) {
		u, ok := upstreams[name]
		if !ok {
			return nil, fmt.Errorf("no mock for %s", name)
		}
		return u, nil
	}
}

func TestGetUpstream_CreatesNewConnection(t *testing.T) {
	mock := &mockUpstream{alive: true}
	mocks := map[string]*mockUpstream{"server1": mock}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, mockConnectFunc(mocks))
	defer mgr.Close()

	upstream, err := mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("GetUpstream failed: %v", err)
	}
	if upstream != mock {
		t.Fatal("expected mock upstream to be returned")
	}
}

func TestGetUpstream_ReusesPooledConnection(t *testing.T) {
	callCount := 0
	mock := &mockUpstream{alive: true}

	factory := func(_ context.Context, name string, _ types.ServerConfig) (Upstream, error) {
		callCount++
		return mock, nil
	}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, factory)
	defer mgr.Close()

	_, err := mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("first GetUpstream failed: %v", err)
	}

	_, err = mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("second GetUpstream failed: %v", err)
	}

	if callCount != 1 {
		t.Fatalf("expected factory to be called once, got %d", callCount)
	}
}

func TestGetUpstream_CreatesNewIfNotAlive(t *testing.T) {
	callCount := 0
	mock1 := &mockUpstream{alive: true}
	mock2 := &mockUpstream{alive: true}

	factory := func(_ context.Context, name string, _ types.ServerConfig) (Upstream, error) {
		callCount++
		if callCount == 1 {
			return mock1, nil
		}
		return mock2, nil
	}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, factory)
	defer mgr.Close()

	u1, err := mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("first GetUpstream failed: %v", err)
	}
	if u1 != mock1 {
		t.Fatal("expected mock1")
	}

	// Mark the first mock as dead.
	mock1.mu.Lock()
	mock1.alive = false
	mock1.mu.Unlock()

	u2, err := mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("second GetUpstream failed: %v", err)
	}
	if u2 != mock2 {
		t.Fatal("expected mock2 after mock1 died")
	}
	if callCount != 2 {
		t.Fatalf("expected factory called twice, got %d", callCount)
	}

	// Verify the old connection was closed.
	if !mock1.isClosed() {
		t.Fatal("expected stale mock1 to be closed")
	}
}

func TestGetUpstream_UnknownServer(t *testing.T) {
	configs := map[string]types.ServerConfig{}
	mgr := NewManager(configs, 5*time.Minute, 0, DefaultConnect)
	defer mgr.Close()

	_, err := mgr.GetUpstream(context.Background(), "nonexistent")
	if err == nil {
		t.Fatal("expected error for unknown server")
	}
}

func TestCallTool_RoutesToCorrectUpstream(t *testing.T) {
	mockA := &mockUpstream{
		alive:      true,
		callResult: json.RawMessage(`{"from":"a"}`),
	}
	mockB := &mockUpstream{
		alive:      true,
		callResult: json.RawMessage(`{"from":"b"}`),
	}
	mocks := map[string]*mockUpstream{"serverA": mockA, "serverB": mockB}

	configs := map[string]types.ServerConfig{
		"serverA": {Command: "a"},
		"serverB": {Command: "b"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, mockConnectFunc(mocks))
	defer mgr.Close()

	result, err := mgr.CallTool(context.Background(), "serverA", "tool1", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("CallTool serverA failed: %v", err)
	}
	if string(result) != `{"from":"a"}` {
		t.Fatalf("unexpected result from serverA: %s", result)
	}

	result, err = mgr.CallTool(context.Background(), "serverB", "tool2", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("CallTool serverB failed: %v", err)
	}
	if string(result) != `{"from":"b"}` {
		t.Fatalf("unexpected result from serverB: %s", result)
	}
}

func TestListToolsAll_AggregatesFromMultipleServers(t *testing.T) {
	mockA := &mockUpstream{
		alive: true,
		tools: []types.ToolEntry{
			{Name: "serverA__tool1", ServerName: "serverA", OriginalName: "tool1"},
		},
	}
	mockB := &mockUpstream{
		alive: true,
		tools: []types.ToolEntry{
			{Name: "serverB__tool2", ServerName: "serverB", OriginalName: "tool2"},
			{Name: "serverB__tool3", ServerName: "serverB", OriginalName: "tool3"},
		},
	}
	mocks := map[string]*mockUpstream{"serverA": mockA, "serverB": mockB}

	configs := map[string]types.ServerConfig{
		"serverA": {Command: "a"},
		"serverB": {Command: "b"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, mockConnectFunc(mocks))
	defer mgr.Close()

	allTools, err := mgr.ListToolsAll(context.Background())
	if err != nil {
		t.Fatalf("ListToolsAll failed: %v", err)
	}

	if len(allTools) != 2 {
		t.Fatalf("expected 2 servers, got %d", len(allTools))
	}
	if len(allTools["serverA"]) != 1 {
		t.Fatalf("expected 1 tool from serverA, got %d", len(allTools["serverA"]))
	}
	if len(allTools["serverB"]) != 2 {
		t.Fatalf("expected 2 tools from serverB, got %d", len(allTools["serverB"]))
	}
}

func TestListToolsAll_HandlesOneServerFailure(t *testing.T) {
	mockA := &mockUpstream{
		alive: true,
		tools: []types.ToolEntry{
			{Name: "serverA__tool1", ServerName: "serverA", OriginalName: "tool1"},
		},
	}
	// serverB will fail to connect because there is no mock for it.
	mocks := map[string]*mockUpstream{"serverA": mockA}

	configs := map[string]types.ServerConfig{
		"serverA": {Command: "a"},
		"serverB": {Command: "b"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, mockConnectFunc(mocks))
	defer mgr.Close()

	allTools, err := mgr.ListToolsAll(context.Background())
	if err != nil {
		t.Fatalf("ListToolsAll failed: %v", err)
	}

	// serverA should succeed, serverB should be absent.
	if len(allTools["serverA"]) != 1 {
		t.Fatalf("expected 1 tool from serverA, got %d", len(allTools["serverA"]))
	}
	if _, exists := allTools["serverB"]; exists {
		t.Fatal("expected serverB to be absent from results due to failure")
	}
}

func TestReaper_ClosesIdleConnections(t *testing.T) {
	mock := &mockUpstream{alive: true}
	mocks := map[string]*mockUpstream{"server1": mock}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	// Use a very short idle timeout so the reaper triggers quickly.
	mgr := NewManager(configs, 50*time.Millisecond, 0, mockConnectFunc(mocks))
	defer mgr.Close()

	_, err := mgr.GetUpstream(context.Background(), "server1")
	if err != nil {
		t.Fatalf("GetUpstream failed: %v", err)
	}

	// Wait long enough for the reaper to run (reaper interval = idleTimeout/2 = 25ms).
	time.Sleep(200 * time.Millisecond)

	if !mock.isClosed() {
		t.Fatal("expected idle connection to be reaped")
	}

	// Verify it's gone from the pool.
	mgr.mu.RLock()
	_, inPool := mgr.pool["server1"]
	mgr.mu.RUnlock()

	if inPool {
		t.Fatal("expected reaped connection to be removed from pool")
	}
}

func TestClose_TearsDownAllConnections(t *testing.T) {
	mockA := &mockUpstream{alive: true}
	mockB := &mockUpstream{alive: true}
	mocks := map[string]*mockUpstream{"serverA": mockA, "serverB": mockB}

	configs := map[string]types.ServerConfig{
		"serverA": {Command: "a"},
		"serverB": {Command: "b"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, mockConnectFunc(mocks))

	// Create connections.
	_, _ = mgr.GetUpstream(context.Background(), "serverA")
	_, _ = mgr.GetUpstream(context.Background(), "serverB")

	err := mgr.Close()
	if err != nil {
		t.Fatalf("Close returned error: %v", err)
	}

	if !mockA.isClosed() {
		t.Fatal("expected serverA to be closed")
	}
	if !mockB.isClosed() {
		t.Fatal("expected serverB to be closed")
	}

	mgr.mu.RLock()
	poolSize := len(mgr.pool)
	mgr.mu.RUnlock()

	if poolSize != 0 {
		t.Fatalf("expected empty pool after Close, got %d entries", poolSize)
	}
}

func TestCallTool_RetriesOnStaleConnection(t *testing.T) {
	callCount := 0
	stale := &mockUpstream{
		alive:   true,
		callErr: fmt.Errorf("connection closed: session not found"),
	}
	fresh := &mockUpstream{
		alive:      true,
		callResult: json.RawMessage(`{"ok":true}`),
	}

	factory := func(_ context.Context, name string, _ types.ServerConfig) (Upstream, error) {
		callCount++
		if callCount == 1 {
			return stale, nil
		}
		return fresh, nil
	}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, factory)
	defer mgr.Close()

	result, err := mgr.CallTool(context.Background(), "server1", "mytool", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("CallTool should have succeeded on retry, got: %v", err)
	}
	if string(result) != `{"ok":true}` {
		t.Fatalf("unexpected result: %s", result)
	}
	if callCount != 2 {
		t.Fatalf("expected factory called twice (initial + reconnect), got %d", callCount)
	}
	if !stale.isClosed() {
		t.Fatal("expected stale connection to be closed")
	}
}

func TestCallTool_ReturnsRetryErrorIfBothFail(t *testing.T) {
	callCount := 0
	factory := func(_ context.Context, name string, _ types.ServerConfig) (Upstream, error) {
		callCount++
		return &mockUpstream{
			alive:   true,
			callErr: fmt.Errorf("fail-%d", callCount),
		}, nil
	}

	configs := map[string]types.ServerConfig{
		"server1": {Command: "echo"},
	}

	mgr := NewManager(configs, 5*time.Minute, 0, factory)
	defer mgr.Close()

	_, err := mgr.CallTool(context.Background(), "server1", "mytool", json.RawMessage(`{}`))
	if err == nil {
		t.Fatal("expected error when both attempts fail")
	}
	// Should return the retry error (fail-2), not the original.
	if err.Error() != "fail-2" {
		t.Fatalf("expected retry error, got: %v", err)
	}
}

func TestListToolsAll_NoServers(t *testing.T) {
	configs := map[string]types.ServerConfig{}
	mgr := NewManager(configs, 5*time.Minute, 0, DefaultConnect)
	defer mgr.Close()

	allTools, err := mgr.ListToolsAll(context.Background())
	if err != nil {
		t.Fatalf("ListToolsAll failed: %v", err)
	}
	if len(allTools) != 0 {
		t.Fatalf("expected empty map, got %d entries", len(allTools))
	}
}
