import React, { useState } from "react";

const presetServers = [
  { name: "slack", label: "Slack", transport: "stdio", command: "npx", args: ["-y", "@anthropic/slack-mcp"], envKeys: ["SLACK_TOKEN"] },
  { name: "github", label: "GitHub", transport: "stdio", command: "gh-mcp", args: [], envKeys: [] },
  { name: "linear", label: "Linear", transport: "stdio", command: "npx", args: ["-y", "@anthropic/linear-mcp"], envKeys: ["LINEAR_API_KEY"] },
  { name: "todoist", label: "Todoist", transport: "http", url: "https://todoist.com/mcp", envKeys: [] },
  { name: "gmail", label: "Gmail", transport: "http", url: "https://gmail.mcp.run/sse", envKeys: [] },
  { name: "notion", label: "Notion", transport: "stdio", command: "npx", args: ["-y", "@anthropic/notion-mcp"], envKeys: ["NOTION_TOKEN"] },
  { name: "telegram", label: "Telegram", transport: "http", url: "https://telegram.mcp.run/mcp", envKeys: [] },
  { name: "posthog", label: "PostHog", transport: "http", url: "https://posthog.com/mcp", envKeys: [] },
];

function buildConfig(selected) {
  const config = { mcpServers: {} };
  for (const name of selected) {
    const preset = presetServers.find((p) => p.name === name);
    if (!preset) continue;
    if (preset.transport === "stdio") {
      const entry = { command: preset.command };
      if (preset.args.length > 0) entry.args = preset.args;
      if (preset.envKeys.length > 0) {
        entry.env = {};
        for (const key of preset.envKeys) {
          entry.env[key] = `your-${key.toLowerCase().replace(/_/g, "-")}`;
        }
      }
      config.mcpServers[preset.name] = entry;
    } else {
      config.mcpServers[preset.name] = {
        type: "http",
        url: preset.url,
      };
    }
  }
  return config;
}

export default function ConfigBuilder() {
  const [selected, setSelected] = useState(["slack", "github"]);

  const toggle = (name) => {
    setSelected((prev) =>
      prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name]
    );
  };

  const config = buildConfig(selected);
  const json = JSON.stringify(config, null, 2);

  return (
    <div style={{
      background: "rgba(255,255,255,0.02)",
      border: "1px solid rgba(255,255,255,0.08)",
      borderRadius: "16px",
      padding: "28px",
      margin: "24px 0",
    }}>
      <div style={{
        fontSize: "16px",
        fontWeight: 700,
        color: "#fff",
        marginBottom: "6px",
      }}>
        Config Builder
      </div>
      <div style={{
        fontSize: "12px",
        color: "rgba(255,255,255,0.45)",
        marginBottom: "20px",
      }}>
        Select your MCP servers to generate a config file.
      </div>

      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))",
        gap: "8px",
        marginBottom: "20px",
      }}>
        {presetServers.map((s) => {
          const active = selected.includes(s.name);
          return (
            <button
              key={s.name}
              onClick={() => toggle(s.name)}
              style={{
                background: active ? "rgba(92, 245, 61, 0.1)" : "rgba(255,255,255,0.03)",
                border: `1px solid ${active ? "rgba(92, 245, 61, 0.35)" : "rgba(255,255,255,0.08)"}`,
                borderRadius: "8px",
                padding: "10px 14px",
                cursor: "pointer",
                color: active ? "#5CF53D" : "rgba(255,255,255,0.5)",
                fontSize: "13px",
                fontWeight: active ? 600 : 400,
                transition: "all 0.15s ease",
                fontFamily: "inherit",
              }}
            >
              {active ? "\u2713 " : ""}{s.label}
            </button>
          );
        })}
      </div>

      <div style={{
        fontSize: "11px",
        fontWeight: 600,
        textTransform: "uppercase",
        letterSpacing: "0.08em",
        color: "rgba(255,255,255,0.35)",
        marginBottom: "8px",
      }}>
        Generated Config &mdash; ~/.config/compressed-mcp-proxy/config.json
      </div>

      <div style={{
        background: "rgba(17, 17, 22, 1)",
        border: "1px solid rgba(255,255,255,0.06)",
        borderRadius: "10px",
        padding: "16px",
        overflow: "auto",
      }}>
        <pre style={{
          margin: 0,
          padding: 0,
          background: "transparent",
          border: "none",
          fontSize: "12px",
          lineHeight: 1.7,
          color: "rgba(255,255,255,0.7)",
        }}>
          <code style={{
            background: "transparent",
            border: "none",
            padding: 0,
            color: "rgba(255,255,255,0.7)",
          }}>
            {json}
          </code>
        </pre>
      </div>

      {selected.length > 0 && (
        <div style={{
          marginTop: "14px",
          fontSize: "12px",
          color: "rgba(255,255,255,0.4)",
        }}>
          {selected.length} server{selected.length !== 1 ? "s" : ""} selected
          {" "}&middot;{" "}
          ~{selected.length * 25} tools compressed into 3 meta-tools
        </div>
      )}
    </div>
  );
}
