import React, { useState } from "react";

const TOKENS_PER_TOOL = 350;
const MCPZIP_BASE_TOKENS = 1200;

export default function TokenSavingsCalculator() {
  const [servers, setServers] = useState(5);
  const [toolsPerServer, setToolsPerServer] = useState(25);

  const totalTools = servers * toolsPerServer;
  const withoutTokens = totalTools * TOKENS_PER_TOOL;
  const withTokens = MCPZIP_BASE_TOKENS;
  const savings = withoutTokens - withTokens;
  const pct = withoutTokens > 0 ? Math.round((savings / withoutTokens) * 100) : 0;

  const barMax = Math.max(withoutTokens, 1);
  const withoutPct = 100;
  const withPct = Math.max((withTokens / barMax) * 100, 2);

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
        marginBottom: "20px",
      }}>
        Token Savings Calculator
      </div>

      <div style={{
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        gap: "20px",
        marginBottom: "24px",
      }}>
        <div>
          <label style={{
            display: "block",
            fontSize: "11px",
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.08em",
            color: "rgba(255,255,255,0.4)",
            marginBottom: "8px",
          }}>
            MCP Servers
          </label>
          <input
            type="range"
            min="1"
            max="20"
            value={servers}
            onChange={(e) => setServers(Number(e.target.value))}
            style={{ width: "100%", accentColor: "#5CF53D" }}
          />
          <div style={{
            fontSize: "28px",
            fontWeight: 700,
            color: "#5CF53D",
            marginTop: "4px",
          }}>{servers}</div>
        </div>
        <div>
          <label style={{
            display: "block",
            fontSize: "11px",
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: "0.08em",
            color: "rgba(255,255,255,0.4)",
            marginBottom: "8px",
          }}>
            Tools per Server
          </label>
          <input
            type="range"
            min="1"
            max="150"
            value={toolsPerServer}
            onChange={(e) => setToolsPerServer(Number(e.target.value))}
            style={{ width: "100%", accentColor: "#5CF53D" }}
          />
          <div style={{
            fontSize: "28px",
            fontWeight: 700,
            color: "#5CF53D",
            marginTop: "4px",
          }}>{toolsPerServer}</div>
        </div>
      </div>

      <div style={{
        display: "grid",
        gridTemplateColumns: "1fr 1fr 1fr",
        gap: "12px",
        marginBottom: "24px",
      }}>
        <div style={{
          background: "rgba(248, 113, 113, 0.06)",
          border: "1px solid rgba(248, 113, 113, 0.15)",
          borderRadius: "10px",
          padding: "14px",
          textAlign: "center",
        }}>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.4)", marginBottom: "4px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em" }}>
            Without mcpzip
          </div>
          <div style={{ fontSize: "22px", fontWeight: 700, color: "#F87171" }}>
            {withoutTokens.toLocaleString()}
          </div>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.35)" }}>tokens</div>
        </div>
        <div style={{
          background: "rgba(92, 245, 61, 0.06)",
          border: "1px solid rgba(92, 245, 61, 0.15)",
          borderRadius: "10px",
          padding: "14px",
          textAlign: "center",
        }}>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.4)", marginBottom: "4px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em" }}>
            With mcpzip
          </div>
          <div style={{ fontSize: "22px", fontWeight: 700, color: "#5CF53D" }}>
            {withTokens.toLocaleString()}
          </div>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.35)" }}>tokens</div>
        </div>
        <div style={{
          background: "rgba(167, 139, 250, 0.06)",
          border: "1px solid rgba(167, 139, 250, 0.15)",
          borderRadius: "10px",
          padding: "14px",
          textAlign: "center",
        }}>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.4)", marginBottom: "4px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em" }}>
            Savings
          </div>
          <div style={{ fontSize: "22px", fontWeight: 700, color: "#A78BFA" }}>
            {pct}%
          </div>
          <div style={{ fontSize: "11px", color: "rgba(255,255,255,0.35)" }}>{savings.toLocaleString()} tokens saved</div>
        </div>
      </div>

      <div style={{ marginTop: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "8px" }}>
          <div style={{ width: "80px", fontSize: "11px", color: "rgba(255,255,255,0.4)", fontWeight: 500 }}>Without</div>
          <div style={{ flex: 1, background: "rgba(255,255,255,0.04)", borderRadius: "6px", height: "24px", overflow: "hidden" }}>
            <div style={{
              width: `${withoutPct}%`,
              height: "100%",
              background: "linear-gradient(90deg, rgba(248,113,113,0.4), rgba(248,113,113,0.2))",
              borderRadius: "6px",
              transition: "width 0.4s ease",
            }} />
          </div>
          <div style={{ width: "70px", fontSize: "12px", color: "#F87171", fontWeight: 600, textAlign: "right" }}>
            {totalTools} tools
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <div style={{ width: "80px", fontSize: "11px", color: "rgba(255,255,255,0.4)", fontWeight: 500 }}>With</div>
          <div style={{ flex: 1, background: "rgba(255,255,255,0.04)", borderRadius: "6px", height: "24px", overflow: "hidden" }}>
            <div style={{
              width: `${withPct}%`,
              height: "100%",
              background: "linear-gradient(90deg, rgba(92,245,61,0.5), rgba(92,245,61,0.25))",
              borderRadius: "6px",
              transition: "width 0.4s ease",
            }} />
          </div>
          <div style={{ width: "70px", fontSize: "12px", color: "#5CF53D", fontWeight: 600, textAlign: "right" }}>
            3 tools
          </div>
        </div>
      </div>
    </div>
  );
}
