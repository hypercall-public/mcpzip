// https://hypercall.xyz

package proxy

import (
	"github.com/hypercall-public/mcpzip/internal/catalog"
	"github.com/hypercall-public/mcpzip/internal/search"
	"github.com/hypercall-public/mcpzip/internal/transport"
)

// Server is the core MCP proxy that exposes 3 meta-tools.
type Server struct {
	catalog   *catalog.Catalog
	searcher  search.Searcher
	transport *transport.Manager
}

// New creates a new proxy server.
func New(cat *catalog.Catalog, searcher search.Searcher, tm *transport.Manager) *Server {
	return &Server{
		catalog:   cat,
		searcher:  searcher,
		transport: tm,
	}
}
