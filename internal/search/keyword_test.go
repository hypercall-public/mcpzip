// https://hypercall.xyz

package search

import (
	"context"
	"testing"

	"github.com/hypercall-public/mcpzip/internal/types"
)

func testCatalog() []types.ToolEntry {
	return []types.ToolEntry{
		{
			Name:          "slack__send_message",
			ServerName:    "slack",
			OriginalName:  "send_message",
			Description:   "Send a message to a Slack channel",
			CompactParams: "channel:string*, message:string*",
		},
		{
			Name:          "slack__channels_list",
			ServerName:    "slack",
			OriginalName:  "channels_list",
			Description:   "List all Slack channels",
			CompactParams: "",
		},
		{
			Name:          "github__create_issue",
			ServerName:    "github",
			OriginalName:  "create_issue",
			Description:   "Create a new issue in a GitHub repository",
			CompactParams: "repo:string*, title:string*, body:string",
		},
		{
			Name:          "github__list_pull_requests",
			ServerName:    "github",
			OriginalName:  "list_pull_requests",
			Description:   "List pull requests for a repository",
			CompactParams: "repo:string*",
		},
		{
			Name:          "notion__search",
			ServerName:    "notion",
			OriginalName:  "search",
			Description:   "Search Notion pages and databases",
			CompactParams: "query:string*",
		},
	}
}

func TestKeywordSearch_ExactNameMatch(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	results, err := ks.Search(context.Background(), "send_message", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) == 0 {
		t.Fatal("expected at least one result")
	}
	if results[0].Name != "slack__send_message" {
		t.Errorf("expected slack__send_message as top result, got %s", results[0].Name)
	}
}

func TestKeywordSearch_PartialTokenMatch(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	results, err := ks.Search(context.Background(), "slack", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 2 {
		t.Fatalf("expected 2 results for 'slack', got %d", len(results))
	}
	// Both slack tools should appear; order is by score desc then name asc.
	names := map[string]bool{}
	for _, r := range results {
		names[r.Name] = true
	}
	if !names["slack__send_message"] || !names["slack__channels_list"] {
		t.Errorf("expected both slack tools, got %v", results)
	}
}

func TestKeywordSearch_NoMatchReturnsEmpty(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	results, err := ks.Search(context.Background(), "kubernetes", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 0 {
		t.Errorf("expected empty results, got %d", len(results))
	}
}

func TestKeywordSearch_LimitRespected(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	// "list" matches channels_list and list_pull_requests at least.
	results, err := ks.Search(context.Background(), "list", 1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 1 {
		t.Errorf("expected 1 result with limit=1, got %d", len(results))
	}
}

func TestKeywordSearch_EmptyQueryReturnsEmpty(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	results, err := ks.Search(context.Background(), "", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 0 {
		t.Errorf("expected empty results for empty query, got %d", len(results))
	}
}

func TestKeywordSearch_CaseInsensitive(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	results, err := ks.Search(context.Background(), "SLACK", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) != 2 {
		t.Fatalf("expected 2 results for 'SLACK', got %d", len(results))
	}
}

func TestKeywordSearch_UnderscoreTokenization(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	// "send" should match "send_message" because underscores are tokenized.
	results, err := ks.Search(context.Background(), "send", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) == 0 {
		t.Fatal("expected at least one result for 'send'")
	}
	if results[0].Name != "slack__send_message" {
		t.Errorf("expected slack__send_message as top result for 'send', got %s", results[0].Name)
	}
}

func TestKeywordSearch_Deterministic(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	// Run the same search multiple times and ensure identical order.
	var prev []types.SearchResult
	for i := 0; i < 5; i++ {
		results, err := ks.Search(context.Background(), "list", 10)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if prev != nil {
			if len(results) != len(prev) {
				t.Fatalf("non-deterministic result count on iteration %d", i)
			}
			for j := range results {
				if results[j].Name != prev[j].Name {
					t.Fatalf("non-deterministic order at position %d on iteration %d: got %s, want %s",
						j, i, results[j].Name, prev[j].Name)
				}
			}
		}
		prev = results
	}
}

func TestKeywordSearch_MultiTokenQuery(t *testing.T) {
	ks := NewKeywordSearcher(testCatalog)
	// "send message" should match send_message with score 2, higher than anything else.
	results, err := ks.Search(context.Background(), "send message", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(results) == 0 {
		t.Fatal("expected results for 'send message'")
	}
	if results[0].Name != "slack__send_message" {
		t.Errorf("expected slack__send_message as top result, got %s", results[0].Name)
	}
}
