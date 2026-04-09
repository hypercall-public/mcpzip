---
sidebar_position: 4
---

# Search

mcpzip uses a two-tier search system to find relevant tools from your catalog.

## Keyword Search

Always active. Uses tokenization and scoring to match queries against tool names, descriptions, and parameter names.

Scoring considers:
- **Exact name matches** — highest weight
- **Token overlap** — matching words between query and tool metadata
- **Server name matches** — the server prefix (e.g., "slack" in "slack__send_message")

Example: searching "send message slack" will score `slack__send_message` highly because it matches on server name ("slack"), tool name ("send_message"), and description tokens.

## Semantic Search (optional)

When a Gemini API key is configured, mcpzip adds LLM-powered semantic search. The LLM receives the query and a compact representation of available tools, then returns the most relevant matches.

This enables natural language queries like:
- "help me manage my todo list" → finds Todoist tools
- "post something to the team" → finds Slack send_message
- "check my upcoming meetings" → finds Google Calendar tools

### How It Works

1. Query arrives at `search_tools`
2. Keyword search runs immediately (fast path)
3. If Gemini is configured, semantic search runs in parallel
4. Results are merged and deduplicated
5. Cached for future identical queries

### Query Cache

Search results are cached by normalized query. Cache keys are computed by:
1. Lowercasing the query
2. Tokenizing into words
3. Sorting tokens
4. Checking for token overlap with cached queries

This means "slack send message" and "send message slack" hit the same cache entry.

## Compact Tool Representation

To minimize tokens sent to the search LLM, mcpzip compresses tool schemas into a compact format:

```
slack__send_message: Send a Slack message [channel:string*, text:string*]
slack__channels_list: List Slack channels [limit:integer]
todoist__create_task: Create a new task [content:string*, project_id:string]
```

The `*` marks required parameters. This representation is much smaller than full JSON schemas while preserving the information the LLM needs to rank relevance.
