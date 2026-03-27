// https://hypercall.xyz

package search

import (
	"context"
	"testing"

	"github.com/jake/mcpzip/internal/types"
)

func TestNewSearcher_EmptyKeyReturnsKeywordSearcher(t *testing.T) {
	s := NewSearcher("", "", testCatalog)
	if _, ok := s.(*KeywordSearcher); !ok {
		t.Errorf("expected *KeywordSearcher, got %T", s)
	}
}

func TestNewSearcher_WithKeyReturnsOrchestratedSearcher(t *testing.T) {
	s := NewSearcher("some-api-key", "gemini-2.0-flash", testCatalog)
	if _, ok := s.(*OrchestratedSearcher); !ok {
		t.Errorf("expected *OrchestratedSearcher, got %T", s)
	}
}

func TestOrchestratedSearcher_LLMFailureFallsBackToKeyword(t *testing.T) {
	s := NewSearcher("some-api-key", "gemini-2.0-flash", testCatalog)
	// The GeminiSearcher stub always returns an error, so this should fall back to keyword results.
	results, err := s.Search(context.Background(), "slack", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) == 0 {
		t.Fatal("expected keyword fallback results, got none")
	}
	// Should have 2 slack results from keyword search.
	if len(results) != 2 {
		t.Errorf("expected 2 fallback results, got %d", len(results))
	}
}

func TestOrchestratedSearcher_CacheUsed(t *testing.T) {
	catalogFn := testCatalog
	o := &OrchestratedSearcher{
		keyword: NewKeywordSearcher(catalogFn),
		llm:     NewGeminiSearcher("key", "model"),
		cache:   NewQueryCache(),
	}

	// Pre-populate the cache with custom results.
	cached := []types.SearchResult{
		{Name: "cached__tool", Description: "From cache", CompactParams: ""},
	}
	o.cache.Put("slack", cached)

	results, err := o.Search(context.Background(), "slack", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 1 || results[0].Name != "cached__tool" {
		t.Errorf("expected cached result, got %v", results)
	}
}

func TestSearcher_EmptyCatalogReturnsEmpty(t *testing.T) {
	emptyCatalog := func() []types.ToolEntry { return nil }

	// Test with keyword searcher.
	s := NewSearcher("", "", emptyCatalog)
	results, err := s.Search(context.Background(), "anything", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 0 {
		t.Errorf("expected empty results from empty catalog, got %d", len(results))
	}

	// Test with orchestrated searcher.
	s = NewSearcher("key", "model", emptyCatalog)
	results, err = s.Search(context.Background(), "anything", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 0 {
		t.Errorf("expected empty results from empty catalog, got %d", len(results))
	}
}

func TestOrchestratedSearcher_LimitApplied(t *testing.T) {
	s := NewSearcher("some-api-key", "model", testCatalog)
	results, err := s.Search(context.Background(), "list", 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 1 {
		t.Errorf("expected 1 result with limit=1, got %d", len(results))
	}
}
