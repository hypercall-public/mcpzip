use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::catalog::cache;
use crate::error::McpzipError;
use crate::types::ToolEntry;

struct CatalogInner {
    tools: Vec<ToolEntry>,
    by_name: HashMap<String, ToolEntry>,
    by_server: HashMap<String, Vec<ToolEntry>>,
}

/// Manages the aggregated tool catalog from all upstream servers.
pub struct Catalog {
    inner: RwLock<CatalogInner>,
    cache_path: PathBuf,
}

impl Catalog {
    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            inner: RwLock::new(CatalogInner {
                tools: Vec::new(),
                by_name: HashMap::new(),
                by_server: HashMap::new(),
            }),
            cache_path,
        }
    }

    /// Load from disk cache. Missing/corrupt cache starts empty (no error).
    pub fn load(&self) -> Result<(), McpzipError> {
        match cache::read_cache(&self.cache_path) {
            Ok(entries) => {
                let mut inner = self.inner.write().unwrap();
                set_tools(&mut inner, entries);
                Ok(())
            }
            Err(_) => Ok(()), // Missing or corrupt is not fatal
        }
    }

    /// Refresh from upstream servers. Takes a map of server_name -> tools.
    /// Merges with existing catalog: only replaces tools for servers that
    /// returned results, keeps cached tools for servers not in the map.
    pub fn refresh(
        &self,
        server_tools: HashMap<String, Vec<ToolEntry>>,
    ) -> Result<(), McpzipError> {
        let mut all: Vec<ToolEntry> = Vec::new();

        {
            let inner = self.inner.read().unwrap();
            // Keep tools from servers that didn't respond this time
            for tool in &inner.tools {
                if !server_tools.contains_key(&tool.server_name) {
                    all.push(tool.clone());
                }
            }
        }

        // Add fresh tools from servers that did respond
        for (_server_name, tools) in server_tools {
            for t in tools {
                all.push(t);
            }
        }

        // Sort for deterministic ordering
        all.sort_by(|a, b| a.name.cmp(&b.name));

        {
            let mut inner = self.inner.write().unwrap();
            set_tools(&mut inner, all.clone());
        }

        if !self.cache_path.as_os_str().is_empty() {
            cache::write_cache(&self.cache_path, &all)?;
        }

        Ok(())
    }

    /// Get all tools.
    pub fn all_tools(&self) -> Vec<ToolEntry> {
        let inner = self.inner.read().unwrap();
        inner.tools.clone()
    }

    /// Get a tool by prefixed name.
    pub fn get_tool(&self, prefixed_name: &str) -> Result<ToolEntry, McpzipError> {
        let inner = self.inner.read().unwrap();
        inner
            .by_name
            .get(prefixed_name)
            .cloned()
            .ok_or_else(|| McpzipError::ToolNotFound(prefixed_name.into()))
    }

    /// Get tools for a specific server.
    pub fn server_tools(&self, server_name: &str) -> Vec<ToolEntry> {
        let inner = self.inner.read().unwrap();
        inner
            .by_server
            .get(server_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Total tool count.
    pub fn tool_count(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.tools.len()
    }

    /// Sorted list of server names.
    pub fn server_names(&self) -> Vec<String> {
        let inner = self.inner.read().unwrap();
        let mut names: Vec<String> = inner.by_server.keys().cloned().collect();
        names.sort();
        names
    }
}

fn set_tools(inner: &mut CatalogInner, tools: Vec<ToolEntry>) {
    inner.by_name.clear();
    inner.by_server.clear();
    for t in &tools {
        inner.by_name.insert(t.name.clone(), t.clone());
        inner
            .by_server
            .entry(t.server_name.clone())
            .or_default()
            .push(t.clone());
    }
    inner.tools = tools;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tools() -> Vec<ToolEntry> {
        vec![
            ToolEntry {
                name: "slack__send".into(),
                server_name: "slack".into(),
                original_name: "send".into(),
                description: "Send a message".into(),
                input_schema: json!({"type": "object"}),
                compact_params: "".into(),
            },
            ToolEntry {
                name: "slack__read".into(),
                server_name: "slack".into(),
                original_name: "read".into(),
                description: "Read messages".into(),
                input_schema: json!(null),
                compact_params: "".into(),
            },
            ToolEntry {
                name: "github__create_pr".into(),
                server_name: "github".into(),
                original_name: "create_pr".into(),
                description: "Create PR".into(),
                input_schema: json!(null),
                compact_params: "".into(),
            },
        ]
    }

    #[test]
    fn test_load_from_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("tools.json");

        // Write cache first
        cache::write_cache(&cache_path, &test_tools()).unwrap();

        let cat = Catalog::new(cache_path);
        cat.load().unwrap();

        assert_eq!(cat.tool_count(), 3);
    }

    #[test]
    fn test_load_missing_cache() {
        let cat = Catalog::new(PathBuf::from("/nonexistent/tools.json"));
        cat.load().unwrap(); // Should not error
        assert_eq!(cat.tool_count(), 0);
    }

    #[test]
    fn test_get_tool() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());

        cat.refresh(server_tools).unwrap();

        let tool = cat.get_tool("slack__send").unwrap();
        assert_eq!(tool.description, "Send a message");
    }

    #[test]
    fn test_get_tool_missing() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));
        assert!(cat.get_tool("nonexistent").is_err());
    }

    #[test]
    fn test_all_tools() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());
        server_tools.insert("github".into(), vec![test_tools()[2].clone()]);

        cat.refresh(server_tools).unwrap();

        let all = cat.all_tools();
        assert_eq!(all.len(), 3);
        // Should be sorted
        assert_eq!(all[0].name, "github__create_pr");
        assert_eq!(all[1].name, "slack__read");
        assert_eq!(all[2].name, "slack__send");
    }

    #[test]
    fn test_server_tools() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());
        server_tools.insert("github".into(), vec![test_tools()[2].clone()]);

        cat.refresh(server_tools).unwrap();

        assert_eq!(cat.server_tools("slack").len(), 2);
        assert_eq!(cat.server_tools("github").len(), 1);
        assert_eq!(cat.server_tools("nonexistent").len(), 0);
    }

    #[test]
    fn test_server_names() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());
        server_tools.insert("github".into(), vec![test_tools()[2].clone()]);

        cat.refresh(server_tools).unwrap();

        let names = cat.server_names();
        assert_eq!(names, vec!["github", "slack"]);
    }

    #[test]
    fn test_refresh_saves_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("tools.json");
        let cat = Catalog::new(cache_path.clone());

        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());

        cat.refresh(server_tools).unwrap();

        // Verify cache was written
        let cached = cache::read_cache(&cache_path).unwrap();
        assert_eq!(cached.len(), 2);
    }

    #[test]
    fn test_empty_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));
        assert_eq!(cat.tool_count(), 0);
        assert!(cat.all_tools().is_empty());
        assert!(cat.server_names().is_empty());
    }

    #[test]
    fn test_catalog_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Catalog>();
    }

    #[test]
    fn test_refresh_merges_keeps_missing_servers() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        // Initial refresh with both slack and github
        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());
        server_tools.insert("github".into(), vec![test_tools()[2].clone()]);
        cat.refresh(server_tools).unwrap();
        assert_eq!(cat.tool_count(), 3);

        // Second refresh: only slack responds (github timed out)
        let mut partial = HashMap::new();
        partial.insert("slack".into(), test_tools()[..2].to_vec());
        cat.refresh(partial).unwrap();

        // github tools should still be there from cache
        assert_eq!(cat.tool_count(), 3);
        assert_eq!(cat.server_tools("github").len(), 1);
        assert_eq!(cat.server_tools("slack").len(), 2);
    }

    #[test]
    fn test_refresh_updates_responding_servers() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::new(dir.path().join("tools.json"));

        // Initial: slack has 2 tools
        let mut server_tools = HashMap::new();
        server_tools.insert("slack".into(), test_tools()[..2].to_vec());
        cat.refresh(server_tools).unwrap();
        assert_eq!(cat.server_tools("slack").len(), 2);

        // Refresh: slack now has 1 tool (tool was removed upstream)
        let mut updated = HashMap::new();
        updated.insert("slack".into(), vec![test_tools()[0].clone()]);
        cat.refresh(updated).unwrap();
        assert_eq!(cat.server_tools("slack").len(), 1);
    }
}
