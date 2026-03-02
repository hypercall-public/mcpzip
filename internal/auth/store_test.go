// https://hypercall.xyz

package auth

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"golang.org/x/oauth2"
)

func TestStoreRoundtrip(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)

	tok := &oauth2.Token{
		AccessToken:  "access-123",
		TokenType:    "Bearer",
		RefreshToken: "refresh-456",
		Expiry:       time.Date(2026, 6, 1, 0, 0, 0, 0, time.UTC),
	}

	if err := store.Save("https://example.com/mcp", tok); err != nil {
		t.Fatalf("Save: %v", err)
	}

	got, err := store.Load("https://example.com/mcp")
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if got == nil {
		t.Fatal("Load returned nil")
	}
	if got.AccessToken != tok.AccessToken {
		t.Errorf("AccessToken = %q, want %q", got.AccessToken, tok.AccessToken)
	}
	if got.RefreshToken != tok.RefreshToken {
		t.Errorf("RefreshToken = %q, want %q", got.RefreshToken, tok.RefreshToken)
	}
	if !got.Expiry.Equal(tok.Expiry) {
		t.Errorf("Expiry = %v, want %v", got.Expiry, tok.Expiry)
	}
}

func TestStoreLoadMissing(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)

	tok, err := store.Load("https://no-such-server.com")
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if tok != nil {
		t.Fatalf("expected nil token, got %+v", tok)
	}
}

func TestStoreLoadCorrupt(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)

	// Write garbage to the token file.
	path := store.path("https://example.com")
	if err := os.WriteFile(path, []byte("not json{{{"), 0600); err != nil {
		t.Fatalf("writing corrupt file: %v", err)
	}

	tok, err := store.Load("https://example.com")
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if tok != nil {
		t.Fatalf("expected nil for corrupt file, got %+v", tok)
	}
}

func TestStoreLoadEmptyToken(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)

	// Write a valid JSON token with empty access token.
	path := store.path("https://example.com")
	if err := os.WriteFile(path, []byte(`{"access_token":""}`), 0600); err != nil {
		t.Fatalf("writing empty token: %v", err)
	}

	tok, err := store.Load("https://example.com")
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if tok != nil {
		t.Fatalf("expected nil for empty access token, got %+v", tok)
	}
}

func TestStoreFilePermissions(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(filepath.Join(dir, "auth"))

	tok := &oauth2.Token{AccessToken: "secret", TokenType: "Bearer"}
	if err := store.Save("https://example.com", tok); err != nil {
		t.Fatalf("Save: %v", err)
	}

	// Check directory permissions.
	info, err := os.Stat(filepath.Join(dir, "auth"))
	if err != nil {
		t.Fatalf("stat dir: %v", err)
	}
	if perm := info.Mode().Perm(); perm != 0700 {
		t.Errorf("dir perm = %o, want 0700", perm)
	}

	// Check file permissions.
	path := store.path("https://example.com")
	info, err = os.Stat(path)
	if err != nil {
		t.Fatalf("stat file: %v", err)
	}
	if perm := info.Mode().Perm(); perm != 0600 {
		t.Errorf("file perm = %o, want 0600", perm)
	}
}

func TestStoreDifferentURLs(t *testing.T) {
	dir := t.TempDir()
	store := NewTokenStore(dir)

	tok1 := &oauth2.Token{AccessToken: "token-1", TokenType: "Bearer"}
	tok2 := &oauth2.Token{AccessToken: "token-2", TokenType: "Bearer"}

	if err := store.Save("https://server-a.com", tok1); err != nil {
		t.Fatalf("Save: %v", err)
	}
	if err := store.Save("https://server-b.com", tok2); err != nil {
		t.Fatalf("Save: %v", err)
	}

	got1, _ := store.Load("https://server-a.com")
	got2, _ := store.Load("https://server-b.com")

	if got1.AccessToken != "token-1" {
		t.Errorf("server-a token = %q, want %q", got1.AccessToken, "token-1")
	}
	if got2.AccessToken != "token-2" {
		t.Errorf("server-b token = %q, want %q", got2.AccessToken, "token-2")
	}
}
