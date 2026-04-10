import React from "react";

const boxStyle = (color, delay = 0) => ({
  background: `rgba(${color}, 0.08)`,
  border: `1px solid rgba(${color}, 0.3)`,
  borderRadius: "10px",
  padding: "14px 18px",
  textAlign: "center",
  fontSize: "13px",
  fontWeight: 600,
  color: `rgba(${color}, 1)`,
  position: "relative",
  animation: `fadeInUp 0.6s ease ${delay}s both`,
});

const arrowStyle = (direction = "right", delay = 0) => ({
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: "rgba(255,255,255,0.3)",
  fontSize: "20px",
  animation: `fadeIn 0.6s ease ${delay}s both`,
  ...(direction === "down" ? { transform: "rotate(90deg)" } : {}),
});

const labelStyle = {
  fontSize: "10px",
  color: "rgba(255,255,255,0.4)",
  textTransform: "uppercase",
  letterSpacing: "0.1em",
  fontWeight: 600,
  marginBottom: "4px",
};

export default function ArchitectureDiagram() {
  return (
    <>
      <style>{`
        @keyframes fadeInUp {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }
        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        @keyframes pulseFlow {
          0%, 100% { opacity: 0.3; }
          50% { opacity: 0.8; }
        }
        .arch-flow-dot {
          width: 4px;
          height: 4px;
          border-radius: 50%;
          background: #5CF53D;
          animation: pulseFlow 2s ease infinite;
          position: absolute;
        }
        .arch-container {
          background: rgba(255,255,255,0.02);
          border: 1px solid rgba(255,255,255,0.06);
          border-radius: 16px;
          padding: 32px;
          margin: 24px 0;
          overflow: hidden;
        }
        .arch-grid {
          display: grid;
          grid-template-columns: 140px 40px 1fr 40px 140px;
          align-items: center;
          gap: 8px;
        }
        .arch-proxy-box {
          background: rgba(92, 245, 61, 0.04);
          border: 1px solid rgba(92, 245, 61, 0.15);
          border-radius: 14px;
          padding: 20px;
        }
        .arch-proxy-grid {
          display: grid;
          grid-template-columns: 1fr 1fr 1fr;
          gap: 10px;
          margin-top: 12px;
        }
        .arch-server-stack {
          display: flex;
          flex-direction: column;
          gap: 8px;
        }
      `}</style>
      <div className="arch-container">
        <div className="arch-grid">
          {/* Claude */}
          <div>
            <div style={labelStyle}>Downstream</div>
            <div style={boxStyle("96, 165, 250", 0)}>
              Claude Code
            </div>
          </div>

          {/* Arrow */}
          <div style={arrowStyle("right", 0.2)}>
            <span style={{ position: "relative" }}>
              <span style={{ opacity: 0.5 }}>&#8594;</span>
              <span className="arch-flow-dot" style={{ top: "-2px", left: "50%", animationDelay: "0s" }} />
            </span>
          </div>

          {/* Proxy */}
          <div className="arch-proxy-box">
            <div style={{ textAlign: "center", marginBottom: "4px" }}>
              <span style={{ fontSize: "11px", ...labelStyle, marginBottom: 0 }}>mcpzip proxy</span>
            </div>
            <div style={{
              textAlign: "center",
              fontSize: "14px",
              fontWeight: 700,
              color: "#5CF53D",
              marginBottom: "12px",
            }}>3 Meta-Tools</div>
            <div className="arch-proxy-grid">
              <div style={boxStyle("92, 245, 61", 0.3)}>
                search_tools
              </div>
              <div style={boxStyle("92, 245, 61", 0.4)}>
                describe_tool
              </div>
              <div style={boxStyle("92, 245, 61", 0.5)}>
                execute_tool
              </div>
            </div>
            <div style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr 1fr",
              gap: "10px",
              marginTop: "10px",
            }}>
              <div style={{ ...boxStyle("167, 139, 250", 0.6), fontSize: "11px", padding: "8px" }}>
                Searcher
              </div>
              <div style={{ ...boxStyle("251, 191, 36", 0.7), fontSize: "11px", padding: "8px" }}>
                Catalog
              </div>
              <div style={{ ...boxStyle("96, 165, 250", 0.8), fontSize: "11px", padding: "8px" }}>
                Manager
              </div>
            </div>
          </div>

          {/* Arrow */}
          <div style={arrowStyle("right", 0.6)}>
            <span style={{ position: "relative" }}>
              <span style={{ opacity: 0.5 }}>&#8594;</span>
              <span className="arch-flow-dot" style={{ top: "-2px", left: "50%", animationDelay: "0.5s" }} />
            </span>
          </div>

          {/* Upstream Servers */}
          <div>
            <div style={labelStyle}>Upstream</div>
            <div className="arch-server-stack">
              <div style={{ ...boxStyle("248, 113, 113", 0.7), fontSize: "11px", padding: "8px 12px" }}>
                Slack (stdio)
              </div>
              <div style={{ ...boxStyle("251, 191, 36", 0.8), fontSize: "11px", padding: "8px 12px" }}>
                Todoist (http)
              </div>
              <div style={{ ...boxStyle("52, 211, 153", 0.9), fontSize: "11px", padding: "8px 12px" }}>
                GitHub (stdio)
              </div>
              <div style={{ ...boxStyle("167, 139, 250", 1.0), fontSize: "11px", padding: "8px 12px" }}>
                Gmail (http)
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
