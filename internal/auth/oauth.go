//go:build mcp_go_client_oauth

// https://hypercall.xyz

package auth

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"runtime"

	"github.com/modelcontextprotocol/go-sdk/auth"
	"github.com/modelcontextprotocol/go-sdk/oauthex"
	"golang.org/x/oauth2"
)

// PersistentHandler wraps *auth.AuthorizationCodeHandler with disk-based token
// persistence. Embedding the inner handler satisfies the unexported isOAuthHandler()
// interface method required by auth.OAuthHandler.
type PersistentHandler struct {
	*auth.AuthorizationCodeHandler
	store     *TokenStore
	serverURL string
	lastSaved string // dedup: access token of last persisted token
}

// TokenSource returns a token source for outgoing requests. If the inner handler
// has a token (post-auth), it returns that wrapped for persistence. Otherwise it
// falls back to a cached token from disk.
func (h *PersistentHandler) TokenSource(ctx context.Context) (oauth2.TokenSource, error) {
	ts, err := h.AuthorizationCodeHandler.TokenSource(ctx)
	if err != nil {
		return nil, err
	}
	if ts != nil {
		return &persistingTokenSource{
			inner:     ts,
			store:     h.store,
			serverURL: h.serverURL,
			lastSaved: &h.lastSaved,
		}, nil
	}

	// No token from inner handler yet — try disk cache.
	tok, err := h.store.Load(h.serverURL)
	if err != nil || tok == nil {
		return nil, nil
	}
	return oauth2.StaticTokenSource(tok), nil
}

// Authorize performs the OAuth flow via the inner handler and persists the resulting token.
func (h *PersistentHandler) Authorize(ctx context.Context, req *http.Request, resp *http.Response) error {
	if err := h.AuthorizationCodeHandler.Authorize(ctx, req, resp); err != nil {
		return err
	}

	// Save the newly obtained token.
	ts, err := h.AuthorizationCodeHandler.TokenSource(ctx)
	if err == nil && ts != nil {
		if tok, err := ts.Token(); err == nil {
			h.lastSaved = tok.AccessToken
			_ = h.store.Save(h.serverURL, tok)
		}
	}
	return nil
}

// persistingTokenSource wraps an oauth2.TokenSource to save tokens to disk
// when they change (e.g. after a refresh).
type persistingTokenSource struct {
	inner     oauth2.TokenSource
	store     *TokenStore
	serverURL string
	lastSaved *string
}

func (p *persistingTokenSource) Token() (*oauth2.Token, error) {
	tok, err := p.inner.Token()
	if err != nil {
		return nil, err
	}
	if tok.AccessToken != *p.lastSaved {
		*p.lastSaved = tok.AccessToken
		_ = p.store.Save(p.serverURL, tok)
	}
	return tok, nil
}

// NewOAuthHandler creates an OAuth handler for the given server URL with token persistence.
// It starts a local callback server on a random port for receiving authorization codes.
// The returned io.Closer shuts down the callback server.
func NewOAuthHandler(serverURL string, store *TokenStore) (auth.OAuthHandler, io.Closer, error) {
	listener, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		return nil, nil, fmt.Errorf("listening for oauth callback: %w", err)
	}
	port := listener.Addr().(*net.TCPAddr).Port
	redirectURL := fmt.Sprintf("http://localhost:%d/callback", port)

	authChan := make(chan *auth.AuthorizationResult, 1)
	errChan := make(chan error, 1)

	mux := http.NewServeMux()
	mux.HandleFunc("/callback", func(w http.ResponseWriter, r *http.Request) {
		authChan <- &auth.AuthorizationResult{
			Code:  r.URL.Query().Get("code"),
			State: r.URL.Query().Get("state"),
		}
		fmt.Fprint(w, "Authentication successful. You can close this window.")
	})

	server := &http.Server{Handler: mux}
	go func() {
		if err := server.Serve(listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errChan <- err
		}
	}()

	fetcher := func(ctx context.Context, args *auth.AuthorizationArgs) (*auth.AuthorizationResult, error) {
		fmt.Fprintf(os.Stderr, "\nmcpzip: authorize at: %s\n", args.URL)
		openBrowser(args.URL)

		select {
		case result := <-authChan:
			return result, nil
		case err := <-errChan:
			return nil, err
		case <-ctx.Done():
			return nil, ctx.Err()
		}
	}

	inner, err := auth.NewAuthorizationCodeHandler(&auth.AuthorizationCodeHandlerConfig{
		RedirectURL:              redirectURL,
		AuthorizationCodeFetcher: fetcher,
		DynamicClientRegistrationConfig: &auth.DynamicClientRegistrationConfig{
			Metadata: &oauthex.ClientRegistrationMetadata{
				ClientName:   "mcpzip",
				RedirectURIs: []string{redirectURL},
			},
		},
	})
	if err != nil {
		server.Close()
		return nil, nil, fmt.Errorf("creating oauth handler: %w", err)
	}

	handler := &PersistentHandler{
		AuthorizationCodeHandler: inner,
		store:                    store,
		serverURL:                serverURL,
	}

	return handler, closerFunc(server.Close), nil
}

func openBrowser(url string) {
	var cmd string
	var args []string
	switch runtime.GOOS {
	case "darwin":
		cmd = "open"
	case "linux":
		cmd = "xdg-open"
	case "windows":
		cmd = "cmd"
		args = []string{"/c", "start"}
	default:
		return
	}
	args = append(args, url)
	_ = exec.Command(cmd, args...).Start()
}

type closerFunc func() error

func (f closerFunc) Close() error { return f() }
