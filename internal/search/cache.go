// https://hypercall.xyz

package search

import (
	"strings"
	"sync"

	"github.com/hypercall-public/mcpzip/internal/types"
)

// QueryCache caches search results keyed by normalized query strings.
// It supports exact match and fuzzy matching based on token overlap.
type QueryCache struct {
	mu    sync.RWMutex
	store map[string][]types.SearchResult
}

// NewQueryCache creates an empty query cache.
func NewQueryCache() *QueryCache {
	return &QueryCache{
		store: make(map[string][]types.SearchResult),
	}
}

// Put stores results for a normalized query key.
func (c *QueryCache) Put(query string, results []types.SearchResult) {
	key := normalizeQuery(query)
	c.mu.Lock()
	defer c.mu.Unlock()
	c.store[key] = results
}

// Get retrieves cached results. It first tries an exact match on the normalized
// query, then falls back to token-overlap matching with a 60% threshold.
func (c *QueryCache) Get(query string) ([]types.SearchResult, bool) {
	key := normalizeQuery(query)
	c.mu.RLock()
	defer c.mu.RUnlock()

	// Exact match.
	if results, ok := c.store[key]; ok {
		return results, true
	}

	// Token overlap matching.
	queryTokens := tokenize(key)
	if len(queryTokens) == 0 {
		return nil, false
	}

	for cachedKey, results := range c.store {
		cachedTokens := tokenize(cachedKey)
		if len(cachedTokens) == 0 {
			continue
		}

		// Count how many of the new query's tokens appear in the cached key's tokens.
		cachedSet := make(map[string]bool, len(cachedTokens))
		for _, t := range cachedTokens {
			cachedSet[t] = true
		}

		matches := 0
		for _, t := range queryTokens {
			if cachedSet[t] {
				matches++
			}
		}

		overlap := float64(matches) / float64(len(queryTokens))
		if overlap >= 0.6 {
			return results, true
		}
	}

	return nil, false
}

// normalizeQuery lowercases and trims a query for cache key consistency.
func normalizeQuery(q string) string {
	tokens := tokenize(q)
	if len(tokens) == 0 {
		return ""
	}
	return joinTokens(tokens)
}

// joinTokens joins tokens with a single space.
func joinTokens(tokens []string) string {
	return strings.Join(tokens, " ")
}
