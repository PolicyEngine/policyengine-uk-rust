"use client";

import { BudgetaryImpact } from "@/lib/types";
import { palette, FF_MONO, FF_BODY } from "@/lib/theme";

interface Props {
  data: BudgetaryImpact;
}

function formatBn(v: number): string {
  const bn = v / 1e9;
  const sign = bn >= 0 ? "+" : "-";
  return `${sign}£${Math.abs(bn).toFixed(1)}bn`;
}

function StatCard({
  label,
  value,
  isPositiveGood,
}: {
  label: string;
  value: number;
  isPositiveGood: boolean;
}) {
  const isPositive = value >= 0;
  const isGood = isPositiveGood ? isPositive : !isPositive;
  const isNeutral = Math.abs(value) < 1e7;
  const colour = isNeutral
    ? palette.textDimmed
    : isGood
    ? palette.positive
    : palette.negative;
  const bgColour = isNeutral
    ? "transparent"
    : isGood
    ? palette.positiveBg
    : palette.negativeBg;

  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        gap: 4,
        padding: "12px 16px",
        background: bgColour,
      }}
    >
      <span
        style={{
          fontFamily: FF_MONO,
          fontSize: 11,
          color: palette.textDimmed,
          textTransform: "uppercase",
          letterSpacing: "0.06em",
        }}
      >
        {label}
      </span>
      <span
        style={{
          fontFamily: FF_MONO,
          fontSize: 22,
          fontWeight: 700,
          fontVariantNumeric: "tabular-nums",
          color: colour,
          lineHeight: 1.1,
        }}
      >
        {formatBn(value)}
      </span>
    </div>
  );
}

export default function BudgetarySummary({ data }: Props) {
  return (
    <div
      style={{
        display: "flex",
        gap: 1,
        background: palette.border,
        border: `1px solid ${palette.border}`,
      }}
    >
      <StatCard
        label="Revenue change"
        value={data.revenue_change}
        isPositiveGood={true}
      />
      <StatCard
        label="Benefit spending"
        value={data.benefit_spending_change}
        isPositiveGood={false}
      />
      <StatCard
        label="Net cost"
        value={data.net_cost}
        isPositiveGood={false}
      />
    </div>
  );
}
