use std::path::Path;

use crate::error::McpzipError;
use crate::types::ToolEntry;

/// Read cached tools from disk.
pub fn read_cache(path: &Path) -> Result<Vec<ToolEntry>, McpzipError> {
    let data = std::fs::read_to_string(path)?;
    let tools: Vec<ToolEntry> = serde_json::from_str(&data)?;
    Ok(tools)
}

/// Write tools to disk cache (pretty-printed JSON).
pub fn write_cache(path: &Path, tools: &[ToolEntry]) -> Result<(), McpzipError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(tools)?;
    std::fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tools() -> Vec<ToolEntry> {
        vec![
            ToolEntry {
                name: "a__tool1".into(),
                server_name: "a".into(),
                original_name: "tool1".into(),
                description: "Tool 1".into(),
                input_schema: json!({"type": "object"}),
                compact_params: "".into(),
            },
            ToolEntry {
                name: "b__tool2".into(),
                server_name: "b".into(),
                original_name: "tool2".into(),
                description: "Tool 2".into(),
                input_schema: json!(null),
                compact_params: "".into(),
            },
        ]
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache").join("tools.json");

        let tools = test_tools();
        write_cache(&path, &tools).unwrap();
        let loaded = read_cache(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "a__tool1");
        assert_eq!(loaded[1].name, "b__tool2");
    }

    #[test]
    fn test_read_missing() {
        assert!(read_cache(Path::new("/nonexistent/tools.json")).is_err());
    }

    #[test]
    fn test_read_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tools.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(read_cache(&path).is_err());
    }

    #[test]
    fn test_write_creates_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep").join("nested").join("tools.json");
        write_cache(&path, &[]).unwrap();
        assert!(path.exists());
    }
}
