export const FF_MONO = "var(--font-mono), 'JetBrains Mono', monospace";
export const FF_DISPLAY =
  "var(--font-display), 'DM Serif Display', Georgia, serif";
export const FF_BODY = "var(--font-body), 'DM Sans', Inter, sans-serif";

export const palette = {
  bgApp: "#ffffff",
  bgPanel: "#ffffff",
  bgPanelHover: "#f3f4f6",
  bgSubtle: "#f1f5f9",
  bgMuted: "#f8f9fa",

  textPrimary: "#1c1a17",
  textSecondary: "#6b6860",
  textMuted: "#9e9a90",
  textDimmed: "#b5b1a9",

  border: "#e2e8f0",
  borderSubtle: "#f1f5f9",
  borderMedium: "#e5e7eb",

  gridLine: "#e5e7eb",
  axisText: "#9e9a90",
  zeroLine: "#9ca3af",

  positive: "#16a34a",
  negative: "#dc2626",
  amber: "#d97706",

  positiveBg: "rgba(22, 163, 74, 0.08)",
  negativeBg: "rgba(220, 38, 38, 0.06)",

  accent: "#228be6",

  tooltipBg: "rgba(255, 255, 255, 0.95)",
  tooltipBorder: "#e2e8f0",
} as const;
