---
sidebar_position: 6.5
title: OAuth
description: OAuth 2.1 authentication for remote MCP servers
---

# OAuth

mcpzip automatically handles OAuth 2.1 authentication for remote HTTP MCP servers. No manual token management required.

## When OAuth is Used

OAuth is triggered when **all** of these conditions are met:

1. The server has `type: "http"`
2. No custom `headers` are configured
3. The server responds with `401 Unauthorized` and a `WWW-Authenticate` header

If you set `headers` (e.g., for API key auth), OAuth is completely skipped.

:::info
Setting custom headers is the escape hatch. If a server has a simpler auth mechanism (API key, pre-shared token), just put it in `headers` and OAuth won't activate.
:::

<details>
<summary><strong>What is OAuth 2.1?</strong></summary>

**OAuth 2.1** is an authorization framework that lets applications access resources on behalf of a user without handling their password directly.

Key concepts:
- **Authorization Server** -- issues access tokens after user grants permission
- **Resource Server** -- the MCP server that requires authentication
- **Client** -- mcpzip, acting on behalf of the user
- **Access Token** -- short-lived credential for API access
- **Refresh Token** -- long-lived credential used to obtain new access tokens

OAuth 2.1 is an evolution of OAuth 2.0 that mandates PKCE, prohibits implicit grants, and tightens security requirements.

</details>

<details>
<summary><strong>What is PKCE?</strong></summary>

**PKCE** (Proof Key for Code Exchange, pronounced "pixy") prevents authorization code interception attacks.

How it works:
1. mcpzip generates a random **code verifier** (a long random string)
2. It creates a **code challenge** by hashing the verifier with SHA-256
3. The code challenge is sent in the authorization request
4. When exchanging the auth code for tokens, mcpzip sends the original code verifier
5. The authorization server verifies that the verifier matches the challenge

This ensures that even if an attacker intercepts the authorization code, they can't exchange it for tokens without the original code verifier.

</details>

## The OAuth Flow

```mermaid
sequenceDiagram
    participant M as mcpzip
    participant B as Browser
    participant AS as Auth Server
    participant RS as MCP Server

    M->>RS: POST /mcp (initialize)
    RS-->>M: 401 Unauthorized<br/>WWW-Authenticate: Bearer resource_metadata="..."

    M->>RS: GET /.well-known/oauth-authorization-server
    RS-->>M: {authorization_endpoint, token_endpoint, ...}

    Note over M: Generate PKCE<br/>code_verifier + code_challenge

    M->>B: Open browser to authorization_endpoint
    B->>AS: User logs in & grants access
    AS->>M: Redirect to localhost callback<br/>with authorization code

    M->>AS: POST token_endpoint<br/>{code, code_verifier, redirect_uri}
    AS-->>M: {access_token, refresh_token, expires_in}

    Note over M: Store tokens to disk

    M->>RS: POST /mcp (initialize)<br/>Authorization: Bearer {access_token}
    RS-->>M: 200 OK - Connected!
```

## Token Storage

Tokens are persisted to disk at:

```
~/.config/compressed-mcp-proxy/auth/{hash}.json
```

The `{hash}` is derived from the server URL, so each server gets its own token file. Token files contain:

| Field | Description |
|-------|-------------|
| `access_token` | Current access token |
| `refresh_token` | Token used to obtain new access tokens |
| `expires_at` | Expiration timestamp |
| `token_type` | Usually `"Bearer"` |

:::warning File Permissions
Token files contain sensitive credentials. Ensure the auth directory has restrictive permissions:

```bash
chmod 700 ~/.config/compressed-mcp-proxy/auth/
chmod 600 ~/.config/compressed-mcp-proxy/auth/*.json
```
:::

## Token Reuse

### Across mcpzip Restarts

Tokens persist on disk, so restarting mcpzip doesn't require re-authentication. On startup:

1. mcpzip checks for a stored token
2. If valid (not expired), uses it directly
3. If expired but has a refresh token, refreshes automatically
4. If no valid token exists, starts the OAuth flow

### From mcp-remote

mcpzip checks for tokens previously saved by `mcp-remote` (the reference MCP OAuth client). If you've already authenticated with a server using mcp-remote, mcpzip will reuse those tokens.

Token locations checked:
1. `~/.config/compressed-mcp-proxy/auth/{hash}.json` (mcpzip's own tokens)
2. mcp-remote's token storage (if available)

```mermaid
flowchart TD
    START([Connection attempt]) --> CHECK[Check stored token]
    CHECK --> VALID{Token valid?}
    VALID -->|Yes| USE[Use token]
    VALID -->|No| REFRESH{Refresh token<br/>available?}
    REFRESH -->|Yes| DO_REFRESH[Refresh token]
    DO_REFRESH --> REFRESHED{Refresh<br/>succeeded?}
    REFRESHED -->|Yes| STORE[Store new token]
    REFRESHED -->|No| OAUTH[Start OAuth flow]
    REFRESH -->|No| MCP_REMOTE{Check mcp-remote<br/>tokens?}
    MCP_REMOTE -->|Found| USE
    MCP_REMOTE -->|Not found| OAUTH
    STORE --> USE
    OAUTH --> STORE

    style START fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style USE fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style OAUTH fill:#1a1a2e,stroke:#60A5FA,color:#60A5FA
```

## Configuration

OAuth is automatic for HTTP servers without custom headers:

```json
{
  "mcpServers": {
    "todoist": {
      "type": "http",
      "url": "https://todoist.com/mcp"
    }
  }
}
```

To **skip OAuth** and use static credentials instead:

```json
{
  "mcpServers": {
    "analytics": {
      "type": "http",
      "url": "https://analytics.example.com/mcp",
      "headers": {
        "Authorization": "Bearer static-api-key"
      }
    }
  }
}
```

## Troubleshooting

<details>
<summary><strong>Browser doesn't open</strong></summary>

If the browser doesn't open automatically during the OAuth flow:

1. Check the terminal output -- mcpzip prints the authorization URL
2. Copy the URL and open it manually in your browser
3. Complete the authorization flow in the browser

On headless systems (servers, containers), OAuth requires a browser and won't work. Use API key auth via `headers` instead.

</details>

<details>
<summary><strong>Token expired, getting 401 errors</strong></summary>

If your token has expired and refresh fails:

1. Delete the cached token:
```bash
rm ~/.config/compressed-mcp-proxy/auth/*.json
```

2. Restart mcpzip -- it will trigger a fresh OAuth flow

</details>

<details>
<summary><strong>Callback server fails to start</strong></summary>

mcpzip runs a temporary local HTTP server to receive the OAuth callback. If port binding fails:

1. Check if another process is using the port
2. Check your firewall settings
3. Try again -- mcpzip uses a dynamic port, so conflicts are rare

</details>

<details>
<summary><strong>"Invalid redirect_uri" error</strong></summary>

This usually means the OAuth application's registered redirect URIs don't match what mcpzip is using. This is a server-side configuration issue -- contact the MCP server's provider.

</details>

<details>
<summary><strong>How do I authenticate with multiple servers?</strong></summary>

Each server maintains its own token independently. If you have 3 OAuth-authenticated servers, you'll go through the browser flow 3 times on first use. After that, tokens are reused from disk.

</details>
