import React from "react";

function Check() {
  return (
    <span style={{
      color: "#5CF53D",
      fontSize: "18px",
      fontWeight: 700,
    }}>&#10003;</span>
  );
}

function Cross() {
  return (
    <span style={{
      color: "#F87171",
      fontSize: "18px",
      fontWeight: 700,
    }}>&#10007;</span>
  );
}

function Value({ children }) {
  return (
    <span style={{
      color: "rgba(255,255,255,0.85)",
      fontSize: "13px",
    }}>
      {children}
    </span>
  );
}

export default function ComparisonTable({ headers, rows }) {
  return (
    <div style={{
      background: "rgba(255,255,255,0.02)",
      border: "1px solid rgba(255,255,255,0.08)",
      borderRadius: "14px",
      overflow: "hidden",
      margin: "24px 0",
    }}>
      <table style={{
        width: "100%",
        borderCollapse: "collapse",
        margin: 0,
        border: "none",
      }}>
        <thead>
          <tr style={{ background: "rgba(255,255,255,0.03)" }}>
            {headers.map((h, i) => (
              <th key={i} style={{
                padding: "14px 18px",
                fontSize: "11px",
                fontWeight: 600,
                textTransform: "uppercase",
                letterSpacing: "0.08em",
                color: i === 0 ? "rgba(255,255,255,0.4)" : (i === 1 ? "#F87171" : "#5CF53D"),
                textAlign: i === 0 ? "left" : "center",
                borderBottom: "1px solid rgba(255,255,255,0.06)",
                border: "none",
              }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr key={i} style={{
              borderBottom: i < rows.length - 1 ? "1px solid rgba(255,255,255,0.04)" : "none",
            }}>
              {row.map((cell, j) => (
                <td key={j} style={{
                  padding: "12px 18px",
                  fontSize: "13px",
                  fontWeight: j === 0 ? 500 : 400,
                  color: j === 0 ? "rgba(255,255,255,0.85)" : "rgba(255,255,255,0.7)",
                  textAlign: j === 0 ? "left" : "center",
                  border: "none",
                  borderBottom: i < rows.length - 1 ? "1px solid rgba(255,255,255,0.04)" : "none",
                }}>
                  {cell === true ? <Check /> : cell === false ? <Cross /> : <Value>{cell}</Value>}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
