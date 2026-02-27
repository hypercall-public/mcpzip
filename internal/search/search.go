// https://hypercall.xyz

package search

import (
	"context"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// Searcher searches the tool catalog.
type Searcher interface {
	Search(ctx context.Context, query string, limit int) ([]types.SearchResult, error)
}

// OrchestratedSearcher performs keyword search as the primary strategy,
// and optionally re-ranks results via an LLM searcher. A cache is used
// to avoid redundant LLM calls.
type OrchestratedSearcher struct {
	keyword *KeywordSearcher
	llm     Searcher
	cache   *QueryCache
}

// NewSearcher constructs the appropriate Searcher based on configuration.
// If apiKey is empty, a plain KeywordSearcher is returned.
// If apiKey is provided, an OrchestratedSearcher wrapping keyword + LLM + cache is returned.
func NewSearcher(apiKey, model string, catalogFn func() []types.ToolEntry) Searcher {
	kw := NewKeywordSearcher(catalogFn)

	if apiKey == "" {
		return kw
	}

	return &OrchestratedSearcher{
		keyword: kw,
		llm:     NewGeminiSearcher(apiKey, model),
		cache:   NewQueryCache(),
	}
}

// Search first checks the LLM cache, then tries LLM re-ranking, falling back
// to keyword search if the LLM fails.
func (o *OrchestratedSearcher) Search(ctx context.Context, query string, limit int) ([]types.SearchResult, error) {
	// Check cache first.
	if cached, ok := o.cache.Get(query); ok {
		return applyLimit(cached, limit), nil
	}

	// Always get keyword results as a fallback.
	kwResults, kwErr := o.keyword.Search(ctx, query, limit)

	// Try LLM re-ranking.
	llmResults, llmErr := o.llm.Search(ctx, query, limit)
	if llmErr == nil && len(llmResults) > 0 {
		o.cache.Put(query, llmResults)
		return applyLimit(llmResults, limit), nil
	}

	// LLM failed or returned nothing; fall back to keyword results.
	if kwErr != nil {
		return nil, kwErr
	}
	return kwResults, nil
}

// applyLimit truncates results to the given limit.
func applyLimit(results []types.SearchResult, limit int) []types.SearchResult {
	if limit > 0 && len(results) > limit {
		return results[:limit]
	}
	return results
}
