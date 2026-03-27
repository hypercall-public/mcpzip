// https://hypercall.xyz

package proxy

import (
	"strings"
	"testing"

	"github.com/jake/mcpzip/internal/catalog"
)

func setupEmptyCatalog() *catalog.Catalog {
	return catalog.New(nil, "")
}

func TestInstructions_WithServers(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()
	instructions := s.Instructions()
	if !strings.Contains(instructions, "slack") {
		t.Error("instructions should mention slack server")
	}
	if !strings.Contains(instructions, "search_tools") {
		t.Error("instructions should mention search_tools")
	}
}

func TestInstructions_Empty(t *testing.T) {
	// Create a server with no tools loaded.
	s := &Server{
		catalog: setupEmptyCatalog(),
	}
	instructions := s.Instructions()
	if !strings.Contains(instructions, "search_tools") {
		t.Error("instructions should mention search_tools")
	}
}
