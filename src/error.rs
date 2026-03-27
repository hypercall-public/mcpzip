use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpzipError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("server not found: {0}")]
    ServerNotFound(String),

    #[error("HTTP error: {0}")]
    Http(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = McpzipError::ToolNotFound("slack__send".into());
        assert_eq!(err.to_string(), "tool not found: slack__send");

        let err = McpzipError::Timeout(120);
        assert_eq!(err.to_string(), "timeout after 120s");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: McpzipError = io_err.into();
        assert!(matches!(err, McpzipError::Io(_)));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<McpzipError>();
    }
}
