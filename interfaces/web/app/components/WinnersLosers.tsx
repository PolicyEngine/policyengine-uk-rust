"use client";

import { WinnersLosers as WLType } from "@/lib/types";
import { palette, FF_MONO, FF_BODY } from "@/lib/theme";

interface Props {
  data: WLType;
}

export default function WinnersLosers({ data }: Props) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: "12px 16px",
        border: `1px solid ${palette.border}`,
      }}
    >
      <div
        style={{
          display: "flex",
          gap: 24,
          fontFamily: FF_MONO,
          fontSize: 14,
          color: palette.textSecondary,
          flexWrap: "wrap",
          alignItems: "baseline",
        }}
      >
        <span>
          <strong style={{ color: palette.positive }}>{data.winners_pct}%</strong>{" "}
          gain
          {data.avg_gain > 0 && (
            <span style={{ color: palette.textDimmed }}>
              {" "}
              (avg +£{data.avg_gain.toLocaleString()}/yr)
            </span>
          )}
        </span>
        <span>
          <strong style={{ color: palette.negative }}>{data.losers_pct}%</strong>{" "}
          lose
          {data.avg_loss > 0 && (
            <span style={{ color: palette.textDimmed }}>
              {" "}
              (avg -£{data.avg_loss.toLocaleString()}/yr)
            </span>
          )}
        </span>
        <span>
          <strong style={{ color: palette.textMuted }}>
            {data.unchanged_pct}%
          </strong>{" "}
          unchanged
        </span>
      </div>

      {/* Stacked bar */}
      <div style={{ display: "flex", height: 4 }}>
        <div
          style={{
            background: palette.positive,
            width: `${data.winners_pct}%`,
            transition: "width 0.3s",
          }}
        />
        <div
          style={{
            background: palette.negative,
            width: `${data.losers_pct}%`,
            transition: "width 0.3s",
          }}
        />
        <div
          style={{
            background: palette.borderMedium,
            width: `${data.unchanged_pct}%`,
            transition: "width 0.3s",
          }}
        />
      </div>
    </div>
  );
}
