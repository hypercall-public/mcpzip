// https://hypercall.xyz

package types

import (
	"encoding/json"
	"testing"
)

func TestPrefixedName(t *testing.T) {
	tests := []struct {
		server, tool, want string
	}{
		{"server", "tool", "server__tool"},
		{"telegram-jakesyl", "send_message", "telegram-jakesyl__send_message"},
		{"slack", "channels_list", "slack__channels_list"},
	}
	for _, tt := range tests {
		got := PrefixedName(tt.server, tt.tool)
		if got != tt.want {
			t.Errorf("PrefixedName(%q, %q) = %q, want %q", tt.server, tt.tool, got, tt.want)
		}
	}
}

func TestParsePrefixedName(t *testing.T) {
	tests := []struct {
		name       string
		wantServer string
		wantTool   string
		wantErr    bool
	}{
		{"server__tool", "server", "tool", false},
		{"telegram-jakesyl__send_message", "telegram-jakesyl", "send_message", false},
		{"my-server__my_tool__with__underscores", "my-server", "my_tool__with__underscores", false},
		{"invalidname", "", "", true},
		{"", "", "", true},
	}
	for _, tt := range tests {
		server, tool, err := ParsePrefixedName(tt.name)
		if (err != nil) != tt.wantErr {
			t.Errorf("ParsePrefixedName(%q) error = %v, wantErr %v", tt.name, err, tt.wantErr)
			continue
		}
		if server != tt.wantServer || tool != tt.wantTool {
			t.Errorf("ParsePrefixedName(%q) = (%q, %q), want (%q, %q)", tt.name, server, tool, tt.wantServer, tt.wantTool)
		}
	}
}

func TestCompactParamsFromSchema_Simple(t *testing.T) {
	schema := json.RawMessage(`{
		"type": "object",
		"properties": {
			"chat_id": {"type": "string"},
			"message": {"type": "string"}
		},
		"required": ["chat_id", "message"]
	}`)
	got := CompactParamsFromSchema(schema)
	want := "chat_id:string*, message:string*"
	if got != want {
		t.Errorf("CompactParamsFromSchema = %q, want %q", got, want)
	}
}

func TestCompactParamsFromSchema_Optional(t *testing.T) {
	schema := json.RawMessage(`{
		"type": "object",
		"properties": {
			"chat_id": {"type": "string"},
			"limit": {"type": "integer"}
		},
		"required": ["chat_id"]
	}`)
	got := CompactParamsFromSchema(schema)
	want := "chat_id:string*, limit:integer"
	if got != want {
		t.Errorf("CompactParamsFromSchema = %q, want %q", got, want)
	}
}

func TestCompactParamsFromSchema_Empty(t *testing.T) {
	schema := json.RawMessage(`{"type": "object"}`)
	got := CompactParamsFromSchema(schema)
	if got != "" {
		t.Errorf("CompactParamsFromSchema(empty) = %q, want empty", got)
	}
}

func TestCompactParamsFromSchema_AnyOf(t *testing.T) {
	schema := json.RawMessage(`{
		"type": "object",
		"properties": {
			"chat_id": {"anyOf": [{"type": "integer"}, {"type": "string"}]}
		},
		"required": ["chat_id"]
	}`)
	got := CompactParamsFromSchema(schema)
	want := "chat_id:integer*"
	if got != want {
		t.Errorf("CompactParamsFromSchema(anyOf) = %q, want %q", got, want)
	}
}

func TestCompactParamsFromSchema_NullableType(t *testing.T) {
	schema := json.RawMessage(`{
		"type": "object",
		"properties": {
			"name": {"type": ["string", "null"]}
		}
	}`)
	got := CompactParamsFromSchema(schema)
	want := "name:string"
	if got != want {
		t.Errorf("CompactParamsFromSchema(nullable) = %q, want %q", got, want)
	}
}

func TestCompactParamsFromSchema_InvalidJSON(t *testing.T) {
	got := CompactParamsFromSchema(json.RawMessage(`invalid`))
	if got != "" {
		t.Errorf("CompactParamsFromSchema(invalid) = %q, want empty", got)
	}
}

func TestCompactParamsFromSchema_Nil(t *testing.T) {
	got := CompactParamsFromSchema(nil)
	if got != "" {
		t.Errorf("CompactParamsFromSchema(nil) = %q, want empty", got)
	}
}

func TestServerConfig_EffectiveType(t *testing.T) {
	if (ServerConfig{}).EffectiveType() != "stdio" {
		t.Error("empty type should default to stdio")
	}
	if (ServerConfig{Type: "http"}).EffectiveType() != "http" {
		t.Error("explicit http should be http")
	}
	if (ServerConfig{Type: "stdio"}).EffectiveType() != "stdio" {
		t.Error("explicit stdio should be stdio")
	}
}
