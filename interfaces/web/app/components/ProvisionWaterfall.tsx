"use client";

import { palette, FF_MONO } from "@/lib/theme";

export interface WaterfallEntry {
  label: string;
  netCost: number; // absolute net_cost at this cumulative step
}

interface Props {
  entries: WaterfallEntry[]; // ordered: baseline first, then each added provision
  loading: boolean;
}

function formatBn(v: number): string {
  const bn = v / 1e9;
  const sign = bn >= 0 ? "+" : "";
  return `${sign}£${bn.toFixed(1)}bn`;
}

export default function ProvisionWaterfall({ entries, loading }: Props) {
  if (entries.length < 2) return null;

  // Each bar is the delta from the previous step
  const bars = entries.slice(1).map((e, i) => ({
    label: e.label,
    delta: e.netCost - entries[i].netCost,
    cumulative: e.netCost,
  }));

  const allDeltas = bars.map((b) => b.delta);
  const maxAbs = Math.max(...allDeltas.map(Math.abs), 1e8);

  return (
    <div
      style={{
        border: `1px solid ${palette.border}`,
        padding: "14px 16px",
        display: "flex",
        flexDirection: "column",
        gap: 10,
        opacity: loading ? 0.5 : 1,
        transition: "opacity 0.2s",
      }}
    >
      <span
        style={{
          fontFamily: FF_MONO,
          fontSize: 11,
          fontWeight: 700,
          color: palette.textDimmed,
          textTransform: "uppercase",
          letterSpacing: "0.06em",
        }}
      >
        Net cost by provision
      </span>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {bars.map((bar) => {
          const pct = Math.abs(bar.delta) / maxAbs;
          const width = Math.max(pct * 100, 0.5);
          const positive = bar.delta >= 0; // net cost goes up = bad (costs money)
          const color = Math.abs(bar.delta) < 1e7
            ? palette.textDimmed
            : positive
            ? palette.negative
            : palette.positive;

          return (
            <div key={bar.label} style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span
                style={{
                  fontFamily: FF_MONO,
                  fontSize: 11,
                  color: palette.textSecondary,
                  width: 200,
                  flexShrink: 0,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
                title={bar.label}
              >
                {bar.label}
              </span>

              <div style={{ flex: 1, position: "relative", height: 18 }}>
                {/* centre line */}
                <div
                  style={{
                    position: "absolute",
                    left: "50%",
                    top: 0,
                    bottom: 0,
                    width: 1,
                    background: palette.border,
                  }}
                />
                {/* bar */}
                <div
                  style={{
                    position: "absolute",
                    top: 2,
                    bottom: 2,
                    width: `${width / 2}%`,
                    left: positive ? "50%" : `${50 - width / 2}%`,
                    background: color,
                    opacity: 0.85,
                  }}
                />
              </div>

              <span
                style={{
                  fontFamily: FF_MONO,
                  fontSize: 11,
                  color,
                  width: 72,
                  textAlign: "right",
                  flexShrink: 0,
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                {formatBn(bar.delta)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
