// https://hypercall.xyz

package search

import (
	"fmt"
	"sync"
	"testing"

	"github.com/hypercall-public/mcpzip/internal/types"
)

func sampleResults() []types.SearchResult {
	return []types.SearchResult{
		{Name: "slack__send_message", Description: "Send a message", CompactParams: "channel:string*"},
		{Name: "slack__channels_list", Description: "List channels", CompactParams: ""},
	}
}

func TestCache_PutAndGetExact(t *testing.T) {
	c := NewQueryCache()
	results := sampleResults()
	c.Put("slack send message", results)

	got, ok := c.Get("slack send message")
	if !ok {
		t.Fatal("expected cache hit for exact query")
	}
	if len(got) != len(results) {
		t.Fatalf("expected %d results, got %d", len(results), len(got))
	}
	for i := range got {
		if got[i].Name != results[i].Name {
			t.Errorf("result %d: expected %s, got %s", i, results[i].Name, got[i].Name)
		}
	}
}

func TestCache_NormalizedExactMatch(t *testing.T) {
	c := NewQueryCache()
	results := sampleResults()
	c.Put("Slack  Send_Message", results)

	// Same query with different casing/formatting should match.
	got, ok := c.Get("slack send message")
	if !ok {
		t.Fatal("expected cache hit for normalized query")
	}
	if len(got) != len(results) {
		t.Fatalf("expected %d results, got %d", len(results), len(got))
	}
}

func TestCache_OverlapMatch(t *testing.T) {
	c := NewQueryCache()
	results := sampleResults()
	// Cache with 3 tokens: "slack send message"
	c.Put("slack send message", results)

	// Query with 3 tokens, 2 overlap: "slack send notification" -> 2/3 = 66% >= 60%
	got, ok := c.Get("slack send notification")
	if !ok {
		t.Fatal("expected cache hit with 66% overlap")
	}
	if len(got) != len(results) {
		t.Fatalf("expected %d results, got %d", len(results), len(got))
	}
}

func TestCache_LowOverlapMisses(t *testing.T) {
	c := NewQueryCache()
	results := sampleResults()
	// Cache with 3 tokens: "slack send message"
	c.Put("slack send message", results)

	// Query with 3 tokens, only 1 overlaps: "slack create issue" -> 1/3 = 33% < 60%
	_, ok := c.Get("slack create issue")
	if ok {
		t.Error("expected cache miss with only 33% overlap")
	}
}

func TestCache_EmptyQueryMisses(t *testing.T) {
	c := NewQueryCache()
	c.Put("slack send message", sampleResults())

	_, ok := c.Get("")
	if ok {
		t.Error("expected cache miss for empty query")
	}
}

func TestCache_MissOnEmptyCache(t *testing.T) {
	c := NewQueryCache()
	_, ok := c.Get("anything")
	if ok {
		t.Error("expected cache miss on empty cache")
	}
}

func TestCache_ThreadSafety(t *testing.T) {
	c := NewQueryCache()
	var wg sync.WaitGroup

	// Concurrent writes.
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			key := fmt.Sprintf("query %d tokens here", i)
			c.Put(key, []types.SearchResult{
				{Name: fmt.Sprintf("tool_%d", i)},
			})
		}(i)
	}

	// Concurrent reads.
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			key := fmt.Sprintf("query %d tokens here", i)
			c.Get(key)
		}(i)
	}

	wg.Wait()
	// If we get here without a panic/race, the test passes.
}
