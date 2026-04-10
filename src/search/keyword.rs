use std::sync::Arc;

use crate::types::{SearchResult, ToolEntry};

/// Catalog function type: returns all tools.
pub type CatalogFn = Arc<dyn Fn() -> Vec<ToolEntry> + Send + Sync>;

/// Keyword-based tool searcher. Scores tools by counting matching tokens
/// in name + description.
pub struct KeywordSearcher {
    catalog_fn: CatalogFn,
}

impl KeywordSearcher {
    pub fn new(catalog_fn: CatalogFn) -> Self {
        Self { catalog_fn }
    }

    /// Search tokenizes the query and scores each tool by token overlap.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let tokens = tokenize(query);
        if tokens.is_empty() {
            return Vec::new();
        }

        let catalog = (self.catalog_fn)();

        let mut scored: Vec<(ToolEntry, usize)> = catalog
            .into_iter()
            .filter_map(|entry| {
                let s = score_entry(&entry, &tokens);
                if s > 0 {
                    Some((entry, s))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending, then by name ascending for determinism.
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

        if limit > 0 && scored.len() > limit {
            scored.truncate(limit);
        }

        scored
            .into_iter()
            .map(|(entry, _)| SearchResult {
                name: entry.name,
                description: entry.description,
                compact_params: entry.compact_params,
            })
            .collect()
    }
}

/// Tokenize splits a string into lowercase tokens on whitespace and underscores.
/// Deduplicates while preserving order.
pub fn tokenize(s: &str) -> Vec<String> {
    let lower = s.to_lowercase().replace('_', " ");
    let mut seen = std::collections::HashSet::new();
    let mut tokens = Vec::new();
    for word in lower.split_whitespace() {
        if seen.insert(word.to_string()) {
            tokens.push(word.to_string());
        }
    }
    tokens
}

/// Score a tool entry against query tokens by counting how many tokens
/// appear in the tool's name + description.
fn score_entry(entry: &ToolEntry, query_tokens: &[String]) -> usize {
    let text = format!("{} {}", entry.name, entry.description)
        .to_lowercase()
        .replace('_', " ");

    query_tokens
        .iter()
        .filter(|token| text.contains(token.as_str()))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_catalog() -> Vec<ToolEntry> {
        vec![
            ToolEntry {
                name: "slack__send_message".into(),
                server_name: "slack".into(),
                original_name: "send_message".into(),
                description: "Send a message to a Slack channel".into(),
                input_schema: json!(null),
                compact_params: "channel:string*, message:string*".into(),
            },
            ToolEntry {
                name: "slack__channels_list".into(),
                server_name: "slack".into(),
                original_name: "channels_list".into(),
                description: "List all Slack channels".into(),
                input_schema: json!(null),
                compact_params: "".into(),
            },
            ToolEntry {
                name: "github__create_issue".into(),
                server_name: "github".into(),
                original_name: "create_issue".into(),
                description: "Create a new issue in a GitHub repository".into(),
                input_schema: json!(null),
                compact_params: "repo:string*, title:string*, body:string".into(),
            },
            ToolEntry {
                name: "github__list_pull_requests".into(),
                server_name: "github".into(),
                original_name: "list_pull_requests".into(),
                description: "List pull requests for a repository".into(),
                input_schema: json!(null),
                compact_params: "repo:string*".into(),
            },
            ToolEntry {
                name: "notion__search".into(),
                server_name: "notion".into(),
                original_name: "search".into(),
                description: "Search Notion pages and databases".into(),
                input_schema: json!(null),
                compact_params: "query:string*".into(),
            },
        ]
    }

    fn make_searcher() -> KeywordSearcher {
        KeywordSearcher::new(Arc::new(test_catalog))
    }

    #[test]
    fn test_exact_name_match() {
        let ks = make_searcher();
        let results = ks.search("send_message", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "slack__send_message");
    }

    #[test]
    fn test_partial_token_match() {
        let ks = make_searcher();
        let results = ks.search("slack", 10);
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"slack__send_message"));
        assert!(names.contains(&"slack__channels_list"));
    }

    #[test]
    fn test_no_match_returns_empty() {
        let ks = make_searcher();
        let results = ks.search("kubernetes", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_limit_respected() {
        let ks = make_searcher();
        let results = ks.search("list", 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let ks = make_searcher();
        let results = ks.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        let ks = make_searcher();
        let results = ks.search("SLACK", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_underscore_tokenization() {
        let ks = make_searcher();
        let results = ks.search("send", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "slack__send_message");
    }

    #[test]
    fn test_deterministic() {
        let ks = make_searcher();
        let first = ks.search("list", 10);
        for _ in 0..5 {
            let results = ks.search("list", 10);
            assert_eq!(results.len(), first.len());
            for (a, b) in results.iter().zip(first.iter()) {
                assert_eq!(a.name, b.name);
            }
        }
    }

    #[test]
    fn test_multi_token_query() {
        let ks = make_searcher();
        let results = ks.search("send message", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "slack__send_message");
    }

    #[test]
    fn test_tokenize_basic() {
        assert_eq!(tokenize("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_underscores() {
        assert_eq!(tokenize("send_message"), vec!["send", "message"]);
    }

    #[test]
    fn test_tokenize_dedup() {
        assert_eq!(tokenize("hello hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_empty() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
    }

    // --- New tests ---

    #[test]
    fn test_mixed_separator_tokenization() {
        // Query with mixed separators: hyphens, underscores, spaces
        let tokens = tokenize("send_message list-channels");
        assert_eq!(tokens, vec!["send", "message", "list-channels"]);
    }

    #[test]
    fn test_tokenize_unicode() {
        let tokens = tokenize("búsqueda herramienta");
        assert_eq!(tokens, vec!["búsqueda", "herramienta"]);
    }

    #[test]
    fn test_tokenize_only_underscores() {
        let tokens = tokenize("___");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_description_match() {
        let ks = make_searcher();
        // "repository" appears in github__create_issue description but not in the name
        let results = ks.search("repository", 10);
        assert!(!results.is_empty());
        // Both GitHub tools mention "repository"
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"github__create_issue"));
        assert!(names.contains(&"github__list_pull_requests"));
    }

    #[test]
    fn test_zero_limit_returns_all() {
        let ks = make_searcher();
        // With limit=0, should return all matches (no truncation)
        let results = ks.search("list", 0);
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_single_char_query() {
        let ks = make_searcher();
        // Single char "a" should match tools containing "a" in name or description
        let results = ks.search("a", 10);
        // "a" appears in many entries: slack, channels, create, databases, etc.
        assert!(!results.is_empty());
    }

    #[test]
    fn test_empty_catalog() {
        let empty_fn: CatalogFn = Arc::new(|| Vec::new());
        let ks = KeywordSearcher::new(empty_fn);
        let results = ks.search("slack", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_whitespace_query() {
        let ks = make_searcher();
        let results = ks.search("   ", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_result_has_compact_params() {
        let ks = make_searcher();
        let results = ks.search("send_message", 1);
        assert!(!results.is_empty());
        assert_eq!(
            results[0].compact_params,
            "channel:string*, message:string*"
        );
    }

    #[test]
    fn test_multi_token_scoring_order() {
        let ks = make_searcher();
        // "slack send message" has 3 tokens; send_message should match all 3
        let results = ks.search("slack send message", 10);
        assert!(!results.is_empty());
        // send_message should score highest (3 tokens match: slack, send, message)
        assert_eq!(results[0].name, "slack__send_message");
    }
}
