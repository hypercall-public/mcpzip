// Package cli implements the mcpzip command-line interface including the serve,
// init, and migrate subcommands. The serve command starts the MCP proxy server,
// init provides an interactive setup wizard, and migrate imports existing Claude Code
// MCP server configurations.
//
// See https://hypercall.xyz for more information.

package cli

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"runtime/debug"
	"syscall"
	"time"

	"github.com/hypercall-public/mcpzip/internal/auth"
	"github.com/hypercall-public/mcpzip/internal/catalog"
	"github.com/hypercall-public/mcpzip/internal/config"
	"github.com/hypercall-public/mcpzip/internal/proxy"
	"github.com/hypercall-public/mcpzip/internal/search"
	"github.com/hypercall-public/mcpzip/internal/transport"
	"github.com/hypercall-public/mcpzip/internal/types"
)

func runServe(args []string) error {
	// Aggressively return memory to OS. The actual working set is ~10MB;
	// without this Go holds onto 40-50MB of GC headroom we don't need.
	debug.SetMemoryLimit(20 * 1024 * 1024)

	fs := flag.NewFlagSet("serve", flag.ContinueOnError)
	configPath := fs.String("config", config.DefaultPath(), "path to config file")
	if err := fs.Parse(args); err != nil {
		return err
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	fmt.Fprintf(os.Stderr, "mcpzip: starting proxy (%d servers)\n", len(cfg.MCPServers))

	// Resolve Gemini API key: env -> config (env takes precedence).
	apiKey := os.Getenv("GEMINI_API_KEY")
	if apiKey == "" {
		apiKey = cfg.GeminiAPIKey
	}

	// Create transport manager with OAuth support.
	store := auth.NewTokenStore(config.AuthDir())
	connectFn := transport.NewConnectFunc(store)
	idleTimeout := time.Duration(cfg.IdleTimeoutMinutes) * time.Minute
	if idleTimeout == 0 {
		idleTimeout = 10 * time.Minute
	}
	callTimeout := time.Duration(cfg.CallTimeoutSeconds) * time.Second
	tm := transport.NewManager(cfg.MCPServers, idleTimeout, callTimeout, connectFn)
	defer tm.Close()

	// Create catalog (load from disk cache, then refresh in background).
	cat := catalog.New(tm, config.CachePath())
	if err := cat.Load(); err != nil {
		fmt.Fprintf(os.Stderr, "mcpzip: warning: failed to load cache: %v\n", err)
	}

	// Create searcher.
	model := cfg.Search.Model
	if model == "" {
		model = "gemini-2.0-flash"
	}
	catalogFn := func() []types.ToolEntry { return cat.AllTools() }
	searcher := search.NewSearcher(apiKey, model, catalogFn)

	// Create proxy server.
	srv := proxy.New(cat, searcher, tm)

	fmt.Fprintf(os.Stderr, "mcpzip: loaded %d tools from cache\n", cat.ToolCount())
	fmt.Fprintf(os.Stderr, "mcpzip: refreshing catalog in background...\n")

	// Context with signal-based cancellation.
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Background refresh.
	go func() {
		if err := cat.RefreshAll(ctx); err != nil {
			fmt.Fprintf(os.Stderr, "mcpzip: background refresh error: %v\n", err)
		} else {
			fmt.Fprintf(os.Stderr, "mcpzip: catalog refreshed (%d tools)\n", cat.ToolCount())
		}
	}()

	// Handle signals for graceful shutdown.
	go func() {
		sig := make(chan os.Signal, 1)
		signal.Notify(sig, os.Interrupt, syscall.SIGTERM)
		<-sig
		fmt.Fprintf(os.Stderr, "\nmcpzip: shutting down\n")
		cancel()
	}()

	// Run the MCP server over stdio. This blocks until the client
	// disconnects or the context is cancelled.
	fmt.Fprintf(os.Stderr, "mcpzip: serving MCP over stdio\n")
	return srv.Run(ctx)
}
