// https://hypercall.xyz

package search

import (
	"context"
	"sort"
	"strings"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// KeywordSearcher scores tools by counting matching tokens in name + description.
type KeywordSearcher struct {
	catalogFn func() []types.ToolEntry
}

// NewKeywordSearcher creates a keyword-based searcher.
func NewKeywordSearcher(catalogFn func() []types.ToolEntry) *KeywordSearcher {
	return &KeywordSearcher{catalogFn: catalogFn}
}

// Search tokenizes the query and scores each tool by token overlap.
func (k *KeywordSearcher) Search(_ context.Context, query string, limit int) ([]types.SearchResult, error) {
	tokens := tokenize(query)
	if len(tokens) == 0 {
		return nil, nil
	}

	catalog := k.catalogFn()

	type scored struct {
		entry types.ToolEntry
		score int
	}

	var results []scored
	for _, entry := range catalog {
		s := scoreEntry(entry, tokens)
		if s > 0 {
			results = append(results, scored{entry: entry, score: s})
		}
	}

	// Sort by score descending, then by name ascending for determinism.
	sort.Slice(results, func(i, j int) bool {
		if results[i].score != results[j].score {
			return results[i].score > results[j].score
		}
		return results[i].entry.Name < results[j].entry.Name
	})

	if limit > 0 && len(results) > limit {
		results = results[:limit]
	}

	out := make([]types.SearchResult, len(results))
	for i, r := range results {
		out[i] = types.SearchResult{
			Name:          r.entry.Name,
			Description:   r.entry.Description,
			CompactParams: r.entry.CompactParams,
		}
	}
	return out, nil
}

// tokenize splits a string into lowercase tokens on whitespace and underscores.
func tokenize(s string) []string {
	s = strings.ToLower(s)
	// Replace underscores with spaces so we can split on whitespace uniformly.
	s = strings.ReplaceAll(s, "_", " ")
	fields := strings.Fields(s)
	// Deduplicate while preserving order.
	seen := make(map[string]bool, len(fields))
	tokens := make([]string, 0, len(fields))
	for _, f := range fields {
		if !seen[f] {
			seen[f] = true
			tokens = append(tokens, f)
		}
	}
	return tokens
}

// scoreEntry counts how many query tokens appear in the tool's name or description.
func scoreEntry(entry types.ToolEntry, queryTokens []string) int {
	// Tokenize the tool's name and description into a searchable string.
	text := strings.ToLower(entry.Name + " " + entry.Description)
	text = strings.ReplaceAll(text, "_", " ")

	score := 0
	for _, token := range queryTokens {
		if strings.Contains(text, token) {
			score++
		}
	}
	return score
}
