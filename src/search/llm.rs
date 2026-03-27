use crate::error::McpzipError;
use crate::types::SearchResult;

/// Stub for LLM-based tool search re-ranking via Gemini.
pub struct GeminiSearcher {
    _api_key: String,
    _model: String,
}

impl GeminiSearcher {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            _api_key: api_key,
            _model: model,
        }
    }

    /// Stub that returns an error indicating LLM search is not yet implemented.
    pub async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchResult>, McpzipError> {
        Err(McpzipError::Transport("LLM search not yet implemented".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stub_returns_error() {
        let gs = GeminiSearcher::new("key".into(), "model".into());
        let result = gs.search("slack", 10).await;
        assert!(result.is_err());
    }
}
