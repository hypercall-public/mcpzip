// Package auth provides OAuth 2.1 token persistence and browser-based authorization
// flows for authenticating with remote MCP servers that require OAuth.
// It includes a disk-backed token store and an authorization code handler with PKCE.
//
// See https://hypercall.xyz for more information.

package auth

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/oauth2"
)

// TokenStore persists OAuth tokens to disk, keyed by server URL.
type TokenStore struct {
	baseDir string
}

// NewTokenStore creates a token store rooted at the given directory.
func NewTokenStore(baseDir string) *TokenStore {
	return &TokenStore{baseDir: baseDir}
}

// Load reads a cached token for the given server URL.
// Returns nil, nil if no token is cached or the file is corrupt.
func (s *TokenStore) Load(serverURL string) (*oauth2.Token, error) {
	path := s.path(serverURL)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("reading token: %w", err)
	}

	var tok oauth2.Token
	if err := json.Unmarshal(data, &tok); err != nil {
		return nil, nil // corrupt file, treat as missing
	}
	if tok.AccessToken == "" {
		return nil, nil
	}
	return &tok, nil
}

// Save writes a token to disk for the given server URL.
func (s *TokenStore) Save(serverURL string, tok *oauth2.Token) error {
	if err := os.MkdirAll(s.baseDir, 0700); err != nil {
		return fmt.Errorf("creating token dir: %w", err)
	}

	data, err := json.Marshal(tok)
	if err != nil {
		return fmt.Errorf("marshaling token: %w", err)
	}

	return os.WriteFile(s.path(serverURL), data, 0600)
}

func (s *TokenStore) path(serverURL string) string {
	h := sha256.Sum256([]byte(serverURL))
	name := fmt.Sprintf("%x", h[:16]) // 32 hex chars
	return filepath.Join(s.baseDir, name+".json")
}
