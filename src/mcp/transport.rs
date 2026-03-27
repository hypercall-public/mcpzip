use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::McpzipError;

/// Transport for sending/receiving JSON-RPC messages.
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn send(&self, msg: Value) -> Result<(), McpzipError>;
    async fn receive(&self) -> Result<Value, McpzipError>;
}

/// NDJSON transport over a pair of async reader/writer streams.
pub struct NdjsonTransport {
    reader: tokio::sync::Mutex<BufReader<Box<dyn tokio::io::AsyncRead + Send + Unpin>>>,
    writer: tokio::sync::Mutex<Box<dyn tokio::io::AsyncWrite + Send + Unpin>>,
}

impl NdjsonTransport {
    pub fn new(
        reader: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        writer: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
    ) -> Self {
        Self {
            reader: tokio::sync::Mutex::new(BufReader::new(reader)),
            writer: tokio::sync::Mutex::new(writer),
        }
    }

    /// Create a transport using stdin/stdout.
    pub fn stdio() -> Self {
        Self::new(
            Box::new(tokio::io::stdin()),
            Box::new(tokio::io::stdout()),
        )
    }
}

#[async_trait]
impl McpTransport for NdjsonTransport {
    async fn send(&self, msg: Value) -> Result<(), McpzipError> {
        let line = serde_json::to_string(&msg)?;
        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        Ok(())
    }

    async fn receive(&self) -> Result<Value, McpzipError> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(McpzipError::Transport("connection closed".into()));
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Ok(serde_json::from_str(trimmed)?);
            }
        }
    }
}

/// Create a pair of in-memory transports for testing.
pub fn memory_transport_pair() -> (NdjsonTransport, NdjsonTransport) {
    let (client_read, server_write) = tokio::io::duplex(8192);
    let (server_read, client_write) = tokio::io::duplex(8192);

    let client = NdjsonTransport::new(Box::new(client_read), Box::new(client_write));
    let server = NdjsonTransport::new(Box::new(server_read), Box::new(server_write));

    (client, server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_memory_transport_roundtrip() {
        let (a, b) = memory_transport_pair();
        let msg = json!({"jsonrpc": "2.0", "method": "test"});
        a.send(msg.clone()).await.unwrap();
        let received = b.receive().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_memory_transport_bidirectional() {
        let (a, b) = memory_transport_pair();

        a.send(json!({"id": 1})).await.unwrap();
        let r1 = b.receive().await.unwrap();
        assert_eq!(r1["id"], 1);

        b.send(json!({"id": 2})).await.unwrap();
        let r2 = a.receive().await.unwrap();
        assert_eq!(r2["id"], 2);
    }

    #[tokio::test]
    async fn test_ndjson_framing() {
        let (a, b) = memory_transport_pair();
        // Send multiple messages
        a.send(json!({"n": 1})).await.unwrap();
        a.send(json!({"n": 2})).await.unwrap();

        let r1 = b.receive().await.unwrap();
        let r2 = b.receive().await.unwrap();
        assert_eq!(r1["n"], 1);
        assert_eq!(r2["n"], 2);
    }

    #[tokio::test]
    async fn test_transport_is_object_safe() {
        fn accepts_trait_object(_t: &dyn McpTransport) {}
        let (a, _b) = memory_transport_pair();
        accepts_trait_object(&a);
    }
}
