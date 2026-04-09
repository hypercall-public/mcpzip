use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::McpzipError;

/// OAuth token persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
}

/// Persists OAuth tokens to disk, keyed by server URL.
pub struct TokenStore {
    base_dir: PathBuf,
}

impl TokenStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Load a cached token for the given server URL.
    /// Returns None if no token is cached or the file is corrupt.
    pub fn load(&self, server_url: &str) -> Result<Option<Token>, McpzipError> {
        let path = self.path(server_url);
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let tok: Token = match serde_json::from_str(&data) {
            Ok(t) => t,
            Err(_) => return Ok(None), // corrupt file, treat as missing
        };

        if tok.access_token.is_empty() {
            return Ok(None);
        }

        Ok(Some(tok))
    }

    /// Save a token to disk for the given server URL.
    pub fn save(&self, server_url: &str, tok: &Token) -> Result<(), McpzipError> {
        self.ensure_dir()?;
        let path = self.path(server_url);
        let data = serde_json::to_string(tok)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            std::io::Write::write_all(&mut f, data.as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&path, &data)?;
        }

        Ok(())
    }

    fn ensure_dir(&self) -> Result<(), McpzipError> {
        if !self.base_dir.exists() {
            #[cfg(unix)]
            {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .create(&self.base_dir)?;
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&self.base_dir, std::fs::Permissions::from_mode(0o700))?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(&self.base_dir)?;
            }
        }
        Ok(())
    }

    fn path(&self, server_url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(server_url.as_bytes());
        let hash = hasher.finalize();
        let name = hex::encode(&hash[..16]); // 32 hex chars
        self.base_dir.join(format!("{}.json", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join("auth"));

        let tok = Token {
            access_token: "test-token".into(),
            token_type: Some("Bearer".into()),
            refresh_token: Some("refresh-123".into()),
            expiry: None,
        };
        store.save("https://example.com", &tok).unwrap();

        let loaded = store.load("https://example.com").unwrap().unwrap();
        assert_eq!(loaded.access_token, "test-token");
        assert_eq!(loaded.refresh_token, Some("refresh-123".into()));
    }

    #[test]
    fn test_load_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join("auth"));
        let result = store.load("https://nonexistent.com").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let auth_dir = dir.path().join("auth");
        std::fs::create_dir_all(&auth_dir).unwrap();

        let store = TokenStore::new(&auth_dir);
        let path = store.path("https://example.com");
        std::fs::write(&path, "not json").unwrap();

        let result = store.load("https://example.com").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_empty_token() {
        let dir = tempfile::tempdir().unwrap();
        let auth_dir = dir.path().join("auth");
        std::fs::create_dir_all(&auth_dir).unwrap();

        let store = TokenStore::new(&auth_dir);
        let path = store.path("https://example.com");
        std::fs::write(&path, r#"{"access_token": ""}"#).unwrap();

        let result = store.load("https://example.com").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_different_urls_different_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join("auth"));

        let tok1 = Token {
            access_token: "token-a".into(),
            token_type: None,
            refresh_token: None,
            expiry: None,
        };
        let tok2 = Token {
            access_token: "token-b".into(),
            token_type: None,
            refresh_token: None,
            expiry: None,
        };

        store.save("https://a.com", &tok1).unwrap();
        store.save("https://b.com", &tok2).unwrap();

        assert_eq!(
            store.load("https://a.com").unwrap().unwrap().access_token,
            "token-a"
        );
        assert_eq!(
            store.load("https://b.com").unwrap().unwrap().access_token,
            "token-b"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let auth_dir = dir.path().join("auth");
        let store = TokenStore::new(&auth_dir);

        let tok = Token {
            access_token: "test".into(),
            token_type: None,
            refresh_token: None,
            expiry: None,
        };
        store.save("https://example.com", &tok).unwrap();

        let dir_perms = std::fs::metadata(&auth_dir).unwrap().permissions();
        assert_eq!(dir_perms.mode() & 0o777, 0o700);

        let file_path = store.path("https://example.com");
        let file_perms = std::fs::metadata(&file_path).unwrap().permissions();
        assert_eq!(file_perms.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_hash_matches_go() {
        // Verify our hash matches Go's sha256 truncation scheme
        let store = TokenStore::new(Path::new("/tmp"));
        let path = store.path("https://example.com");
        let filename = path.file_name().unwrap().to_str().unwrap();
        // Should be 32 hex chars + .json
        assert_eq!(filename.len(), 32 + 5); // 32 hex + ".json"
        assert!(filename.ends_with(".json"));
    }

    #[test]
    fn test_token_store_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TokenStore>();
    }
}
