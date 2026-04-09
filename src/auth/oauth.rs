use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::store::{Token, TokenStore};
use crate::error::McpzipError;

/// OAuth client registration info, persisted alongside tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
}

/// Authorization server metadata (subset we care about).
#[derive(Debug, Clone, Deserialize)]
struct AuthServerMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    registration_endpoint: Option<String>,
    #[serde(default)]
    code_challenge_methods_supported: Option<Vec<String>>,
}

/// Protected resource metadata (RFC 9728).
#[derive(Debug, Clone, Deserialize)]
struct ProtectedResourceMetadata {
    #[serde(default)]
    authorization_servers: Option<Vec<String>>,
}

/// OAuth handler that manages the full auth flow for an HTTP upstream.
pub struct OAuthHandler {
    server_url: String,
    store: Arc<TokenStore>,
}

impl OAuthHandler {
    pub fn new(server_url: String, store: Arc<TokenStore>) -> Self {
        Self { server_url, store }
    }

    /// Get a valid token, using cached token or triggering browser auth flow.
    pub async fn get_token(&self) -> Result<Token, McpzipError> {
        self.get_token_with_hint("", "").await
    }

    /// Get a token, with optional hints from WWW-Authenticate header.
    pub async fn get_token_with_hint(
        &self,
        www_authenticate: &str,
        resource_url: &str,
    ) -> Result<Token, McpzipError> {
        // Try cached token first
        if let Some(tok) = self.store.load(&self.server_url)? {
            if !tok.access_token.is_empty() {
                return Ok(tok);
            }
        }

        // Try to find mcp-remote's cached tokens
        if let Some(tok) = self.try_mcp_remote_tokens().await {
            // Save to our store for future use
            self.store.save(&self.server_url, &tok)?;
            return Ok(tok);
        }

        // Full OAuth browser flow
        let resource = if resource_url.is_empty() {
            &self.server_url
        } else {
            resource_url
        };
        self.browser_auth_flow(www_authenticate, resource).await
    }

    /// Try to reuse tokens cached by mcp-remote.
    async fn try_mcp_remote_tokens(&self) -> Option<Token> {
        let home = dirs::home_dir()?;
        let mcp_auth_dir = home.join(".mcp-auth");

        // Try each mcp-remote version dir
        let entries = std::fs::read_dir(&mcp_auth_dir).ok()?;
        for entry in entries.flatten() {
            if !entry.file_type().ok()?.is_dir() {
                continue;
            }
            let dir = entry.path();

            // Look for token files and try each one against our server
            let token_files: Vec<_> = std::fs::read_dir(&dir)
                .ok()?
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with("_tokens.json"))
                .collect();

            for tf in token_files {
                let data = std::fs::read_to_string(tf.path()).ok()?;
                if let Ok(tokens) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(access_token) = tokens.get("access_token").and_then(|v| v.as_str())
                    {
                        // Quick validation: try using this token
                        let client = reqwest::Client::new();
                        let resp = client
                            .post(&self.server_url)
                            .header("Accept", "application/json, text/event-stream")
                            .header("Authorization", format!("Bearer {}", access_token))
                            .header("Content-Type", "application/json")
                            .body(r#"{"jsonrpc":"2.0","id":0,"method":"ping"}"#)
                            .send()
                            .await
                            .ok()?;

                        if resp.status().is_success()
                            || resp.status() == reqwest::StatusCode::ACCEPTED
                        {
                            return Some(Token {
                                access_token: access_token.into(),
                                token_type: tokens
                                    .get("token_type")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.into()),
                                refresh_token: tokens
                                    .get("refresh_token")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.into()),
                                expiry: None,
                            });
                        }
                    }
                }
            }
        }
        None
    }

    /// Full OAuth 2.1 browser flow with PKCE.
    async fn browser_auth_flow(
        &self,
        www_authenticate: &str,
        resource: &str,
    ) -> Result<Token, McpzipError> {
        let client = reqwest::Client::new();

        // Step 1: Discover authorization server
        let auth_server_url = self.discover_auth_server(&client, www_authenticate).await?;

        // Step 2: Get authorization server metadata
        let metadata = self
            .get_auth_server_metadata(&client, &auth_server_url)
            .await?;

        // Verify PKCE support
        if let Some(ref methods) = metadata.code_challenge_methods_supported {
            if !methods.iter().any(|m| m == "S256") {
                return Err(McpzipError::Auth(
                    "authorization server does not support S256 PKCE".into(),
                ));
            }
        }

        // Step 3: Dynamic client registration (if supported and we don't have a client_id)
        let client_info = self.register_client(&client, &metadata).await?;

        // Step 4: Generate PKCE code verifier + challenge
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier);

        // Step 5: Start local callback server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| McpzipError::Auth(format!("binding callback server: {}", e)))?;
        let port = listener
            .local_addr()
            .map_err(|e| McpzipError::Auth(format!("getting port: {}", e)))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", port);

        // Step 6: Build authorization URL and open browser
        let state = generate_code_verifier(); // reuse as random state
        let auth_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&resource={}",
            metadata.authorization_endpoint,
            urlencoding::encode(&client_info.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(&code_challenge),
            urlencoding::encode(&state),
            urlencoding::encode(resource),
        );

        eprintln!("mcpzip: opening browser for OAuth authorization...");
        eprintln!("mcpzip: if browser doesn't open, visit: {}", auth_url);
        let _ = open::that(&auth_url);

        // Step 7: Wait for callback with auth code
        let (code, returned_state) = wait_for_callback(listener).await?;

        if returned_state != state {
            return Err(McpzipError::Auth("OAuth state mismatch".into()));
        }

        // Step 8: Exchange code for token
        let token = self
            .exchange_code(
                &client,
                &metadata.token_endpoint,
                &code,
                &code_verifier,
                &redirect_uri,
                &client_info.client_id,
                resource,
            )
            .await?;

        // Step 9: Persist token
        self.store.save(&self.server_url, &token)?;

        Ok(token)
    }

    /// Discover the authorization server URL from protected resource metadata or WWW-Authenticate.
    async fn discover_auth_server(
        &self,
        client: &reqwest::Client,
        www_authenticate: &str,
    ) -> Result<String, McpzipError> {
        // Try to extract resource_metadata from WWW-Authenticate header
        if let Some(metadata_url) = extract_resource_metadata(www_authenticate) {
            if let Ok(prm) = client.get(&metadata_url).send().await {
                if let Ok(meta) = prm.json::<ProtectedResourceMetadata>().await {
                    if let Some(servers) = meta.authorization_servers {
                        if let Some(first) = servers.first() {
                            return Ok(first.clone());
                        }
                    }
                }
            }
        }

        // Fallback: well-known URI on the server
        let url = url::Url::parse(&self.server_url)
            .map_err(|e| McpzipError::Auth(format!("invalid server URL: {}", e)))?;

        let base = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));

        // Try with path
        let path = url.path().trim_end_matches('/');
        if !path.is_empty() && path != "/" {
            let well_known = format!("{}/.well-known/oauth-protected-resource{}", base, path);
            if let Ok(resp) = client.get(&well_known).send().await {
                if let Ok(meta) = resp.json::<ProtectedResourceMetadata>().await {
                    if let Some(servers) = meta.authorization_servers {
                        if let Some(first) = servers.first() {
                            return Ok(first.clone());
                        }
                    }
                }
            }
        }

        // Try at root
        let well_known = format!("{}/.well-known/oauth-protected-resource", base);
        if let Ok(resp) = client.get(&well_known).send().await {
            if let Ok(meta) = resp.json::<ProtectedResourceMetadata>().await {
                if let Some(servers) = meta.authorization_servers {
                    if let Some(first) = servers.first() {
                        return Ok(first.clone());
                    }
                }
            }
        }

        // Last resort: assume auth server is same origin
        Ok(base)
    }

    /// Fetch authorization server metadata.
    async fn get_auth_server_metadata(
        &self,
        client: &reqwest::Client,
        auth_server: &str,
    ) -> Result<AuthServerMetadata, McpzipError> {
        let url = url::Url::parse(auth_server)
            .map_err(|e| McpzipError::Auth(format!("invalid auth server URL: {}", e)))?;

        let base = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));
        let path = url.path().trim_end_matches('/');

        // Try OAuth 2.0 metadata with path insertion
        let endpoints = if !path.is_empty() && path != "/" {
            vec![
                format!("{}/.well-known/oauth-authorization-server{}", base, path),
                format!("{}/.well-known/openid-configuration{}", base, path),
                format!("{}{}/.well-known/openid-configuration", base, path),
            ]
        } else {
            vec![
                format!("{}/.well-known/oauth-authorization-server", base),
                format!("{}/.well-known/openid-configuration", base),
            ]
        };

        for endpoint in &endpoints {
            if let Ok(resp) = client.get(endpoint).send().await {
                if resp.status().is_success() {
                    if let Ok(meta) = resp.json::<AuthServerMetadata>().await {
                        return Ok(meta);
                    }
                }
            }
        }

        Err(McpzipError::Auth(format!(
            "could not discover authorization server metadata for {}",
            auth_server
        )))
    }

    /// Register as a dynamic client (RFC 7591).
    async fn register_client(
        &self,
        client: &reqwest::Client,
        metadata: &AuthServerMetadata,
    ) -> Result<ClientInfo, McpzipError> {
        let reg_endpoint = metadata.registration_endpoint.as_ref().ok_or_else(|| {
            McpzipError::Auth("no registration_endpoint in auth server metadata".into())
        })?;

        let reg_req = serde_json::json!({
            "client_name": "mcpzip",
            "redirect_uris": ["http://127.0.0.1/oauth/callback"],
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"]
        });

        let resp = client
            .post(reg_endpoint)
            .json(&reg_req)
            .send()
            .await
            .map_err(|e| McpzipError::Auth(format!("client registration failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(McpzipError::Auth(format!(
                "client registration returned {}: {}",
                status, body
            )));
        }

        let info: ClientInfo = resp
            .json()
            .await
            .map_err(|e| McpzipError::Auth(format!("parsing registration response: {}", e)))?;

        Ok(info)
    }

    /// Exchange authorization code for tokens.
    #[allow(clippy::too_many_arguments)]
    async fn exchange_code(
        &self,
        client: &reqwest::Client,
        token_endpoint: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        client_id: &str,
        resource: &str,
    ) -> Result<Token, McpzipError> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", code_verifier),
            ("resource", resource),
        ];

        let resp = client
            .post(token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| McpzipError::Auth(format!("token exchange failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(McpzipError::Auth(format!(
                "token exchange returned {}: {}",
                status, body
            )));
        }

        let token_resp: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| McpzipError::Auth(format!("parsing token response: {}", e)))?;

        Ok(Token {
            access_token: token_resp
                .get("access_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpzipError::Auth("no access_token in response".into()))?
                .into(),
            token_type: token_resp
                .get("token_type")
                .and_then(|v| v.as_str())
                .map(|s| s.into()),
            refresh_token: token_resp
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .map(|s| s.into()),
            expiry: None,
        })
    }

    /// Get an authorization header value for HTTP requests.
    pub async fn authorization_header(&self) -> Result<String, McpzipError> {
        let tok = self.get_token().await?;
        let token_type = tok.token_type.as_deref().unwrap_or("Bearer");
        // Capitalize first letter for consistency
        let token_type = if token_type.eq_ignore_ascii_case("bearer") {
            "Bearer"
        } else {
            token_type
        };
        Ok(format!("{} {}", token_type, tok.access_token))
    }
}

/// Generate a random PKCE code verifier (43-128 chars, base64url).
fn generate_code_verifier() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    base64_url_encode(&bytes)
}

/// Generate S256 code challenge from verifier.
fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    base64_url_encode(&hash)
}

/// Base64url encoding without padding.
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Extract resource_metadata URL from WWW-Authenticate header.
fn extract_resource_metadata(header: &str) -> Option<String> {
    // Look for resource_metadata="..." in the header
    let key = "resource_metadata=\"";
    let start = header.find(key)? + key.len();
    let rest = &header[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Wait for the OAuth callback on the local server.
/// Returns (code, state).
async fn wait_for_callback(
    listener: tokio::net::TcpListener,
) -> Result<(String, String), McpzipError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut stream, _) =
        tokio::time::timeout(std::time::Duration::from_secs(120), listener.accept())
            .await
            .map_err(|_| McpzipError::Auth("OAuth callback timed out (120s)".into()))?
            .map_err(|e| McpzipError::Auth(format!("accepting callback: {}", e)))?;

    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| McpzipError::Auth(format!("reading callback: {}", e)))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the GET request for code and state params
    let path = request
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("");

    let query = path.split('?').nth(1).unwrap_or("");
    let mut code = String::new();
    let mut state = String::new();

    for param in query.split('&') {
        if let Some((k, v)) = param.split_once('=') {
            match k {
                "code" => code = urlencoding::decode(v).unwrap_or_default().into_owned(),
                "state" => state = urlencoding::decode(v).unwrap_or_default().into_owned(),
                _ => {}
            }
        }
    }

    // Send success response to browser
    let html = "<html><body><h1>Authorization successful!</h1><p>You can close this tab.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes()).await;

    if code.is_empty() {
        // Check for error
        let error = query
            .split('&')
            .find_map(|p| p.strip_prefix("error="))
            .unwrap_or("unknown");
        return Err(McpzipError::Auth(format!(
            "OAuth callback error: {}",
            error
        )));
    }

    Ok((code, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cached_token_returned() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(TokenStore::new(dir.path().join("auth")));

        let tok = Token {
            access_token: "cached-token".into(),
            token_type: Some("Bearer".into()),
            refresh_token: None,
            expiry: None,
        };
        store.save("https://example.com", &tok).unwrap();

        let handler = OAuthHandler::new("https://example.com".into(), store);
        let result = handler.get_token().await.unwrap();
        assert_eq!(result.access_token, "cached-token");
    }

    #[tokio::test]
    async fn test_authorization_header() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(TokenStore::new(dir.path().join("auth")));

        let tok = Token {
            access_token: "my-token".into(),
            token_type: Some("Bearer".into()),
            refresh_token: None,
            expiry: None,
        };
        store.save("https://example.com", &tok).unwrap();

        let handler = OAuthHandler::new("https://example.com".into(), store);
        let header = handler.authorization_header().await.unwrap();
        assert_eq!(header, "Bearer my-token");
    }

    #[test]
    fn test_oauth_handler_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OAuthHandler>();
    }

    #[test]
    fn test_generate_code_verifier() {
        let v = generate_code_verifier();
        assert!(!v.is_empty());
        assert!(v.len() >= 32);
    }

    #[test]
    fn test_generate_code_challenge() {
        let verifier = "test_verifier";
        let challenge = generate_code_challenge(verifier);
        assert!(!challenge.is_empty());
        // S256 should produce consistent output
        let challenge2 = generate_code_challenge(verifier);
        assert_eq!(challenge, challenge2);
    }

    #[test]
    fn test_extract_resource_metadata() {
        let header = r#"Bearer resource_metadata="https://example.com/.well-known/oauth-protected-resource", scope="files:read""#;
        assert_eq!(
            extract_resource_metadata(header),
            Some("https://example.com/.well-known/oauth-protected-resource".into())
        );
    }

    #[test]
    fn test_extract_resource_metadata_missing() {
        assert_eq!(extract_resource_metadata("Bearer"), None);
    }
}
