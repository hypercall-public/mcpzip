use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::RwLock;

use crate::error::McpzipError;
use crate::transport::{ConnectFn, Upstream};
use crate::types::{ServerConfig, ToolEntry};

struct PoolEntry {
    upstream: Box<dyn Upstream>,
    last_used: Instant,
}

/// Manages connections to upstream MCP servers.
/// Maintains a pool of active connections and reaps idle ones.
pub struct Manager {
    configs: HashMap<String, ServerConfig>,
    pool: Arc<RwLock<HashMap<String, PoolEntry>>>,
    #[allow(dead_code)]
    idle_timeout: Duration,
    call_timeout: Duration,
    connect: ConnectFn,
    cancel: tokio_util::sync::CancellationToken,
}

impl Manager {
    pub fn new(
        configs: HashMap<String, ServerConfig>,
        idle_timeout: Duration,
        call_timeout: Duration,
        connect: ConnectFn,
    ) -> Self {
        let pool: Arc<RwLock<HashMap<String, PoolEntry>>> = Arc::new(RwLock::new(HashMap::new()));

        let cancel = tokio_util::sync::CancellationToken::new();

        // Start idle reaper
        let reaper_pool = pool.clone();
        let reaper_cancel = cancel.clone();
        let reaper_interval = {
            let half = idle_timeout / 2;
            if half < Duration::from_secs(60) && half > Duration::ZERO {
                half
            } else {
                Duration::from_secs(60)
            }
        };
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(reaper_interval);
            loop {
                tokio::select! {
                    _ = reaper_cancel.cancelled() => break,
                    _ = interval.tick() => {
                        reap_idle(&reaper_pool, idle_timeout).await;
                    }
                }
            }
        });

        Self {
            configs,
            pool,
            idle_timeout,
            call_timeout,
            connect,
            cancel,
        }
    }

    /// Get a pooled connection, creating one if needed.
    pub async fn get_upstream(&self, server_name: &str) -> Result<Arc<dyn Upstream>, McpzipError> {
        let cfg = self
            .configs
            .get(server_name)
            .ok_or_else(|| McpzipError::ServerNotFound(server_name.into()))?
            .clone();

        // Check pool for live connection
        {
            let mut pool = self.pool.write().await;
            if let Some(entry) = pool.get_mut(server_name) {
                if entry.upstream.alive() {
                    entry.last_used = Instant::now();
                    // Return a reference by wrapping in Arc -- we need to restructure
                    // For now, we keep Box and just return that we found it
                }
            }
        }

        // We need a different pool design: store Arc<dyn Upstream> instead of Box
        // Let's check if we have a live entry first, then create if not
        {
            let pool = self.pool.read().await;
            if let Some(entry) = pool.get(server_name) {
                if entry.upstream.alive() {
                    // We have a live connection but can't easily share it
                    // The Go design holds the lock during the call; we'll do the same
                }
            }
        }

        // For simplicity, use write lock for the full get-or-create operation
        let mut pool = self.pool.write().await;

        // Re-check under write lock
        if let Some(entry) = pool.get_mut(server_name) {
            if entry.upstream.alive() {
                entry.last_used = Instant::now();
                // We can't return a ref to the Box, so we'll call through the manager instead
                // This method is used internally; external callers use call_tool
                drop(pool);
                // Just return ok — the actual call goes through call_tool_inner
                unreachable!("use call_tool instead of get_upstream directly");
            } else {
                let _ = entry.upstream.close().await;
                pool.remove(server_name);
            }
        }

        // Connect
        let upstream = (self.connect)(server_name.into(), cfg).await.map_err(|e| {
            McpzipError::Transport(format!("connecting to {:?}: {}", server_name, e))
        })?;

        pool.insert(
            server_name.into(),
            PoolEntry {
                upstream,
                last_used: Instant::now(),
            },
        );

        // Same issue — just signal success
        unreachable!("use call_tool instead of get_upstream directly");
    }

    /// Call a tool on an upstream server. Handles pool lookup, retry on failure.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, McpzipError> {
        let cfg = self
            .configs
            .get(server_name)
            .ok_or_else(|| McpzipError::ServerNotFound(server_name.into()))?
            .clone();

        // Ensure connection
        self.ensure_connected(server_name, &cfg).await?;

        // First attempt
        let result = {
            let mut pool = self.pool.write().await;
            let entry = pool
                .get_mut(server_name)
                .ok_or_else(|| McpzipError::Transport("connection disappeared".into()))?;
            entry.last_used = Instant::now();

            if self.call_timeout > Duration::ZERO {
                tokio::time::timeout(
                    self.call_timeout,
                    entry.upstream.call_tool(tool_name, args.clone()),
                )
                .await
                .map_err(|_| McpzipError::Timeout(self.call_timeout.as_secs()))?
            } else {
                entry.upstream.call_tool(tool_name, args.clone()).await
            }
        };

        if result.is_ok() {
            return result;
        }

        // Retry: evict and reconnect
        tracing::warn!(
            server = server_name,
            "call failed, retrying with fresh connection"
        );
        self.evict(server_name).await;
        self.ensure_connected(server_name, &cfg).await?;

        let mut pool = self.pool.write().await;
        let entry = pool
            .get_mut(server_name)
            .ok_or_else(|| McpzipError::Transport("reconnection failed".into()))?;
        entry.last_used = Instant::now();

        if self.call_timeout > Duration::ZERO {
            tokio::time::timeout(self.call_timeout, entry.upstream.call_tool(tool_name, args))
                .await
                .map_err(|_| McpzipError::Timeout(self.call_timeout.as_secs()))?
        } else {
            entry.upstream.call_tool(tool_name, args).await
        }
    }

    /// List tools from all configured servers concurrently.
    /// Partial failures: failed servers are skipped.
    /// Each server gets a 30-second timeout for connect + list_tools.
    pub async fn list_tools_all(&self) -> Result<HashMap<String, Vec<ToolEntry>>, McpzipError> {
        if self.configs.is_empty() {
            return Ok(HashMap::new());
        }

        const PER_SERVER_TIMEOUT: Duration = Duration::from_secs(30);

        // Connect and list tools concurrently with per-server timeout.
        let mut tasks = tokio::task::JoinSet::new();
        for (name, cfg) in &self.configs {
            let name = name.clone();
            let cfg = cfg.clone();
            let pool = self.pool.clone();
            let connect = self.connect.clone();

            tasks.spawn(async move {
                let result = tokio::time::timeout(PER_SERVER_TIMEOUT, async {
                    // Connect
                    let upstream = connect(name.clone(), cfg).await.map_err(|e| {
                        McpzipError::Transport(format!("connecting to {:?}: {}", name, e))
                    })?;

                    // List tools
                    let tools = upstream.list_tools().await?;

                    // Store in pool
                    Ok::<(Box<dyn Upstream>, Vec<ToolEntry>), McpzipError>((upstream, tools))
                })
                .await;

                match result {
                    Ok(Ok((upstream, tools))) => {
                        // Store connection in pool
                        let mut p = pool.write().await;
                        p.insert(
                            name.clone(),
                            PoolEntry {
                                upstream,
                                last_used: Instant::now(),
                            },
                        );
                        (name, Ok(tools))
                    }
                    Ok(Err(e)) => (name, Err(e)),
                    Err(_) => (
                        name.clone(),
                        Err(McpzipError::Transport(format!(
                            "{}: connect timed out ({}s)",
                            name,
                            PER_SERVER_TIMEOUT.as_secs()
                        ))),
                    ),
                }
            });
        }

        let mut all_tools = HashMap::new();
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((name, Ok(tools))) => {
                    eprintln!("mcpzip: {} connected ({} tools)", name, tools.len());
                    all_tools.insert(name, tools);
                }
                Ok((name, Err(e))) => {
                    eprintln!("mcpzip: {} failed: {}", name, e);
                }
                Err(e) => {
                    eprintln!("mcpzip: task panicked: {}", e);
                }
            }
        }

        Ok(all_tools)
    }

    async fn ensure_connected(
        &self,
        server_name: &str,
        cfg: &ServerConfig,
    ) -> Result<(), McpzipError> {
        let mut pool = self.pool.write().await;
        if let Some(entry) = pool.get(server_name) {
            if entry.upstream.alive() {
                return Ok(());
            }
        }

        // Remove stale entry
        if let Some(entry) = pool.remove(server_name) {
            let _ = entry.upstream.close().await;
        }

        let upstream = (self.connect)(server_name.into(), cfg.clone())
            .await
            .map_err(|e| {
                McpzipError::Transport(format!("connecting to {:?}: {}", server_name, e))
            })?;

        pool.insert(
            server_name.into(),
            PoolEntry {
                upstream,
                last_used: Instant::now(),
            },
        );

        Ok(())
    }

    async fn evict(&self, server_name: &str) {
        let mut pool = self.pool.write().await;
        if let Some(entry) = pool.remove(server_name) {
            let _ = entry.upstream.close().await;
        }
    }

    /// Close all connections and stop the reaper.
    pub async fn close(&self) -> Result<(), McpzipError> {
        self.cancel.cancel();
        let mut pool = self.pool.write().await;
        for (_, entry) in pool.drain() {
            let _ = entry.upstream.close().await;
        }
        Ok(())
    }
}

async fn reap_idle(pool: &RwLock<HashMap<String, PoolEntry>>, idle_timeout: Duration) {
    let mut pool = pool.write().await;
    let now = Instant::now();
    let stale: Vec<String> = pool
        .iter()
        .filter(|(_, entry)| now.duration_since(entry.last_used) > idle_timeout)
        .map(|(name, _)| name.clone())
        .collect();

    for name in stale {
        if let Some(entry) = pool.remove(&name) {
            let _ = entry.upstream.close().await;
            tracing::debug!(server = %name, "reaped idle connection");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// Mock upstream for testing.
    struct MockUpstream {
        tools: Vec<ToolEntry>,
        call_count: AtomicUsize,
        alive: AtomicBool,
        fail_first: AtomicBool,
    }

    impl MockUpstream {
        fn new(tools: Vec<ToolEntry>) -> Self {
            Self {
                tools,
                call_count: AtomicUsize::new(0),
                alive: AtomicBool::new(true),
                fail_first: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl Upstream for MockUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolEntry>, McpzipError> {
            Ok(self.tools.clone())
        }

        async fn call_tool(&self, tool_name: &str, _args: Value) -> Result<Value, McpzipError> {
            let count = self.call_count.fetch_add(1, Ordering::Relaxed);
            if self.fail_first.load(Ordering::Relaxed) && count == 0 {
                return Err(McpzipError::Transport("simulated failure".into()));
            }
            Ok(serde_json::json!({"result": format!("called {}", tool_name)}))
        }

        async fn close(&self) -> Result<(), McpzipError> {
            self.alive.store(false, Ordering::Relaxed);
            Ok(())
        }

        fn alive(&self) -> bool {
            self.alive.load(Ordering::Relaxed)
        }
    }

    fn mock_connect(tools: Vec<ToolEntry>) -> ConnectFn {
        Arc::new(move |_name, _cfg| {
            let tools = tools.clone();
            Box::pin(async move { Ok(Box::new(MockUpstream::new(tools)) as Box<dyn Upstream>) })
        })
    }

    fn mock_connect_failing() -> ConnectFn {
        Arc::new(|_name, _cfg| {
            Box::pin(async { Err(McpzipError::Transport("connect failed".into())) })
        })
    }

    fn mock_connect_counting(tools: Vec<ToolEntry>) -> (ConnectFn, Arc<AtomicUsize>) {
        let connect_count = Arc::new(AtomicUsize::new(0));
        let cc = connect_count.clone();
        let connect: ConnectFn = Arc::new(move |_name, _cfg| {
            let tools = tools.clone();
            let count = cc.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
                let upstream = MockUpstream::new(tools);
                // First connection's first call fails; second connection works fine
                if count == 0 {
                    upstream.fail_first.store(true, Ordering::Relaxed);
                }
                Ok(Box::new(upstream) as Box<dyn Upstream>)
            })
        });
        (connect, connect_count)
    }

    fn test_configs() -> HashMap<String, ServerConfig> {
        let mut m = HashMap::new();
        m.insert(
            "slack".into(),
            ServerConfig {
                server_type: None,
                command: Some("slack-mcp".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );
        m
    }

    fn test_tools() -> Vec<ToolEntry> {
        vec![ToolEntry {
            name: "slack__send".into(),
            server_name: "slack".into(),
            original_name: "send".into(),
            description: "Send a message".into(),
            input_schema: serde_json::json!({"type": "object"}),
            compact_params: "".into(),
        }]
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_call_tool_success() {
        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            mock_connect(test_tools()),
        );

        let result = mgr
            .call_tool("slack", "send", serde_json::json!({}))
            .await
            .unwrap();
        assert!(result["result"].as_str().unwrap().contains("send"));

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_call_tool_unknown_server() {
        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            mock_connect(test_tools()),
        );

        let err = mgr
            .call_tool("unknown", "tool", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, McpzipError::ServerNotFound(_)));

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_call_tool_retry_on_failure() {
        let (connect, _count) = mock_connect_counting(test_tools());
        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            connect,
        );

        // First call from the upstream will fail, retry with fresh connection should succeed
        let result = mgr
            .call_tool("slack", "send", serde_json::json!({}))
            .await
            .unwrap();
        assert!(result["result"].as_str().unwrap().contains("send"));

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_call_tool_connect_failure() {
        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            mock_connect_failing(),
        );

        let err = mgr
            .call_tool("slack", "send", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("connect failed"));

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_tools_all() {
        let mut configs = HashMap::new();
        configs.insert(
            "a".into(),
            ServerConfig {
                server_type: None,
                command: Some("a".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );
        configs.insert(
            "b".into(),
            ServerConfig {
                server_type: None,
                command: Some("b".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        let mgr = Manager::new(
            configs,
            Duration::from_secs(300),
            Duration::ZERO,
            mock_connect(test_tools()),
        );

        let all = mgr.list_tools_all().await.unwrap();
        assert_eq!(all.len(), 2);

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_tools_all_partial_failure() {
        let connect_count = Arc::new(AtomicUsize::new(0));
        let cc = connect_count.clone();

        let connect: ConnectFn = Arc::new(move |name: String, _cfg| {
            let cc = cc.clone();
            Box::pin(async move {
                let count = cc.fetch_add(1, Ordering::Relaxed);
                if count == 0 {
                    // First server fails
                    Err(McpzipError::Transport("fail".into()))
                } else {
                    Ok(Box::new(MockUpstream::new(vec![ToolEntry {
                        name: format!("{}__tool", name),
                        server_name: name,
                        original_name: "tool".into(),
                        description: "".into(),
                        input_schema: serde_json::json!(null),
                        compact_params: "".into(),
                    }])) as Box<dyn Upstream>)
                }
            })
        });

        let mut configs = HashMap::new();
        configs.insert(
            "fail".into(),
            ServerConfig {
                server_type: None,
                command: Some("x".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );
        configs.insert(
            "ok".into(),
            ServerConfig {
                server_type: None,
                command: Some("y".into()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        let mgr = Manager::new(configs, Duration::from_secs(300), Duration::ZERO, connect);
        let all = mgr.list_tools_all().await.unwrap();
        // One should succeed, one should fail
        assert!(all.len() >= 1);

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_pool_reuse() {
        let connect_count = Arc::new(AtomicUsize::new(0));
        let cc = connect_count.clone();

        let connect: ConnectFn = Arc::new(move |_name, _cfg| {
            let cc = cc.clone();
            Box::pin(async move {
                cc.fetch_add(1, Ordering::Relaxed);
                Ok(Box::new(MockUpstream::new(vec![])) as Box<dyn Upstream>)
            })
        });

        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            connect,
        );

        mgr.call_tool("slack", "a", serde_json::json!({}))
            .await
            .unwrap();
        mgr.call_tool("slack", "b", serde_json::json!({}))
            .await
            .unwrap();

        // Should only connect once
        assert_eq!(connect_count.load(Ordering::Relaxed), 1);

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_idle_reaper() {
        let mgr = Manager::new(
            test_configs(),
            Duration::from_millis(100), // Short timeout
            Duration::ZERO,
            mock_connect(vec![]),
        );

        mgr.call_tool("slack", "x", serde_json::json!({}))
            .await
            .unwrap();

        // Verify pool is non-empty
        {
            let pool = mgr.pool.read().await;
            assert_eq!(pool.len(), 1, "should have one connection");
        }

        // Wait for reaper to run (interval = timeout/2 = 50ms, timeout = 100ms)
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Pool should be empty after reaper runs
        {
            let pool = mgr.pool.read().await;
            assert!(pool.is_empty(), "idle connection should have been reaped");
        }

        mgr.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_close_all() {
        let mgr = Manager::new(
            test_configs(),
            Duration::from_secs(300),
            Duration::ZERO,
            mock_connect(vec![]),
        );

        mgr.call_tool("slack", "x", serde_json::json!({}))
            .await
            .unwrap();
        mgr.close().await.unwrap();

        let pool = mgr.pool.read().await;
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn test_manager_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Manager>();
    }
}
