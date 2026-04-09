use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::config;
use crate::error::McpzipError;
use crate::types::{ProxyConfig, ServerConfig};

#[derive(Args)]
pub struct MigrateArgs {
    /// Output config file path
    #[arg(long, default_value_os_t = config::default_path())]
    pub config: PathBuf,

    /// Path to Claude Code config (auto-detected if empty)
    #[arg(long)]
    pub claude_config: Option<PathBuf>,

    /// Show what would happen without writing files
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run_migrate(args: &MigrateArgs) -> Result<(), McpzipError> {
    // Find Claude Code config.
    let claude_path = match &args.claude_config {
        Some(p) => p.clone(),
        None => config::find_claude_code_config_path()?,
    };

    // Load Claude Code config.
    let claude_cfg = config::load_claude_code_config_from(&claude_path)?;

    // Find the mcpzip binary path.
    let mcpzip_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "mcpzip".into());

    if args.dry_run {
        println!(
            "Dry run: would migrate {} server(s) from {}",
            claude_cfg.mcp_servers.len(),
            claude_path.display()
        );
        println!("\n1. Write mcpzip config to {}:", args.config.display());
        for (name, sc) in &claude_cfg.mcp_servers {
            println!("   - {} ({})", name, sc.effective_type());
        }
        println!("\n2. Update {}:", claude_path.display());
        println!(
            "   - Remove {} individual server entries",
            claude_cfg.mcp_servers.len()
        );
        println!(
            "   - Add single \"mcpzip\" entry pointing to {}",
            mcpzip_bin
        );
        return Ok(());
    }

    // Step 1: Write the mcpzip proxy config.
    write_proxy_config(&claude_cfg.mcp_servers, &args.config)?;

    // Step 2: Update the Claude Code config.
    update_claude_config(&claude_path, claude_cfg.mcp_servers.len(), &mcpzip_bin)?;

    println!("\nDone! Restart Claude Code to use mcpzip.");
    Ok(())
}

/// Write the mcpzip config file with all migrated servers.
pub fn write_proxy_config(
    servers: &HashMap<String, ServerConfig>,
    output_path: &Path,
) -> Result<(), McpzipError> {
    let proxy_cfg = ProxyConfig {
        gemini_api_key: None,
        search: Default::default(),
        idle_timeout_minutes: None,
        call_timeout_seconds: None,
        mcp_servers: servers.clone(),
    };

    let data = serde_json::to_string_pretty(&proxy_cfg)?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, format!("{}\n", data))?;

    println!(
        "Wrote mcpzip config to {} ({} servers)",
        output_path.display(),
        servers.len()
    );
    for (name, sc) in servers {
        println!("  - {} ({})", name, sc.effective_type());
    }
    Ok(())
}

/// Replace mcpServers in Claude Code config with a single mcpzip entry.
fn update_claude_config(
    claude_path: &Path,
    old_count: usize,
    mcpzip_bin: &str,
) -> Result<(), McpzipError> {
    let data = std::fs::read_to_string(claude_path)?;
    let mut raw: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data)?;

    // Replace mcpServers with single mcpzip entry.
    let mut new_servers = serde_json::Map::new();
    new_servers.insert(
        "mcpzip".into(),
        serde_json::json!({
            "type": "stdio",
            "command": mcpzip_bin,
            "args": ["serve"]
        }),
    );
    raw.insert("mcpServers".into(), serde_json::Value::Object(new_servers));

    let out = serde_json::to_string_pretty(&raw)?;
    std::fs::write(claude_path, format!("{}\n", out))?;

    println!("\nUpdated {}:", claude_path.display());
    println!("  Replaced {} servers with single mcpzip entry", old_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_proxy_config_basic() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("config.json");

        let mut servers = HashMap::new();
        servers.insert(
            "slack".into(),
            ServerConfig {
                server_type: None,
                command: Some("slack-mcp".into()),
                args: Some(vec!["--token".into(), "abc".into()]),
                env: None,
                url: None,
                headers: None,
            },
        );
        servers.insert(
            "github".into(),
            ServerConfig {
                server_type: None,
                command: Some("gh-mcp".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        write_proxy_config(&servers, &output).unwrap();

        let data = std::fs::read_to_string(&output).unwrap();
        let proxy_cfg: ProxyConfig = serde_json::from_str(&data).unwrap();
        assert_eq!(proxy_cfg.mcp_servers.len(), 2);
        assert_eq!(
            proxy_cfg.mcp_servers["slack"].command.as_deref(),
            Some("slack-mcp")
        );
    }

    #[test]
    fn test_write_proxy_config_empty() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("config.json");

        let servers = HashMap::new();
        write_proxy_config(&servers, &output).unwrap();

        let data = std::fs::read_to_string(&output).unwrap();
        let proxy_cfg: ProxyConfig = serde_json::from_str(&data).unwrap();
        assert_eq!(proxy_cfg.mcp_servers.len(), 0);
    }

    #[test]
    fn test_write_proxy_config_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("nested").join("deep").join("config.json");

        let mut servers = HashMap::new();
        servers.insert(
            "test".into(),
            ServerConfig {
                server_type: None,
                command: Some("test-mcp".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        write_proxy_config(&servers, &output).unwrap();
        assert!(output.exists());
    }

    #[test]
    fn test_write_proxy_config_preserves_http() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("config.json");

        let mut servers = HashMap::new();
        servers.insert(
            "remote".into(),
            ServerConfig {
                server_type: Some("http".into()),
                command: None,
                args: None,
                env: None,
                url: Some("http://localhost:8080/mcp".into()),
                headers: None,
            },
        );

        write_proxy_config(&servers, &output).unwrap();

        let data = std::fs::read_to_string(&output).unwrap();
        let proxy_cfg: ProxyConfig = serde_json::from_str(&data).unwrap();
        let s = &proxy_cfg.mcp_servers["remote"];
        assert_eq!(s.server_type.as_deref(), Some("http"));
        assert_eq!(s.url.as_deref(), Some("http://localhost:8080/mcp"));
    }

    #[test]
    fn test_update_claude_config() {
        let dir = tempfile::tempdir().unwrap();
        let claude_path = dir.path().join("claude.json");

        let content = r#"{"mcpServers": {"test": {"command": "test-mcp"}}}"#;
        std::fs::write(&claude_path, content).unwrap();

        update_claude_config(&claude_path, 1, "/usr/local/bin/mcpzip").unwrap();

        let data = std::fs::read_to_string(&claude_path).unwrap();
        let raw: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data).unwrap();
        let servers = raw["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("mcpzip"));
        assert_eq!(servers["mcpzip"]["command"], "/usr/local/bin/mcpzip");
    }
}
