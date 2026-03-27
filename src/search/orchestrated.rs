use async_trait::async_trait;

use crate::error::McpzipError;
use crate::search::keyword::{CatalogFn, KeywordSearcher};
use crate::search::llm::GeminiSearcher;
use crate::search::query_cache::QueryCache;
use crate::types::SearchResult;

/// Searcher searches the tool catalog.
#[async_trait]
pub trait Searcher: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, McpzipError>;
}

/// KeywordSearcher implements Searcher.
#[async_trait]
impl Searcher for KeywordSearcher {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, McpzipError> {
        Ok(self.search(query, limit))
    }
}

/// Orchestrated searcher: keyword search as primary, optional LLM re-ranking,
/// with a cache to avoid redundant LLM calls.
pub struct OrchestratedSearcher {
    keyword: KeywordSearcher,
    llm: GeminiSearcher,
    cache: QueryCache,
}

impl OrchestratedSearcher {
    pub fn new(keyword: KeywordSearcher, llm: GeminiSearcher) -> Self {
        Self {
            keyword,
            llm,
            cache: QueryCache::new(),
        }
    }
}

#[async_trait]
impl Searcher for OrchestratedSearcher {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, McpzipError> {
        // Check cache first.
        if let Some(cached) = self.cache.get(query) {
            return Ok(apply_limit(cached, limit));
        }

        // Always get keyword results as fallback.
        let kw_results = self.keyword.search(query, limit);

        // Try LLM re-ranking.
        match self.llm.search(query, limit).await {
            Ok(llm_results) if !llm_results.is_empty() => {
                self.cache.put(query, llm_results.clone());
                Ok(apply_limit(llm_results, limit))
            }
            _ => {
                // LLM failed or returned nothing; use keyword results.
                Ok(kw_results)
            }
        }
    }
}

/// Construct the appropriate Searcher based on configuration.
/// If api_key is empty, returns a KeywordSearcher.
/// If api_key is provided, returns an OrchestratedSearcher.
pub fn new_searcher(api_key: &str, model: &str, catalog_fn: CatalogFn) -> Box<dyn Searcher> {
    let kw = KeywordSearcher::new(catalog_fn);

    if api_key.is_empty() {
        Box::new(kw)
    } else {
        let llm = GeminiSearcher::new(api_key.into(), model.into());
        Box::new(OrchestratedSearcher::new(kw, llm))
    }
}

fn apply_limit(results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    if limit > 0 && results.len() > limit {
        results[..limit].to_vec()
    } else {
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolEntry;
    use serde_json::json;
    use std::sync::Arc;

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

    #[tokio::test]
    async fn test_new_searcher_empty_key_returns_keyword() {
        let s = new_searcher("", "", Arc::new(test_catalog));
        let results = s.search("slack", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_new_searcher_with_key_returns_orchestrated() {
        let s = new_searcher("some-api-key", "gemini-2.0-flash", Arc::new(test_catalog));
        // LLM stub always fails, so this falls back to keyword.
        let results = s.search("slack", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_orchestrated_llm_failure_falls_back_to_keyword() {
        let s = new_searcher("some-api-key", "gemini-2.0-flash", Arc::new(test_catalog));
        let results = s.search("slack", 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_orchestrated_cache_used() {
        let kw = KeywordSearcher::new(Arc::new(test_catalog));
        let llm = GeminiSearcher::new("key".into(), "model".into());
        let o = OrchestratedSearcher::new(kw, llm);

        // Pre-populate cache.
        let cached = vec![SearchResult {
            name: "cached__tool".into(),
            description: "From cache".into(),
            compact_params: "".into(),
        }];
        o.cache.put("slack", cached.clone());

        let results = o.search("slack", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "cached__tool");
    }

    #[tokio::test]
    async fn test_empty_catalog_returns_empty() {
        let empty_fn: CatalogFn = Arc::new(|| Vec::new());

        // Keyword searcher.
        let s = new_searcher("", "", empty_fn.clone());
        let results = s.search("anything", 10).await.unwrap();
        assert!(results.is_empty());

        // Orchestrated searcher.
        let s = new_searcher("key", "model", empty_fn);
        let results = s.search("anything", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_orchestrated_limit_applied() {
        let s = new_searcher("some-api-key", "model", Arc::new(test_catalog));
        let results = s.search("list", 1).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
