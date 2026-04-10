import React from "react";

const iconColors = {
  compress: "92, 245, 61",
  search: "96, 165, 250",
  speed: "251, 191, 36",
  lock: "167, 139, 250",
  plug: "52, 211, 153",
  refresh: "248, 113, 113",
  box: "34, 211, 238",
  key: "251, 191, 36",
};

const icons = {
  compress: "\u{1F5DC}",
  search: "\u{1F50D}",
  speed: "\u26A1",
  lock: "\u{1F512}",
  plug: "\u{1F50C}",
  refresh: "\u{1F504}",
  box: "\u{1F4E6}",
  key: "\u{1F511}",
};

export default function FeatureGrid({ features }) {
  return (
    <div style={{
      display: "grid",
      gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
      gap: "14px",
      margin: "24px 0",
    }}>
      {features.map((f, i) => {
        const color = iconColors[f.icon] || "255,255,255";
        return (
          <div key={i} style={{
            background: `rgba(${color}, 0.03)`,
            border: `1px solid rgba(${color}, 0.12)`,
            borderRadius: "12px",
            padding: "20px",
            transition: "all 0.2s ease",
            cursor: "default",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = `rgba(${color}, 0.3)`;
            e.currentTarget.style.transform = "translateY(-2px)";
            e.currentTarget.style.boxShadow = `0 8px 24px rgba(${color}, 0.08)`;
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = `rgba(${color}, 0.12)`;
            e.currentTarget.style.transform = "translateY(0)";
            e.currentTarget.style.boxShadow = "none";
          }}
          >
            <div style={{
              fontSize: "24px",
              marginBottom: "10px",
            }}>
              {icons[f.icon] || "\u2022"}
            </div>
            <div style={{
              fontSize: "14px",
              fontWeight: 600,
              color: `rgba(${color}, 1)`,
              marginBottom: "6px",
            }}>
              {f.title}
            </div>
            <div style={{
              fontSize: "12px",
              color: "rgba(255,255,255,0.5)",
              lineHeight: 1.6,
            }}>
              {f.description}
            </div>
          </div>
        );
      })}
    </div>
  );
}
