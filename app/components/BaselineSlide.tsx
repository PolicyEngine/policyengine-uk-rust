"use client";

import { useMemo, useRef, useState } from "react";
import { SimulationResult } from "@/lib/types";
import { palette, FF_MONO } from "@/lib/theme";

interface Props {
  baselines: Record<string, SimulationResult>;
  years: number[];
}

// ── Stacked area + net line chart ─────────────────────────────────────────

interface StackedSeries {
  label: string;
  color: string;
  values: number[]; // one per year, positive = revenue/tax, negative = spending
}

function StackedAreaChart({
  positiveSeries,
  negativeSeries,
  years,
  formatY,
  realTerms,
  cpiIndices,
}: {
  positiveSeries: StackedSeries[];
  negativeSeries: StackedSeries[];
  years: number[];
  formatY: (v: number) => string;
  realTerms: boolean;
  cpiIndices: number[];
}) {
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);
  const clipId = useRef(`clip-${Math.random().toString(36).slice(2)}`).current;

  const W = 900;
  const H = 400;
  const PAD = { top: 16, right: 16, bottom: 32, left: 72 };
  const innerW = W - PAD.left - PAD.right;
  const innerH = H - PAD.top - PAD.bottom;

  const deflate = (v: number, i: number) =>
    realTerms ? (v / cpiIndices[i]) * 100 : v;

  // Compute stacked values
  const n = years.length;
  if (n < 2) return null;

  // Positive stacks (taxes/revenue) — stacked upward from 0
  const posStacks = positiveSeries.map((s) =>
    s.values.map((v, i) => deflate(v, i))
  );
  const posCumulative: number[][] = [];
  for (let si = 0; si < posStacks.length; si++) {
    posCumulative.push(
      posStacks[si].map((v, i) =>
        posStacks.slice(0, si + 1).reduce((sum, s) => sum + s[i], 0)
      )
    );
  }

  // Negative stacks (benefits/spending) — stacked downward from 0
  const negStacks = negativeSeries.map((s) =>
    s.values.map((v, i) => -deflate(v, i))
  );
  const negCumulative: number[][] = [];
  for (let si = 0; si < negStacks.length; si++) {
    negCumulative.push(
      negStacks[si].map((v, i) =>
        negStacks.slice(0, si + 1).reduce((sum, s) => sum + s[i], 0)
      )
    );
  }

  // Net line = total positive - total negative
  const netValues = Array.from({ length: n }, (_, i) => {
    const totalPos = posStacks.reduce((sum, s) => sum + s[i], 0);
    const totalNeg = negStacks.reduce((sum, s) => sum + s[i], 0);
    return totalPos + totalNeg; // negStacks are already negative
  });

  // Y range
  const maxPos =
    posCumulative.length > 0
      ? Math.max(...posCumulative[posCumulative.length - 1])
      : 0;
  const maxNeg =
    negCumulative.length > 0
      ? Math.min(...negCumulative[negCumulative.length - 1])
      : 0;
  const allVals = [maxPos, maxNeg, ...netValues];
  const yMaxRaw = Math.max(...allVals);
  const yMinRaw = Math.min(...allVals);

  // Nice tick calculation
  const niceStep = (range: number, targetTicks: number) => {
    const rough = range / targetTicks;
    const mag = Math.pow(10, Math.floor(Math.log10(rough)));
    const norm = rough / mag;
    const nice = norm <= 1.5 ? 1 : norm <= 3 ? 2 : norm <= 7 ? 5 : 10;
    return nice * mag;
  };

  const rawRange = (yMaxRaw - yMinRaw) || 1;
  const step = niceStep(rawRange, 6);
  const yMin = Math.floor(yMinRaw / step) * step;
  const yMax = Math.ceil(yMaxRaw / step) * step;

  const ticks: number[] = [];
  for (let t = yMin; t <= yMax + step * 0.01; t += step) {
    ticks.push(t);
  }

  const xScale = (i: number) =>
    PAD.left + (i / (n - 1)) * innerW;
  const yScale = (v: number) =>
    PAD.top + innerH - ((v - yMin) / (yMax - yMin)) * innerH;

  // Build area paths
  const areaPath = (topVals: number[], bottomVals: number[]) => {
    const top = topVals
      .map((v, i) => `${i === 0 ? "M" : "L"} ${xScale(i).toFixed(1)} ${yScale(v).toFixed(1)}`)
      .join(" ");
    const bottom = [...bottomVals]
      .reverse()
      .map(
        (v, i) =>
          `L ${xScale(bottomVals.length - 1 - i).toFixed(1)} ${yScale(v).toFixed(1)}`
      )
      .join(" ");
    return `${top} ${bottom} Z`;
  };

  const linePath = (vals: number[]) =>
    vals
      .map(
        (v, i) =>
          `${i === 0 ? "M" : "L"} ${xScale(i).toFixed(1)} ${yScale(v).toFixed(1)}`
      )
      .join(" ");

  const handleMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const svg = e.currentTarget;
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const svgPt = pt.matrixTransform(svg.getScreenCTM()!.inverse());
    const idx = Math.round(((svgPt.x - PAD.left) / innerW) * (n - 1));
    setHoverIdx(Math.max(0, Math.min(n - 1, idx)));
  };

  // All series for tooltip
  const allSeries = [
    ...positiveSeries.map((s, si) => ({
      label: s.label,
      color: s.color,
      getValue: (i: number) => posStacks[si][i],
    })),
    ...negativeSeries.map((s, si) => ({
      label: s.label,
      color: s.color,
      getValue: (i: number) => negStacks[si][i],
    })),
  ];

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      style={{ width: "100%", height: "100%", display: "block" }}
      preserveAspectRatio="xMidYMid meet"
      onMouseMove={handleMouseMove}
      onMouseLeave={() => setHoverIdx(null)}
    >
      {/* Clip path for left-to-right reveal */}
      <defs>
        <clipPath id={clipId}>
          <rect x={PAD.left} y={0} width={0} height={H}>
            <animate
              attributeName="width"
              from="0"
              to={innerW + PAD.right}
              dur="1.4s"
              fill="freeze"
              calcMode="spline"
              keySplines="0.25 0.1 0.25 1"
              keyTimes="0;1"
            />
          </rect>
        </clipPath>
      </defs>

      {/* Grid lines */}
      {ticks.map((t, i) => (
        <line
          key={i}
          x1={PAD.left}
          x2={W - PAD.right}
          y1={yScale(t)}
          y2={yScale(t)}
          stroke={palette.gridLine}
          strokeWidth={0.5}
        />
      ))}

      {/* Zero line */}
      <line
        x1={PAD.left}
        x2={W - PAD.right}
        y1={yScale(0)}
        y2={yScale(0)}
        stroke={palette.zeroLine}
        strokeWidth={1}
      />

      {/* Y-axis labels */}
      {ticks.map((t, i) => (
        <text
          key={i}
          x={PAD.left - 8}
          y={yScale(t) + 4}
          textAnchor="end"
          fontSize={10}
          fill={palette.axisText}
          fontFamily={FF_MONO}
        >
          {formatY(t)}
        </text>
      ))}

      {/* X-axis labels */}
      {years.map((y, i) => {
        const step = n > 15 ? 5 : n > 8 ? 2 : 1;
        if (i % step !== 0 && i !== n - 1) return null;
        return (
          <text
            key={y}
            x={xScale(i)}
            y={H - 8}
            textAnchor="middle"
            fontSize={10}
            fill={palette.axisText}
            fontFamily={FF_MONO}
          >
            {y}/{(y + 1).toString().slice(-2)}
          </text>
        );
      })}

      <g clipPath={`url(#${clipId})`}>
        {/* Positive stacked areas */}
        {positiveSeries.map((s, si) => {
          const top = posCumulative[si];
          const bottom =
            si === 0
              ? Array(n).fill(0)
              : posCumulative[si - 1];
          return (
            <path
              key={`pos-${s.label}`}
              d={areaPath(top, bottom)}
              fill={s.color}
              opacity={0.6}
            />
          );
        })}

        {/* Negative stacked areas */}
        {negativeSeries.map((s, si) => {
          const top =
            si === 0
              ? Array(n).fill(0)
              : negCumulative[si - 1];
          const bottom = negCumulative[si];
          return (
            <path
              key={`neg-${s.label}`}
              d={areaPath(top, bottom)}
              fill={s.color}
              opacity={0.6}
            />
          );
        })}

        {/* Net line */}
        <path
          d={linePath(netValues)}
          fill="none"
          stroke={palette.textPrimary}
          strokeWidth={2.5}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>

      {/* Hover crosshair + tooltip */}
      {hoverIdx !== null && (
        <>
          <line
            x1={xScale(hoverIdx)}
            x2={xScale(hoverIdx)}
            y1={PAD.top}
            y2={PAD.top + innerH}
            stroke={palette.textDimmed}
            strokeWidth={1}
            strokeDasharray="4 3"
          />
          {/* Tooltip */}
          {(() => {
            const tooltipW = 180;
            const nonZero = allSeries.filter(
              (s) => Math.abs(s.getValue(hoverIdx)) > 1e6
            );
            const tooltipH = 18 + (nonZero.length + 1) * 15 + 4;
            const tx = Math.min(
              xScale(hoverIdx) + 10,
              W - PAD.right - tooltipW - 4
            );
            return (
              <>
                <rect
                  x={tx}
                  y={PAD.top + 4}
                  width={tooltipW}
                  height={tooltipH}
                  rx={3}
                  fill={palette.bgApp}
                  stroke={palette.border}
                  strokeWidth={1}
                />
                <text
                  x={tx + 8}
                  y={PAD.top + 18}
                  fontSize={10}
                  fontWeight={700}
                  fill={palette.textPrimary}
                  fontFamily={FF_MONO}
                >
                  {years[hoverIdx]}/{(years[hoverIdx] + 1).toString().slice(-2)}
                </text>
                {nonZero.map((s, si) => (
                  <text
                    key={s.label}
                    x={tx + 8}
                    y={PAD.top + 34 + si * 15}
                    fontSize={9}
                    fill={s.color}
                    fontFamily={FF_MONO}
                  >
                    {s.label}: {formatY(s.getValue(hoverIdx))}
                  </text>
                ))}
                <text
                  x={tx + 8}
                  y={PAD.top + 34 + nonZero.length * 15}
                  fontSize={10}
                  fontWeight={700}
                  fill={palette.textPrimary}
                  fontFamily={FF_MONO}
                >
                  Net: {formatY(netValues[hoverIdx])}
                </text>
              </>
            );
          })()}
        </>
      )}
    </svg>
  );
}

// ── Legend ─────────────────────────────────────────────────────────────────

function Legend({
  items,
}: {
  items: { label: string; color: string; type?: "area" | "line" }[];
}) {
  return (
    <div style={{ display: "flex", gap: 14, flexWrap: "wrap" }}>
      {items.map((item) => (
        <div
          key={item.label}
          style={{ display: "flex", alignItems: "center", gap: 4 }}
        >
          {item.type === "line" ? (
            <svg width={16} height={10}>
              <line
                x1={0}
                y1={5}
                x2={16}
                y2={5}
                stroke={item.color}
                strokeWidth={2.5}
              />
            </svg>
          ) : (
            <svg width={12} height={10}>
              <rect
                x={0}
                y={1}
                width={12}
                height={8}
                fill={item.color}
                opacity={0.6}
                rx={1}
              />
            </svg>
          )}
          <span
            style={{
              fontFamily: FF_MONO,
              fontSize: 9,
              color: palette.textSecondary,
            }}
          >
            {item.label}
          </span>
        </div>
      ))}
    </div>
  );
}

// ── Panel ──────────────────────────────────────────────────────────────────

function Panel({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div
      style={{
        border: `1px solid ${palette.border}`,
        display: "flex",
        flexDirection: "column",
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          padding: "8px 14px",
          borderBottom: `1px solid ${palette.border}`,
          background: palette.bgSubtle,
          flexShrink: 0,
        }}
      >
        <span
          style={{
            fontFamily: FF_MONO,
            fontSize: 10,
            fontWeight: 700,
            color: palette.textPrimary,
            textTransform: "uppercase",
            letterSpacing: "0.06em",
          }}
        >
          {title}
        </span>
      </div>
      <div
        style={{
          padding: "12px 14px",
          flex: 1,
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
          gap: 8,
        }}
      >
        {children}
      </div>
    </div>
  );
}

// ── Format helpers ─────────────────────────────────────────────────────────

function fmtBnAxis(v: number): string {
  const abs = Math.abs(v);
  const sign = v < 0 ? "-" : "";
  if (abs >= 1e9) return `${sign}£${(abs / 1e9).toFixed(0)}bn`;
  if (abs >= 1e6) return `${sign}£${(abs / 1e6).toFixed(0)}m`;
  return `${sign}£${(abs / 1000).toFixed(0)}k`;
}

// ── Colour palette for series ─────────────────────────────────────────────

const INCOME_COLORS = {
  employment: "#22c55e",           // green
  self_employment: "#15803d",      // dark green
  pension: "#a855f7",              // purple
  savings: "#eab308",              // yellow
  dividends: "#f59e0b",           // amber
  property: "#78716c",            // stone
  other: "#9ca3af",               // gray
};

const TAX_COLORS = {
  income_tax: "#3b82f6",      // blue
  employee_ni: "#6366f1",     // indigo
  employer_ni: "#8b5cf6",     // violet
};

const BENEFIT_COLORS = {
  universal_credit: "#f97316",     // orange
  state_pension: "#ef4444",        // red
  child_benefit: "#ec4899",        // pink
  pension_credit: "#f59e0b",       // amber
  housing_benefit: "#84cc16",      // lime
  child_tax_credit: "#14b8a6",     // teal
  working_tax_credit: "#06b6d4",   // cyan
  income_support: "#a78bfa",       // light violet
  esa_income_related: "#fb923c",   // light orange
  jsa_income_based: "#fbbf24",     // yellow
  carers_allowance: "#34d399",     // emerald
  other: "#9ca3af",                // gray for small items
};

// ── Main slide ─────────────────────────────────────────────────────────────

export default function BaselineSlide({ baselines, years }: Props) {
  const [realTerms, setRealTerms] = useState(false);

  const data = useMemo(
    () =>
      years
        .map((y) => ({ year: y, r: baselines[String(y)] }))
        .filter((d) => !!d.r),
    [baselines, years]
  );

  const validYears = data.map((d) => d.year);
  const cpiIndices = data.map((d) => d.r.cpi_index ?? 100);

  if (data.length === 0) {
    return (
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: FF_MONO,
          fontSize: 13,
          color: palette.textDimmed,
        }}
      >
        Loading baseline data...
      </div>
    );
  }

  // ── Positive series: market income + benefits (income to households) ──

  // Market income series
  const incomeSeries: StackedSeries[] = [
    {
      label: "Employment",
      color: INCOME_COLORS.employment,
      values: data.map((d) => d.r.income_breakdown?.employment_income ?? 0),
    },
    {
      label: "Self-employment",
      color: INCOME_COLORS.self_employment,
      values: data.map((d) => d.r.income_breakdown?.self_employment_income ?? 0),
    },
    {
      label: "Pensions (private)",
      color: INCOME_COLORS.pension,
      values: data.map((d) => d.r.income_breakdown?.pension_income ?? 0),
    },
    {
      label: "Investment",
      color: INCOME_COLORS.dividends,
      values: data.map((d) =>
        (d.r.income_breakdown?.savings_interest_income ?? 0) +
        (d.r.income_breakdown?.dividend_income ?? 0) +
        (d.r.income_breakdown?.property_income ?? 0)
      ),
    },
  ];

  // Benefit series
  const benefitCandidates: { label: string; color: string; values: number[] }[] = [
    { label: "State Pension", color: BENEFIT_COLORS.state_pension, values: data.map((d) => d.r.program_breakdown.state_pension) },
    { label: "Universal Credit", color: BENEFIT_COLORS.universal_credit, values: data.map((d) => d.r.program_breakdown.universal_credit) },
    { label: "Child Benefit", color: BENEFIT_COLORS.child_benefit, values: data.map((d) => d.r.program_breakdown.child_benefit) },
    { label: "Housing Benefit", color: BENEFIT_COLORS.housing_benefit, values: data.map((d) => d.r.program_breakdown.housing_benefit) },
    { label: "Pension Credit", color: BENEFIT_COLORS.pension_credit, values: data.map((d) => d.r.program_breakdown.pension_credit) },
    { label: "Tax Credits", color: BENEFIT_COLORS.child_tax_credit, values: data.map((d) => d.r.program_breakdown.child_tax_credit + d.r.program_breakdown.working_tax_credit) },
    { label: "Income Support", color: BENEFIT_COLORS.income_support, values: data.map((d) => d.r.program_breakdown.income_support) },
    { label: "ESA (Income)", color: BENEFIT_COLORS.esa_income_related, values: data.map((d) => d.r.program_breakdown.esa_income_related) },
    { label: "Carer's Allowance", color: BENEFIT_COLORS.carers_allowance, values: data.map((d) => d.r.program_breakdown.carers_allowance) },
  ];

  const threshold = 2e9;
  const significantBenefits: StackedSeries[] = [];
  const otherBenValues = Array(data.length).fill(0) as number[];

  for (const b of benefitCandidates) {
    if (Math.max(...b.values) > threshold) {
      significantBenefits.push(b);
    } else {
      b.values.forEach((v, i) => { otherBenValues[i] += v; });
    }
  }
  // Add passthrough + small items
  data.forEach((d, i) => {
    otherBenValues[i] += (d.r.program_breakdown.passthrough_benefits ?? 0)
      + d.r.program_breakdown.jsa_income_based
      + d.r.program_breakdown.scottish_child_payment;
  });
  if (Math.max(...otherBenValues) > 1e6) {
    significantBenefits.push({ label: "Other benefits", color: BENEFIT_COLORS.other, values: otherBenValues });
  }

  // All positive series: income + benefits
  const positiveSeries = [...incomeSeries, ...significantBenefits];

  // ── Negative series: taxes ──
  const taxSeries: StackedSeries[] = [
    { label: "Income Tax", color: TAX_COLORS.income_tax, values: data.map((d) => d.r.program_breakdown.income_tax) },
    { label: "Employee NI", color: TAX_COLORS.employee_ni, values: data.map((d) => d.r.program_breakdown.employee_ni) },
    { label: "Employer NI", color: TAX_COLORS.employer_ni, values: data.map((d) => d.r.program_breakdown.employer_ni) },
  ];

  const legendItems = [
    ...incomeSeries.map((s) => ({ label: s.label, color: s.color, type: "area" as const })),
    ...significantBenefits.map((s) => ({ label: s.label, color: s.color, type: "area" as const })),
    ...taxSeries.map((s) => ({ label: s.label, color: s.color, type: "area" as const })),
    { label: "Net income", color: palette.textPrimary, type: "line" as const },
  ];

  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        gap: 10,
        overflow: "hidden",
        minHeight: 0,
      }}
    >
      <Panel
        title={`Household income, benefits & taxes${realTerms ? " (2025/26 prices)" : " (nominal)"}`}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "flex-start",
            gap: 8,
          }}
        >
          <Legend items={legendItems} />
          <button
            onClick={() => setRealTerms(!realTerms)}
            style={{
              fontFamily: FF_MONO,
              fontSize: 10,
              color: palette.textSecondary,
              background: realTerms ? palette.bgSubtle : "transparent",
              border: `1px solid ${palette.border}`,
              padding: "3px 10px",
              cursor: "pointer",
              borderRadius: 0,
              flexShrink: 0,
            }}
          >
            {realTerms ? "Real (2025/26)" : "Nominal"}
          </button>
        </div>
        <div style={{ flex: 1, minHeight: 0 }}>
          <StackedAreaChart
            positiveSeries={positiveSeries}
            negativeSeries={taxSeries}
            years={validYears}
            formatY={fmtBnAxis}
            realTerms={realTerms}
            cpiIndices={cpiIndices}
          />
        </div>
      </Panel>
    </div>
  );
}
