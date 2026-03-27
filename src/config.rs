use std::path::{Path, PathBuf};

use crate::error::McpzipError;
use crate::types::ProxyConfig;

const CONFIG_DIR: &str = "compressed-mcp-proxy";
const CONFIG_FILE: &str = "config.json";
const CACHE_DIR: &str = "cache";
const CACHE_FILE: &str = "tools.json";
const AUTH_DIR: &str = "auth";

/// Returns ~/.config/compressed-mcp-proxy/config.json
pub fn default_path() -> PathBuf {
    base_dir().join(CONFIG_FILE)
}

/// Returns ~/.config/compressed-mcp-proxy/cache/tools.json
pub fn cache_path() -> PathBuf {
    base_dir().join(CACHE_DIR).join(CACHE_FILE)
}

/// Returns ~/.config/compressed-mcp-proxy/auth/
pub fn auth_dir() -> PathBuf {
    base_dir().join(AUTH_DIR)
}

/// Returns ~/.config/compressed-mcp-proxy/
/// Uses ~/.config explicitly to match Go behavior (not macOS Library/Application Support).
fn base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join(CONFIG_DIR)
}

/// Load config from the given path.
pub fn load(path: &Path) -> Result<ProxyConfig, McpzipError> {
    let data = std::fs::read_to_string(path)?;
    let cfg: ProxyConfig = serde_json::from_str(&data)?;
    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &ProxyConfig) -> Result<(), McpzipError> {
    if cfg.mcp_servers.is_empty() {
        return Err(McpzipError::Config(
            "at least one MCP server must be defined".into(),
        ));
    }
    for (name, sc) in &cfg.mcp_servers {
        match sc.effective_type() {
            "stdio" => {
                if sc.command.as_ref().map_or(true, |c| c.is_empty()) {
                    return Err(McpzipError::Config(format!(
                        "server {:?}: stdio server must have a command",
                        name
                    )));
                }
            }
            "http" | "sse" => {
                if sc.url.as_ref().map_or(true, |u| u.is_empty()) {
                    return Err(McpzipError::Config(format!(
                        "server {:?}: {} server must have a url",
                        name,
                        sc.effective_type()
                    )));
                }
            }
            other => {
                return Err(McpzipError::Config(format!(
                    "server {:?}: unsupported type {:?} (must be \"stdio\", \"http\", or \"sse\")",
                    name, other
                )));
            }
        }
    }
    Ok(())
}

/// Claude Code config format (for migration).
#[derive(Debug, serde::Deserialize)]
pub struct ClaudeCodeConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: std::collections::HashMap<String, crate::types::ServerConfig>,
}

/// Load Claude Code config from common locations.
pub fn load_claude_code_config() -> Result<ClaudeCodeConfig, McpzipError> {
    for path in claude_code_config_paths() {
        if let Ok(cfg) = load_claude_code_config_from(&path) {
            return Ok(cfg);
        }
    }
    Err(McpzipError::Config(
        "no Claude Code config found with MCP servers".into(),
    ))
}

/// Load Claude Code config from a specific path.
pub fn load_claude_code_config_from(path: &Path) -> Result<ClaudeCodeConfig, McpzipError> {
    let data = std::fs::read_to_string(path)?;
    let cfg: ClaudeCodeConfig = serde_json::from_str(&data)?;
    if cfg.mcp_servers.is_empty() {
        return Err(McpzipError::Config(format!(
            "no MCP servers found in {}",
            path.display()
        )));
    }
    Ok(cfg)
}

/// Find the path to the Claude Code config file.
pub fn find_claude_code_config_path() -> Result<PathBuf, McpzipError> {
    for path in claude_code_config_paths() {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(McpzipError::Config(
        "no Claude Code config found (checked ~/.claude.json and ~/.claude/config.json)".into(),
    ))
}

fn claude_code_config_paths() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    vec![
        home.join(".claude.json"),
        home.join(".claude").join("config.json"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"mcpServers": {{"slack": {{"command": "slack-mcp"}}}}}}"#
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
        assert_eq!(cfg.mcp_servers["slack"].effective_type(), "stdio");
    }

    #[test]
    fn test_load_multiple_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"a": {"command": "a"}, "b": {"type": "http", "url": "https://b.com"}}}"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 2);
    }

    #[test]
    fn test_load_missing_file() {
        let result = load(Path::new("/nonexistent/config.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not json").unwrap();

        assert!(load(&path).is_err());
    }

    #[test]
    fn test_load_empty_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

        let err = load(&path).unwrap_err();
        assert!(err.to_string().contains("at least one"));
    }

    #[test]
    fn test_validate_stdio_no_command() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"mcpServers": {"x": {}}}"#).unwrap();

        let err = load(&path).unwrap_err();
        assert!(err.to_string().contains("must have a command"));
    }

    #[test]
    fn test_validate_http_no_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"x": {"type": "http"}}}"#,
        )
        .unwrap();

        let err = load(&path).unwrap_err();
        assert!(err.to_string().contains("must have a url"));
    }

    #[test]
    fn test_validate_unsupported_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"x": {"type": "grpc", "command": "y"}}}"#,
        )
        .unwrap();

        let err = load(&path).unwrap_err();
        assert!(err.to_string().contains("unsupported type"));
    }

    #[test]
    fn test_config_paths() {
        let dp = default_path();
        assert!(dp.to_string_lossy().contains("compressed-mcp-proxy"));
        assert!(dp.to_string_lossy().ends_with("config.json"));

        let cp = cache_path();
        assert!(cp.to_string_lossy().contains("cache"));
        assert!(cp.to_string_lossy().ends_with("tools.json"));

        let ad = auth_dir();
        assert!(ad.to_string_lossy().ends_with("auth"));
    }

    #[test]
    fn test_config_with_idle_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"idle_timeout_minutes": 3, "mcpServers": {"s": {"command": "x"}}}"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        assert_eq!(cfg.idle_timeout_minutes, Some(3));
    }

    #[test]
    fn test_config_with_gemini_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"gemini_api_key": "test-key", "mcpServers": {"s": {"command": "x"}}}"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        assert_eq!(cfg.gemini_api_key, Some("test-key".into()));
    }

    #[test]
    fn test_config_with_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"s": {"command": "x", "env": {"TOKEN": "abc"}}}}"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        let env = cfg.mcp_servers["s"].env.as_ref().unwrap();
        assert_eq!(env["TOKEN"], "abc");
    }

    #[test]
    fn test_load_claude_code_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"slack": {"command": "slack-mcp"}}}"#,
        )
        .unwrap();

        let cfg = load_claude_code_config_from(&path).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
    }

    #[test]
    fn test_load_claude_code_config_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

        assert!(load_claude_code_config_from(&path).is_err());
    }
}
