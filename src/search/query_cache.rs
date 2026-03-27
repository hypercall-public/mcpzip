use std::collections::HashMap;
use std::sync::RwLock;

use crate::search::keyword::tokenize;
use crate::types::SearchResult;

/// Caches search results keyed by normalized query strings.
/// Supports exact match and fuzzy matching based on token overlap (60% threshold).
pub struct QueryCache {
    store: RwLock<HashMap<String, Vec<SearchResult>>>,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }

    /// Store results for a normalized query key.
    pub fn put(&self, query: &str, results: Vec<SearchResult>) {
        let key = normalize_query(query);
        self.store.write().unwrap().insert(key, results);
    }

    /// Retrieve cached results. Tries exact match first, then falls back to
    /// token-overlap matching with a 60% threshold.
    pub fn get(&self, query: &str) -> Option<Vec<SearchResult>> {
        let key = normalize_query(query);
        let store = self.store.read().unwrap();

        // Exact match.
        if let Some(results) = store.get(&key) {
            return Some(results.clone());
        }

        // Token overlap matching.
        let query_tokens = tokenize(&key);
        if query_tokens.is_empty() {
            return None;
        }

        for (cached_key, results) in store.iter() {
            let cached_tokens = tokenize(cached_key);
            if cached_tokens.is_empty() {
                continue;
            }

            let cached_set: std::collections::HashSet<&str> =
                cached_tokens.iter().map(|s| s.as_str()).collect();

            let matches = query_tokens
                .iter()
                .filter(|t| cached_set.contains(t.as_str()))
                .count();

            let overlap = matches as f64 / query_tokens.len() as f64;
            if overlap >= 0.6 {
                return Some(results.clone());
            }
        }

        None
    }
}

/// Normalize a query for cache key consistency: lowercase, deduplicate, join with spaces.
fn normalize_query(q: &str) -> String {
    let tokens = tokenize(q);
    if tokens.is_empty() {
        return String::new();
    }
    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results() -> Vec<SearchResult> {
        vec![
            SearchResult {
                name: "slack__send_message".into(),
                description: "Send a message".into(),
                compact_params: "channel:string*".into(),
            },
            SearchResult {
                name: "slack__channels_list".into(),
                description: "List channels".into(),
                compact_params: "".into(),
            },
        ]
    }

    #[test]
    fn test_put_and_get_exact() {
        let c = QueryCache::new();
        let results = sample_results();
        c.put("slack send message", results.clone());

        let got = c.get("slack send message").unwrap();
        assert_eq!(got.len(), results.len());
        for (a, b) in got.iter().zip(results.iter()) {
            assert_eq!(a.name, b.name);
        }
    }

    #[test]
    fn test_normalized_exact_match() {
        let c = QueryCache::new();
        c.put("Slack  Send_Message", sample_results());

        let got = c.get("slack send message");
        assert!(got.is_some());
        assert_eq!(got.unwrap().len(), sample_results().len());
    }

    #[test]
    fn test_overlap_match() {
        let c = QueryCache::new();
        c.put("slack send message", sample_results());

        // 2/3 = 66% overlap >= 60%
        let got = c.get("slack send notification");
        assert!(got.is_some());
        assert_eq!(got.unwrap().len(), sample_results().len());
    }

    #[test]
    fn test_low_overlap_misses() {
        let c = QueryCache::new();
        c.put("slack send message", sample_results());

        // 1/3 = 33% overlap < 60%
        let got = c.get("slack create issue");
        assert!(got.is_none());
    }

    #[test]
    fn test_empty_query_misses() {
        let c = QueryCache::new();
        c.put("slack send message", sample_results());

        assert!(c.get("").is_none());
    }

    #[test]
    fn test_miss_on_empty_cache() {
        let c = QueryCache::new();
        assert!(c.get("anything").is_none());
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        let c = Arc::new(QueryCache::new());
        let mut handles = Vec::new();

        // Concurrent writes.
        for i in 0..100 {
            let c = Arc::clone(&c);
            handles.push(std::thread::spawn(move || {
                let key = format!("query {} tokens here", i);
                c.put(
                    &key,
                    vec![SearchResult {
                        name: format!("tool_{}", i),
                        description: String::new(),
                        compact_params: String::new(),
                    }],
                );
            }));
        }

        // Concurrent reads.
        for i in 0..100 {
            let c = Arc::clone(&c);
            handles.push(std::thread::spawn(move || {
                let key = format!("query {} tokens here", i);
                let _ = c.get(&key);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
