"use client";

import { useState } from "react";
import { palette, FF_MONO, FF_BODY } from "@/lib/theme";

interface Props {
  label: string;
  value: number;
  baselineValue: number;
  min: number;
  max: number;
  step: number;
  format: "currency" | "percent";
  onChange: (v: number) => void;
}

function formatValue(v: number, format: "currency" | "percent"): string {
  if (format === "currency") {
    return `£${v.toLocaleString("en-GB", { maximumFractionDigits: 0 })}`;
  }
  return `${(v * 100).toFixed(v < 0.1 ? 1 : 0)}%`;
}

export default function ParameterSlider({
  label,
  value,
  baselineValue,
  min,
  max,
  step,
  format,
  onChange,
}: Props) {
  const isChanged = Math.abs(value - baselineValue) > step * 0.5;
  const [hovered, setHovered] = useState(false);

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "5px 6px",
        margin: "0 -6px",
        background: hovered ? palette.bgSubtle : "transparent",
        transition: "background 0.15s",
      }}
    >
      {/* Label */}
      <span
        style={{
          fontSize: "0.9rem",
          fontWeight: 500,
          color: palette.textPrimary,
          lineHeight: 1.3,
          minWidth: 110,
          flexShrink: 0,
        }}
      >
        {label}
      </span>

      {/* Slider track */}
      <div style={{ flex: 1, minWidth: 80 }}>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          style={{ width: "100%", margin: 0 }}
        />
      </div>

      {/* Value */}
      <span
        style={{
          fontFamily: FF_MONO,
          fontSize: "0.95rem",
          fontWeight: 600,
          fontVariantNumeric: "tabular-nums",
          color: isChanged ? palette.accent : palette.textPrimary,
          minWidth: 60,
          textAlign: "right",
          flexShrink: 0,
        }}
      >
        {formatValue(value, format)}
      </span>

      {/* Reset button */}
      {isChanged ? (
        <button
          onClick={() => onChange(baselineValue)}
          style={{
            fontFamily: FF_MONO,
            fontSize: 11,
            color: palette.textMuted,
            background: "transparent",
            border: `1.5px solid ${palette.borderMedium}`,
            padding: "2px 6px",
            cursor: "pointer",
            borderRadius: 0,
            flexShrink: 0,
            transition: "all 0.15s",
            whiteSpace: "nowrap",
          }}
        >
          {formatValue(baselineValue, format)}
        </button>
      ) : (
        <div style={{ width: 52, flexShrink: 0 }} />
      )}
    </div>
  );
}
