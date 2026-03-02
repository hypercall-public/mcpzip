// https://hypercall.xyz

package types

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"
)

const NameSeparator = "__"

// ToolEntry represents a cached tool from an upstream MCP server.
type ToolEntry struct {
	Name          string          `json:"name"`           // prefixed: "servername__toolname"
	ServerName    string          `json:"server_name"`    // which upstream server
	OriginalName  string          `json:"original_name"`  // unprefixed tool name
	Description   string          `json:"description"`    // tool description
	InputSchema   json.RawMessage `json:"input_schema"`   // full JSON Schema
	CompactParams string          `json:"compact_params"` // "chat_id:string*, message:string*"
}

// SearchResult is returned by the search engine.
type SearchResult struct {
	Name          string `json:"name"`           // prefixed name
	Description   string `json:"description"`    // first sentence
	CompactParams string `json:"compact_params"` // param summary
}

// ServerConfig defines how to connect to an upstream MCP server.
type ServerConfig struct {
	Type    string            `json:"type,omitempty"`    // "stdio" or "http" (default: "stdio")
	Command string            `json:"command,omitempty"` // for stdio
	Args    []string          `json:"args,omitempty"`    // for stdio
	Env     map[string]string `json:"env,omitempty"`     // for stdio
	URL     string            `json:"url,omitempty"`     // for http
}

// EffectiveType returns the server type, defaulting to "stdio".
func (s ServerConfig) EffectiveType() string {
	if s.Type == "" {
		return "stdio"
	}
	return s.Type
}

// SearchConfig holds search engine settings.
type SearchConfig struct {
	DefaultLimit int    `json:"default_limit,omitempty"`
	Model        string `json:"model,omitempty"`
}

// ProxyConfig is the full proxy configuration.
type ProxyConfig struct {
	GeminiAPIKey       string                  `json:"gemini_api_key,omitempty"`
	Search             SearchConfig            `json:"search,omitempty"`
	IdleTimeoutMinutes int                     `json:"idle_timeout_minutes,omitempty"`
	CallTimeoutSeconds int                     `json:"call_timeout_seconds,omitempty"`
	MCPServers         map[string]ServerConfig `json:"mcpServers"`
}

// ServerStatus reports health info for an upstream server.
type ServerStatus struct {
	Name        string `json:"name"`
	Connected   bool   `json:"connected"`
	ToolCount   int    `json:"tool_count"`
	LastRefresh string `json:"last_refresh"`
	Error       string `json:"error,omitempty"`
}

// PrefixedName returns "server__tool".
func PrefixedName(server, tool string) string {
	return server + NameSeparator + tool
}

// ParsePrefixedName splits "server__tool" into (server, tool).
// Splits on the first occurrence of "__".
func ParsePrefixedName(name string) (server, tool string, err error) {
	idx := strings.Index(name, NameSeparator)
	if idx < 0 {
		return "", "", fmt.Errorf("invalid prefixed name %q: missing separator %q", name, NameSeparator)
	}
	return name[:idx], name[idx+len(NameSeparator):], nil
}

// CompactParamsFromSchema generates a compact parameter summary from a JSON Schema.
// Format: "param1:type*, param2:type" where * marks required params.
func CompactParamsFromSchema(schema json.RawMessage) string {
	if len(schema) == 0 {
		return ""
	}

	var s struct {
		Properties map[string]json.RawMessage `json:"properties"`
		Required   []string                   `json:"required"`
	}
	if err := json.Unmarshal(schema, &s); err != nil {
		return ""
	}
	if len(s.Properties) == 0 {
		return ""
	}

	requiredSet := make(map[string]bool, len(s.Required))
	for _, r := range s.Required {
		requiredSet[r] = true
	}

	// Sort parameter names for deterministic output.
	names := make([]string, 0, len(s.Properties))
	for name := range s.Properties {
		names = append(names, name)
	}
	sort.Strings(names)

	parts := make([]string, 0, len(names))
	for _, name := range names {
		typ := extractType(s.Properties[name])
		entry := name + ":" + typ
		if requiredSet[name] {
			entry += "*"
		}
		parts = append(parts, entry)
	}

	return strings.Join(parts, ", ")
}

// extractType pulls a simple type string from a JSON Schema property.
func extractType(raw json.RawMessage) string {
	var prop struct {
		Type  interface{} `json:"type"`
		AnyOf []struct {
			Type string `json:"type"`
		} `json:"anyOf"`
	}
	if err := json.Unmarshal(raw, &prop); err != nil {
		return "any"
	}

	switch t := prop.Type.(type) {
	case string:
		return t
	case []interface{}:
		// type: ["string", "null"] -> "string"
		for _, v := range t {
			if s, ok := v.(string); ok && s != "null" {
				return s
			}
		}
		if len(t) > 0 {
			if s, ok := t[0].(string); ok {
				return s
			}
		}
	}

	// Handle anyOf (e.g., Union[int, str])
	if len(prop.AnyOf) > 0 {
		for _, a := range prop.AnyOf {
			if a.Type != "" && a.Type != "null" {
				return a.Type
			}
		}
		if prop.AnyOf[0].Type != "" {
			return prop.AnyOf[0].Type
		}
	}

	return "any"
}
