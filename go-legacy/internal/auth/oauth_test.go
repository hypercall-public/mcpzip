//go:build mcp_go_client_oauth

// https://hypercall.xyz

package auth

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"testing"
	"time"

	"golang.org/x/oauth2"
)

func TestNewOAuthHandlerPortAllocation(t *testing.T) {
	store := NewTokenStore(t.TempDir())
	handler, closer, err := NewOAuthHandler("https://example.com/mcp", store)
	if err != nil {
		t.Fatalf("NewOAuthHandler: %v", err)
	}
	defer closer.Close()

	if handler == nil {
		t.Fatal("handler is nil")
	}
}

func TestNewOAuthHandlerMultipleServers(t *testing.T) {
	store := NewTokenStore(t.TempDir())

	handler1, closer1, err := NewOAuthHandler("https://server1.com/mcp", store)
	if err != nil {
		t.Fatalf("NewOAuthHandler (1): %v", err)
	}
	defer closer1.Close()

	handler2, closer2, err := NewOAuthHandler("https://server2.com/mcp", store)
	if err != nil {
		t.Fatalf("NewOAuthHandler (2): %v", err)
	}
	defer closer2.Close()

	if handler1 == nil || handler2 == nil {
		t.Fatal("one or both handlers are nil")
	}
}

func TestTokenSourceNilBeforeAuth(t *testing.T) {
	store := NewTokenStore(t.TempDir())
	handler, closer, err := NewOAuthHandler("https://example.com/mcp", store)
	if err != nil {
		t.Fatalf("NewOAuthHandler: %v", err)
	}
	defer closer.Close()

	ph := handler.(*PersistentHandler)
	ts, err := ph.TokenSource(context.Background())
	if err != nil {
		t.Fatalf("TokenSource: %v", err)
	}
	if ts != nil {
		t.Error("expected nil TokenSource before auth")
	}
}

func TestTokenSourceWithCachedToken(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)
	serverURL := "https://example.com/mcp"

	if err := store.Save(serverURL, &oauth2.Token{
		AccessToken: "cached-access",
		TokenType:   "Bearer",
	}); err != nil {
		t.Fatalf("Save: %v", err)
	}

	handler, closer, err := NewOAuthHandler(serverURL, store)
	if err != nil {
		t.Fatalf("NewOAuthHandler: %v", err)
	}
	defer closer.Close()

	ph := handler.(*PersistentHandler)
	ts, err := ph.TokenSource(context.Background())
	if err != nil {
		t.Fatalf("TokenSource: %v", err)
	}
	if ts == nil {
		t.Fatal("expected non-nil TokenSource with cached token")
	}
	got, err := ts.Token()
	if err != nil {
		t.Fatalf("Token: %v", err)
	}
	if got.AccessToken != "cached-access" {
		t.Errorf("AccessToken = %q, want %q", got.AccessToken, "cached-access")
	}
}

func TestCallbackServerIntegration(t *testing.T) {
	type result struct {
		code  string
		state string
	}
	authChan := make(chan result, 1)

	mux := http.NewServeMux()
	mux.HandleFunc("/callback", func(w http.ResponseWriter, r *http.Request) {
		authChan <- result{
			code:  r.URL.Query().Get("code"),
			state: r.URL.Query().Get("state"),
		}
		fmt.Fprint(w, "Authentication successful. You can close this window.")
	})

	ln, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	port := ln.Addr().(*net.TCPAddr).Port

	server := &http.Server{Handler: mux}
	go server.Serve(ln)
	defer server.Close()

	url := fmt.Sprintf("http://localhost:%d/callback?code=test-code&state=test-state", port)
	resp, err := http.Get(url)
	if err != nil {
		t.Fatalf("GET callback: %v", err)
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)

	if resp.StatusCode != 200 {
		t.Errorf("status = %d, want 200", resp.StatusCode)
	}
	if string(body) != "Authentication successful. You can close this window." {
		t.Errorf("body = %q", string(body))
	}

	select {
	case r := <-authChan:
		if r.code != "test-code" {
			t.Errorf("code = %q, want %q", r.code, "test-code")
		}
		if r.state != "test-state" {
			t.Errorf("state = %q, want %q", r.state, "test-state")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for auth result")
	}
}

func TestCloserShutsDownServer(t *testing.T) {
	store := NewTokenStore(t.TempDir())
	_, closer, err := NewOAuthHandler("https://example.com/mcp", store)
	if err != nil {
		t.Fatalf("NewOAuthHandler: %v", err)
	}

	if err := closer.Close(); err != nil {
		t.Errorf("first Close: %v", err)
	}

	// Second close should not panic.
	_ = closer.Close()
}
