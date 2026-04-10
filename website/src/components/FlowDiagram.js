import React from "react";

const stepColors = [
  "96, 165, 250",   // blue
  "92, 245, 61",    // green
  "167, 139, 250",  // purple
  "251, 191, 36",   // yellow
  "52, 211, 153",   // teal
  "248, 113, 113",  // red
];

function Step({ number, title, description, color, isLast }) {
  return (
    <div style={{ display: "flex", gap: "16px", alignItems: "flex-start" }}>
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", flexShrink: 0 }}>
        <div style={{
          width: "36px",
          height: "36px",
          borderRadius: "50%",
          background: `rgba(${color}, 0.12)`,
          border: `2px solid rgba(${color}, 0.4)`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: "14px",
          fontWeight: 700,
          color: `rgba(${color}, 1)`,
        }}>
          {number}
        </div>
        {!isLast && (
          <div style={{
            width: "2px",
            height: "40px",
            background: `linear-gradient(to bottom, rgba(${color}, 0.3), rgba(255,255,255,0.05))`,
            marginTop: "4px",
          }} />
        )}
      </div>
      <div style={{ paddingTop: "4px", paddingBottom: isLast ? 0 : "20px" }}>
        <div style={{
          fontSize: "15px",
          fontWeight: 600,
          color: `rgba(${color}, 1)`,
          marginBottom: "4px",
        }}>
          {title}
        </div>
        <div style={{
          fontSize: "13px",
          color: "rgba(255,255,255,0.55)",
          lineHeight: 1.6,
        }}>
          {description}
        </div>
      </div>
    </div>
  );
}

export default function FlowDiagram({ steps }) {
  return (
    <div style={{
      background: "rgba(255,255,255,0.02)",
      border: "1px solid rgba(255,255,255,0.06)",
      borderRadius: "14px",
      padding: "24px",
      margin: "24px 0",
    }}>
      {steps.map((step, i) => (
        <Step
          key={i}
          number={i + 1}
          title={step.title}
          description={step.description}
          color={stepColors[i % stepColors.length]}
          isLast={i === steps.length - 1}
        />
      ))}
    </div>
  );
}
