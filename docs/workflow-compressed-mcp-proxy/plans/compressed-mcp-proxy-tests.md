# Test Cases: mcpzip

## Unit Tests

### Types Tests (`internal/types/types_test.go`)
- TestPrefixedName: "server" + "tool" -> "server__tool"
- TestPrefixedName_SpecialChars: "telegram-jakesyl" + "send_message" -> "telegram-jakesyl__send_message"
- TestParsePrefixedName: "server__tool" -> ("server", "tool", nil)
- TestParsePrefixedName_MultipleUnderscores: "my-server__my_tool_name" -> ("my-server", "my_tool_name", nil)
- TestParsePrefixedName_NoSeparator: "invalidname" -> ("", "", error)
- TestCompactParamsFromSchema_Simple: {chat_id: string, message: string} required:[chat_id,message] -> "chat_id:string*, message:string*"
- TestCompactParamsFromSchema_Optional: {chat_id: string, limit: integer} required:[chat_id] -> "chat_id:string*, limit:integer"
- TestCompactParamsFromSchema_Empty: {} -> ""
- TestCompactParamsFromSchema_ComplexTypes: anyOf, arrays -> simplified type strings

### Config Tests (`internal/config/config_test.go`)
- TestLoad_ValidConfig: complete valid JSON -> parsed ProxyConfig
- TestLoad_MinimalConfig: just one server -> works with defaults
- TestLoad_MissingFile: nonexistent path -> error
- TestLoad_InvalidJSON: malformed JSON -> error
- TestLoad_NoServers: empty mcpServers -> validation error
- TestLoad_InvalidServerType: type "unknown" -> validation error
- TestLoad_StdioNoCommand: stdio server without command -> validation error
- TestLoad_HttpNoURL: http server without url -> validation error
- TestDefaultPath: returns ~/.config/compressed-mcp-proxy/config.json (with home expansion)
- TestCachePath: returns ~/.config/compressed-mcp-proxy/cache/tools.json
- TestLoadClaudeCodeConfig: parse real ~/.claude.json format

### Catalog Tests (`internal/catalog/catalog_test.go`)
- TestCatalog_LoadFromCache: valid cache file -> populated catalog
- TestCatalog_LoadMissingCache: no cache file -> empty catalog, no error
- TestCatalog_LoadCorruptCache: invalid JSON -> error
- TestCatalog_AllTools: returns all tools from all servers, prefixed
- TestCatalog_GetTool_Found: existing prefixed name -> ToolEntry
- TestCatalog_GetTool_NotFound: unknown name -> error
- TestCatalog_ServerStatus: returns status for each configured server
- TestCatalog_SaveAndLoad: save cache, reload, verify identical
- TestCatalog_RefreshAll_Success: mock upstream returns tools -> catalog updated
- TestCatalog_RefreshAll_PartialFailure: one server times out, others succeed -> partial update
- TestCatalog_RefreshAll_Pagination: upstream returns nextCursor -> fetches all pages
- TestCatalog_ConcurrentAccess: read catalog while refresh is running -> no race

### Search Tests

#### KeywordSearcher (`internal/search/keyword_test.go`)
- TestKeyword_ExactNameMatch: query "send_message" matches tool "send_message" highly
- TestKeyword_PartialMatch: query "send telegram" matches "telegram-jakesyl__send_message"
- TestKeyword_NoMatch: query "database query" with only telegram tools -> empty results
- TestKeyword_Ranking: more keyword overlap ranks higher
- TestKeyword_CaseInsensitive: "Send Message" matches "send_message"
- TestKeyword_LimitRespected: limit=3 returns at most 3 results
- TestKeyword_EmptyCatalog: empty tool list -> empty results
- TestKeyword_EmptyQuery: empty query -> empty results

#### QueryCache (`internal/search/cache_test.go`)
- TestCache_StoreAndRetrieve: exact query -> cached result
- TestCache_NormalizedMatch: "Send Message" matches cache for "send message"
- TestCache_KeywordOverlapHit: "send telegram msg" matches "send telegram message" (>60% overlap)
- TestCache_KeywordOverlapMiss: "list contacts" does NOT match "send telegram message" (<60% overlap)
- TestCache_EmptyCache: no entries -> nil (cache miss)
- TestCache_ConcurrentAccess: store and retrieve from multiple goroutines

#### GeminiSearcher (`internal/search/llm_test.go`)
- TestGemini_SearchSuccess: mock Gemini returns JSON array of tool names -> mapped to SearchResults
- TestGemini_ParsesResponse: validates JSON parsing of Gemini output
- TestGemini_APIError: Gemini returns error -> propagated
- TestGemini_InvalidResponse: Gemini returns non-JSON -> error
- TestGemini_LimitPassedToPrompt: limit parameter included in prompt

#### OrchestratedSearcher (`internal/search/search_test.go`)
- TestOrchestrated_CacheHit: cached query -> returns cached, no LLM call
- TestOrchestrated_CacheMiss_LLMSuccess: uncached -> calls LLM, caches result
- TestOrchestrated_CacheMiss_LLMFail_KeywordFallback: LLM fails -> keyword results returned
- TestOrchestrated_NoAPIKey: created without key -> uses keyword searcher directly
- TestOrchestrated_ResultsCached: second identical query -> no LLM call

### Transport Tests

#### Manager (`internal/transport/manager_test.go`)
- TestManager_GetConnection_CreatesNew: first call -> creates connection
- TestManager_GetConnection_ReusesExisting: second call -> same connection
- TestManager_GetConnection_UnknownServer: server not in config -> error
- TestManager_IdleReaper: connection unused > timeout -> closed
- TestManager_IdleReaper_ActiveNotReaped: recently used connection -> kept
- TestManager_Close: all connections closed, reaper stopped
- TestManager_CallTool_ForwardsCorrectly: tool call -> correct upstream

#### StdioUpstream (`internal/transport/stdio_test.go`)
- TestStdio_Connect: spawn mock server -> connected, initialized
- TestStdio_CallTool: call tool -> correct result returned
- TestStdio_Close: child process killed
- TestStdio_ServerCrash: process dies -> error on next call
- TestStdio_RetryOnFailure: first connect fails, retry succeeds

#### HTTPUpstream (`internal/transport/http_test.go`)
- TestHTTP_Connect: connect to mock HTTP server -> initialized
- TestHTTP_CallTool: call tool -> correct result returned
- TestHTTP_Close: disconnected
- TestHTTP_ConnectionRefused: server not running -> error

### CLI Tests (`internal/cli/`)
- TestMigrate_ReadsClaudeConfig: parses test Claude Code config
- TestMigrate_GeneratesProxyConfig: correct proxy config output
- TestMigrate_UpdatesClaudeConfig: replaces servers with mcpzip entry
- TestMigrate_PreservesNonMCPConfig: other Claude Code settings untouched

## Integration Tests

### Proxy Integration (`internal/proxy/proxy_integration_test.go`)
- TestProxy_SearchAndExecute: start proxy with mock upstream, search for tool, execute it, verify result matches upstream
- TestProxy_MultipleUpstreams: two mock servers, tools from both searchable, execute routes to correct one
- TestProxy_DescribeTool: search, describe, verify full schema returned
- TestProxy_UnknownTool: execute_tool with invalid name -> clear error
- TestProxy_UpstreamFailure: one upstream down -> search still returns other tools, execute for down server -> error message
- TestProxy_CatalogRefresh: modify upstream tools, trigger refresh, verify new tools appear in search
- TestProxy_ResourceForwarding: upstream has resources, proxy lists them aggregated, read routes correctly
- TestProxy_PromptForwarding: same as resources but for prompts
- TestProxy_AdminTools: search for "proxy status" -> finds proxy_status tool, execute it -> returns status
- TestProxy_AdminToolsNotInList: tools/list does NOT include proxy_status or proxy_refresh

## E2E Tests

### Happy Path
- E2E_FullWorkflow: start proxy with real config (test servers), search for tool, describe it, execute it, verify real response
- E2E_MigrateAndServe: run migrate on test Claude Code config, then start proxy with generated config

### Error Scenarios
- E2E_NoGeminiKey: start without Gemini key -> falls back to keyword search, still works
- E2E_UpstreamTimeout: upstream server hangs -> timeout error, other servers still work
- E2E_InvalidConfig: malformed config -> clear error message on startup

### Edge Cases
- E2E_EmptyCatalog: no upstream servers configured -> proxy starts, search returns empty
- E2E_DuplicateToolNames: two servers with same tool name -> both appear with prefixes
- E2E_LargeToolCatalog: 500 tools across 10 servers -> search still works correctly

### Token Savings Verification
- E2E_TokenCount: compare token count of 3 meta-tool schemas vs full upstream tool schemas
- E2E_TokenSavings: verify >90% reduction for typical tool counts (100+ tools)
