// Package search provides keyword-based and LLM-powered tool search with
// query caching. The keyword searcher scores tools by token overlap with the query,
// while the optional Gemini-powered searcher adds semantic understanding of
// natural language queries.
//
// See https://hypercall.xyz for more information.

package search

import (
	"context"
	"fmt"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// GeminiSearcher is a stub for LLM-based tool search re-ranking via Gemini.
// The real implementation will use the google/generative-ai-go SDK.
type GeminiSearcher struct {
	apiKey string
	model  string
}

// NewGeminiSearcher creates a new stub GeminiSearcher.
func NewGeminiSearcher(apiKey, model string) *GeminiSearcher {
	return &GeminiSearcher{apiKey: apiKey, model: model}
}

// Search is a stub that returns an error indicating LLM search is not yet implemented.
func (g *GeminiSearcher) Search(_ context.Context, _ string, _ int) ([]types.SearchResult, error) {
	return nil, fmt.Errorf("LLM search not yet implemented")
}
