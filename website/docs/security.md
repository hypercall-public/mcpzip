---
sidebar_position: 8.5
title: Security
description: How mcpzip handles tokens, credentials, and sensitive data
---

# Security

mcpzip handles OAuth tokens, API keys, and MCP server credentials. Here's how it keeps them safe.

## Credential Storage

### Config File

Your main config lives at `~/.config/compressed-mcp-proxy/config.json`. This file may contain:
- API keys in `env` blocks (e.g., `SLACK_TOKEN`)
- API keys in `headers` blocks (e.g., `Authorization: Bearer ...`)
- Gemini API key

:::danger Protect Your Config File
Set restrictive file permissions on your config:

```bash
chmod 600 ~/.config/compressed-mcp-proxy/config.json
```

This ensures only your user can read the file. Never commit this file to version control.
:::

### OAuth Tokens

OAuth tokens are stored at:

```
~/.config/compressed-mcp-proxy/auth/{hash}.json
```

Each file contains an access token, refresh token, and expiration timestamp. The filename hash is derived from the server URL.

```bash
# Set restrictive permissions on the auth directory
chmod 700 ~/.config/compressed-mcp-proxy/auth/
chmod 600 ~/.config/compressed-mcp-proxy/auth/*.json
```

### Tool Cache

The tool catalog cache is stored at:

```
~/.config/compressed-mcp-proxy/cache/tools.json
```

This file contains tool names, descriptions, and parameter schemas. It does **not** contain credentials, tokens, or user data.

## File Permission Summary

| Path | Contains Secrets | Recommended Permissions |
|------|-----------------|------------------------|
| `config.json` | Yes (API keys, tokens) | `600` (owner read/write) |
| `auth/*.json` | Yes (OAuth tokens) | `600` (owner read/write) |
| `auth/` | Directory | `700` (owner only) |
| `cache/tools.json` | No | `644` (default) |

## OAuth Security

mcpzip implements OAuth 2.1 with these security measures:

| Feature | Description |
|---------|-------------|
| **PKCE** | Proof Key for Code Exchange prevents authorization code interception |
| **Code verifier** | 128-character random string, never transmitted over the network |
| **State parameter** | Prevents CSRF attacks on the callback |
| **Localhost callback** | Callback server runs on localhost only, not externally accessible |
| **Dynamic port** | Callback server uses a random available port to avoid conflicts |
| **TLS** | Token exchange happens over HTTPS |

```mermaid
flowchart TD
    GEN[Generate random<br/>code_verifier 128 chars] --> HASH[SHA-256 hash to create<br/>code_challenge]
    HASH --> AUTH[Send code_challenge<br/>to auth server]
    AUTH --> BROWSER[User authorizes<br/>in browser]
    BROWSER --> CALLBACK[Auth code returned<br/>to localhost callback]
    CALLBACK --> EXCHANGE[Exchange code +<br/>code_verifier for tokens]
    EXCHANGE --> VERIFY{Server verifies<br/>SHA-256 of verifier<br/>matches challenge}
    VERIFY -->|Match| TOKEN[Tokens issued]
    VERIFY -->|Mismatch| REJECT[Request rejected]

    style GEN fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style TOKEN fill:#1a1a2e,stroke:#5CF53D,color:#5CF53D
    style REJECT fill:#1a1a2e,stroke:#F87171,color:#F87171
```

:::info Why PKCE Matters
Without PKCE, an attacker who intercepts the authorization code (e.g., via a malicious browser extension or shared device) could exchange it for tokens. With PKCE, the code is useless without the original code verifier, which never leaves mcpzip's process memory.
:::

## Environment Variables

Sensitive values can be set via environment variables instead of storing them in the config file:

| Variable | Purpose |
|----------|---------|
| `GEMINI_API_KEY` | Gemini API key (overrides config file) |

For server-specific credentials, use the `env` block in the config:

```json
{
  "mcpServers": {
    "slack": {
      "command": "npx",
      "args": ["-y", "@anthropic/slack-mcp"],
      "env": {
        "SLACK_TOKEN": "xoxb-..."
      }
    }
  }
}
```

These environment variables are passed **only** to the specific server process. They are not exposed to other servers or to Claude.

:::tip Use Environment References
For maximum security, reference environment variables from your shell instead of hardcoding secrets in the config:

```bash
# In your shell profile (~/.zshrc, ~/.bashrc)
export SLACK_TOKEN="xoxb-your-actual-token"
```

Then in your mcpzip config:
```json
{
  "mcpServers": {
    "slack": {
      "command": "npx",
      "args": ["-y", "@anthropic/slack-mcp"],
      "env": {
        "SLACK_TOKEN": "$SLACK_TOKEN"
      }
    }
  }
}
```
:::

## Network Security

### stdio Transport

stdio servers run as local child processes. Communication happens via stdin/stdout pipes -- no network traffic. The process inherits only the environment variables explicitly listed in the `env` config.

### HTTP Transport

HTTP connections use TLS (HTTPS). mcpzip validates server certificates using the system certificate store.

| Feature | Behavior |
|---------|----------|
| Protocol | HTTPS (TLS 1.2+) |
| Certificate validation | System certificate store |
| Custom headers | Sent with every request |
| OAuth tokens | Sent as `Authorization: Bearer` header |

### SSE Transport

SSE (Server-Sent Events) connections also use HTTPS and follow the same security model as HTTP.

## Process Isolation

Each stdio upstream server runs as a separate OS process:

- Processes are isolated from each other
- Each process gets only its own `env` variables
- Processes are killed on mcpzip shutdown
- A compromised server process cannot access other servers' credentials

```mermaid
flowchart TD
    subgraph mcpzip["mcpzip process"]
        PROXY[Proxy Server]
        CATALOG[Catalog]
        MANAGER[Manager]
    end

    subgraph isolated1["Process: Slack MCP"]
        S1[stdin/stdout]
        ENV1["env: SLACK_TOKEN=xoxb-..."]
    end

    subgraph isolated2["Process: GitHub MCP"]
        S2[stdin/stdout]
        ENV2["env: GH_TOKEN=ghp-..."]
    end

    MANAGER --> S1
    MANAGER --> S2

    style mcpzip fill:#0a0a0f,stroke:#5CF53D,color:#fff
    style isolated1 fill:#0a0a0f,stroke:#60A5FA,color:#fff
    style isolated2 fill:#0a0a0f,stroke:#A78BFA,color:#fff
```

## Reporting Vulnerabilities

If you discover a security vulnerability, please report it privately:

1. **Do not** open a public GitHub issue
2. Email security concerns to the [Hypercall team](https://hypercall.xyz)
3. Include steps to reproduce
4. We will acknowledge within 48 hours
