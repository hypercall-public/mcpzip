// https://hypercall.xyz

package proxy

import (
	"context"
	"testing"
)

func TestPrefixURI(t *testing.T) {
	got := PrefixURI("slack", "file:///channels.json")
	want := "slack__file:///channels.json"
	if got != want {
		t.Errorf("PrefixURI = %q, want %q", got, want)
	}
}

func TestParsePrefixedURI(t *testing.T) {
	server, uri, err := ParsePrefixedURI("slack__file:///channels.json")
	if err != nil {
		t.Fatal(err)
	}
	if server != "slack" || uri != "file:///channels.json" {
		t.Errorf("got (%q, %q), want (slack, file:///channels.json)", server, uri)
	}
}

func TestParsePrefixedURI_Invalid(t *testing.T) {
	_, _, err := ParsePrefixedURI("no-separator")
	if err == nil {
		t.Error("expected error for URI without separator")
	}
}

func TestListResources_Empty(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()
	resources, err := s.ListResources(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(resources) != 0 {
		t.Errorf("expected empty resources, got %d", len(resources))
	}
}

func TestListPrompts_Empty(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()
	prompts, err := s.ListPrompts(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(prompts) != 0 {
		t.Errorf("expected empty prompts, got %d", len(prompts))
	}
}

func TestReadResource_NotImplemented(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()
	_, err := s.ReadResource(context.Background(), "slack__file:///test")
	if err == nil {
		t.Error("expected not-implemented error")
	}
}

func TestGetPrompt_NotImplemented(t *testing.T) {
	s := setupTestServer()
	defer s.transport.Close()
	_, err := s.GetPrompt(context.Background(), "slack__greeting")
	if err == nil {
		t.Error("expected not-implemented error")
	}
}
