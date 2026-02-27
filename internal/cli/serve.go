// https://hypercall.xyz

package cli

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/hypercall-public/mcpzip/internal/catalog"
	"github.com/hypercall-public/mcpzip/internal/config"
	"github.com/hypercall-public/mcpzip/internal/proxy"
	"github.com/hypercall-public/mcpzip/internal/search"
	"github.com/hypercall-public/mcpzip/internal/transport"
	"github.com/hypercall-public/mcpzip/internal/types"
)

func runServe(args []string) error {
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

	// Create transport manager.
	idleTimeout := time.Duration(cfg.IdleTimeoutMinutes) * time.Minute
	if idleTimeout == 0 {
		idleTimeout = 10 * time.Minute
	}
	tm := transport.NewManager(cfg.MCPServers, idleTimeout, nil)
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
	// TODO: Wire into go-sdk MCP server with stdio transport.
	// Currently the proxy logic works but isn't connected to MCP protocol layer.
	_ = proxy.New(cat, searcher, tm)

	fmt.Fprintf(os.Stderr, "mcpzip: loaded %d tools from cache\n", cat.ToolCount())
	fmt.Fprintf(os.Stderr, "mcpzip: refreshing catalog in background...\n")

	// Background refresh.
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go func() {
		if err := cat.RefreshAll(ctx); err != nil {
			fmt.Fprintf(os.Stderr, "mcpzip: background refresh error: %v\n", err)
		} else {
			fmt.Fprintf(os.Stderr, "mcpzip: catalog refreshed (%d tools)\n", cat.ToolCount())
		}
	}()

	// Wait for interrupt.
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt, syscall.SIGTERM)
	<-sig
	fmt.Fprintf(os.Stderr, "\nmcpzip: shutting down\n")
	return nil
}
