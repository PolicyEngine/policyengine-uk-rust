"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import ParameterSlider from "@/components/ParameterSlider";
import DecileChart from "@/components/DecileChart";
import BudgetarySummary from "@/components/BudgetarySummary";
import WinnersLosers from "@/components/WinnersLosers";
import ProvisionWaterfall, { WaterfallEntry } from "@/components/ProvisionWaterfall";
import BaselineSlide from "@/components/BaselineSlide";
import { SLIDERS, SECTIONS, YEARS, BASELINE_YEARS } from "@/lib/constants";
import { palette, FF_MONO, FF_DISPLAY } from "@/lib/theme";
import {
  fetchAllBaselines,
  fetchParameters,
  runSimulation,
  runSimulationMultiYear,
} from "@/lib/api";
import { SimulationResult } from "@/lib/types";

const HEADER_HEIGHT = 56;
const PRIMARY = palette.accent;

function getParam(params: Record<string, unknown>, path: string[]): number {
  let current: unknown = params;
  for (const key of path) {
    if (current == null) return 0;
    if (Array.isArray(current)) {
      current = (current as unknown[])[parseInt(key)];
    } else if (typeof current === "object") {
      current = (current as Record<string, unknown>)[key];
    }
  }
  return typeof current === "number" ? current : 0;
}

function buildReformOverlay(
  sliderValues: Record<string, number>,
  baselineValues: Record<string, number>,
  baselineParams: Record<string, unknown>
): Record<string, unknown> {
  const overlay: Record<string, unknown> = {};

  for (const slider of SLIDERS) {
    const val = sliderValues[slider.key];
    const base = baselineValues[slider.key];
    if (Math.abs(val - base) < slider.step * 0.5) continue;

    const isBracketPath =
      slider.path[1] === "uk_brackets" || slider.path[1] === "scottish_brackets";

    if (isBracketPath) {
      const section = slider.path[0];
      const bracketKey = slider.path[1];
      if (!overlay[section]) overlay[section] = {};
      const sectionOverlay = overlay[section] as Record<string, unknown>;
      if (!sectionOverlay[bracketKey]) {
        const baseSection = baselineParams[section] as Record<string, unknown>;
        sectionOverlay[bracketKey] = JSON.parse(
          JSON.stringify(baseSection[bracketKey])
        );
      }
      const brackets = sectionOverlay[bracketKey] as Array<
        Record<string, number>
      >;
      const idx = parseInt(slider.path[2]);
      const field = slider.path[3];
      brackets[idx][field] = val;
    } else {
      const section = slider.path[0];
      if (!overlay[section]) overlay[section] = {};
      const sectionObj = overlay[section] as Record<string, unknown>;
      sectionObj[slider.path[1]] = val;
    }
  }

  return overlay;
}

function formatBnShort(v: number): string {
  const bn = v / 1e9;
  const sign = bn >= 0 ? "+" : "";
  return `${sign}£${bn.toFixed(1)}bn`;
}

export default function Home() {
  const [loading, setLoading] = useState(true);
  const [results, setResults] = useState<Record<string, SimulationResult>>({});
  const [baselineParams, setBaselineParams] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [sliderValues, setSliderValues] = useState<Record<string, number>>({});
  const [baselineValues, setBaselineValues] = useState<Record<string, number>>(
    {}
  );
  const [baselines, setBaselines] = useState<Record<string, SimulationResult>>(
    {}
  );
  const debounceRef = useRef<NodeJS.Timeout | null>(null);
  const [hasReform, setHasReform] = useState(false);
  const [selectedYear, setSelectedYear] = useState(2025);
  const [activeTab, setActiveTab] = useState(0);
  const [waterfallEntries, setWaterfallEntries] = useState<WaterfallEntry[]>([]);
  const [waterfallLoading, setWaterfallLoading] = useState(false);

  // Load baselines + params for primary year (2025) on mount
  useEffect(() => {
    setLoading(true);
    Promise.all([fetchAllBaselines(), fetchParameters(2025)])
      .then(([allBaselines, params]) => {
        setBaselines(allBaselines);
        setResults(allBaselines);
        setBaselineParams(params);

        const values: Record<string, number> = {};
        for (const s of SLIDERS) {
          values[s.key] = getParam(params, s.path);
        }
        setSliderValues(values);
        setBaselineValues(values);
        setHasReform(false);
        setLoading(false);
      })
      .catch((e) => {
        console.error("Failed to load baselines:", e);
        setLoading(false);
      });
  }, []);

  const handleSliderChange = useCallback(
    (key: string, value: number) => {
      const newValues = { ...sliderValues, [key]: value };
      setSliderValues(newValues);

      const anyChanged = SLIDERS.some(
        (s) =>
          Math.abs(newValues[s.key] - baselineValues[s.key]) > s.step * 0.5
      );
      setHasReform(anyChanged);

      if (debounceRef.current) clearTimeout(debounceRef.current);

      if (!anyChanged) {
        setResults(baselines);
        setWaterfallEntries([]);
        return;
      }

      debounceRef.current = setTimeout(() => {
        if (!baselineParams) return;
        setLoading(true);
        setWaterfallLoading(true);
        const overlay = buildReformOverlay(
          newValues,
          baselineValues,
          baselineParams
        );

        // Changed provisions in order
        const changedSliders = SLIDERS.filter(
          (s) => Math.abs(newValues[s.key] - baselineValues[s.key]) > s.step * 0.5
        );

        // Multi-year results + per-provision waterfall for selected year, in parallel
        Promise.all([
          runSimulationMultiYear(YEARS, overlay),
          // Run cumulative overlays [:1], [:2], ... [:N] all in parallel
          Promise.all(
            changedSliders.map((_, i) => {
              const cumOverlay = buildReformOverlay(
                // Apply only first i+1 changed sliders
                Object.fromEntries(
                  SLIDERS.map((s) => {
                    const idx = changedSliders.indexOf(s);
                    return [s.key, idx >= 0 && idx <= i ? newValues[s.key] : baselineValues[s.key]];
                  })
                ),
                baselineValues,
                baselineParams
              );
              return runSimulation(selectedYear, cumOverlay);
            })
          ),
        ])
          .then(([multiRes, provisionResults]) => {
            setResults(multiRes);

            const baselineNetCost = baselines[String(selectedYear)]?.budgetary_impact.net_cost ?? 0;
            const entries: WaterfallEntry[] = [
              { label: "Baseline", netCost: baselineNetCost },
              ...provisionResults.map((r, i) => ({
                label: changedSliders[i].label,
                netCost: r.budgetary_impact.net_cost,
              })),
            ];
            setWaterfallEntries(entries);
            setLoading(false);
            setWaterfallLoading(false);
          })
          .catch((e) => {
            console.error("Simulation error:", e);
            setLoading(false);
            setWaterfallLoading(false);
          });
      }, 300);
    },
    [sliderValues, baselineValues, baselineParams, baselines]
  );

  const resetAll = useCallback(() => {
    setSliderValues({ ...baselineValues });
    setHasReform(false);
    setResults(baselines);
    setWaterfallEntries([]);
  }, [baselineValues, baselines]);

  const numChanged = SLIDERS.filter(
    (s) =>
      Math.abs((sliderValues[s.key] ?? 0) - (baselineValues[s.key] ?? 0)) >
      s.step * 0.5
  ).length;

  const selectedResult = results[String(selectedYear)];

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        background: palette.bgApp,
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          height: HEADER_HEIGHT,
          flexShrink: 0,
          background: palette.bgApp,
          borderBottom: `1px solid ${palette.border}`,
          display: "flex",
          alignItems: "stretch",
          padding: "0 8px",
        }}
      >
        <div
          style={{
            padding: "0 20px",
            flexShrink: 0,
            borderRight: `1px solid ${palette.border}`,
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <span
            style={{
              fontFamily: FF_DISPLAY,
              fontWeight: 400,
              fontStyle: "italic",
              fontSize: 22,
              color: PRIMARY,
              lineHeight: 1,
            }}
          >
            PolicyEngine UK
          </span>
        </div>

        <div style={{ flex: 1, display: "flex", alignItems: "stretch" }}>
          {["Baseline Forecasts", "Policy Calculator"].map((title, i) => {
            const isActive = activeTab === i;
            return (
              <button
                key={i}
                onClick={() => setActiveTab(i)}
                style={{
                  fontFamily: FF_MONO,
                  fontSize: 13,
                  color: isActive ? palette.textPrimary : palette.textDimmed,
                  background: "transparent",
                  border: "none",
                  borderBottom: isActive ? `3px solid ${PRIMARY}` : "3px solid transparent",
                  borderTop: "3px solid transparent",
                  padding: "0 20px",
                  height: HEADER_HEIGHT,
                  cursor: "pointer",
                  whiteSpace: "nowrap",
                }}
              >
                {i + 1}. {title}
              </button>
            );
          })}
        </div>

        {activeTab === 1 && hasReform && (
          <div style={{ display: "flex", alignItems: "center", paddingRight: 12 }}>
            <button
              onClick={resetAll}
              style={{
                fontFamily: FF_MONO,
                fontSize: 13,
                color: "#fff",
                background: palette.accent,
                border: `1.5px solid ${palette.accent}`,
                padding: "5px 12px",
                cursor: "pointer",
                borderRadius: 0,
                fontWeight: 600,
              }}
            >
              Reset all ({numChanged})
            </button>
          </div>
        )}
      </div>

      {/* Loading spinner */}
      {loading && (
        <div
          style={{
            position: "fixed",
            bottom: 20,
            right: 20,
            zIndex: 1000,
            width: 36,
            height: 36,
            border: `3px solid ${palette.border}`,
            borderTop: `3px solid ${PRIMARY}`,
            borderRadius: "50%",
            animation: "spin 0.8s linear infinite",
          }}
        />
      )}

      {/* Tab 1: Baseline Forecasts */}
      <div
        style={{
          display: activeTab === 0 ? "flex" : "none",
          height: `calc(100vh - ${HEADER_HEIGHT}px)`,
          overflow: "hidden",
          background: palette.bgApp,
          padding: "16px 20px",
        }}
      >
        <BaselineSlide baselines={baselines} years={BASELINE_YEARS} />
      </div>

      {/* Tab 2: Policy Calculator */}
      <div
        style={{
          display: activeTab === 1 ? "flex" : "none",
          height: `calc(100vh - ${HEADER_HEIGHT}px)`,
          overflow: "hidden",
          background: palette.bgApp,
          borderTop: `1px solid ${palette.border}`,
        }}
      >
          {/* Left panel: parameters */}
          <div
            style={{
              flex: "0 0 36%",
              minWidth: 0,
              overflow: "hidden",
              display: "flex",
              flexDirection: "column",
              borderRight: `1px solid ${palette.border}`,
            }}
          >
            <div
              style={{
                flexShrink: 0,
                padding: "14px 20px 10px",
                borderBottom: `1px solid ${palette.border}`,
              }}
            >
              <span
                style={{
                  fontFamily: FF_MONO,
                  fontSize: 11,
                  fontWeight: 700,
                  color: palette.textPrimary,
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Tax & benefit parameters
              </span>
            </div>

            <div
              style={{
                flex: 1,
                overflowY: "auto",
                overscrollBehavior: "contain",
                padding: "12px 20px 16px",
                display: "flex",
                flexDirection: "column",
              }}
            >
              {SECTIONS.map((section) => (
                <div
                  key={section}
                  style={{ borderBottom: `1px solid ${palette.borderSubtle}` }}
                >
                  <div style={{ padding: "6px 6px 2px", margin: "0 -6px" }}>
                    <span
                      style={{
                        fontSize: "0.95rem",
                        fontWeight: 600,
                        color: palette.textSecondary,
                        letterSpacing: "0.02em",
                      }}
                    >
                      {section}
                    </span>
                  </div>
                  {SLIDERS.filter((s) => s.section === section).map(
                    (slider) => (
                      <ParameterSlider
                        key={slider.key}
                        label={slider.label}
                        value={sliderValues[slider.key] ?? 0}
                        baselineValue={baselineValues[slider.key] ?? 0}
                        min={slider.min}
                        max={slider.max}
                        step={slider.step}
                        format={slider.format}
                        onChange={(v) => handleSliderChange(slider.key, v)}
                      />
                    )
                  )}
                </div>
              ))}
            </div>
          </div>

          {/* Right panel: multi-year results */}
          <div
            style={{
              flex: 1,
              minWidth: 0,
              overflow: "auto",
              padding: "20px 28px",
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            {/* Multi-year summary table */}
            <div
              style={{
                border: `1px solid ${palette.border}`,
                flexShrink: 0,
              }}
            >
              <table
                style={{
                  width: "100%",
                  borderCollapse: "collapse",
                  fontFamily: FF_MONO,
                  fontSize: 13,
                }}
              >
                <thead>
                  <tr
                    style={{
                      borderBottom: `1px solid ${palette.border}`,
                      background: palette.bgSubtle,
                    }}
                  >
                    <th style={thStyle}>Year</th>
                    <th style={thStyle}>Revenue</th>
                    <th style={thStyle}>Benefits</th>
                    <th style={thStyle}>Net cost</th>
                    <th style={thStyle}>Winners</th>
                    <th style={thStyle}>Losers</th>
                  </tr>
                </thead>
                <tbody>
                  {YEARS.map((y) => {
                    const r = results[String(y)];
                    if (!r) return null;
                    const isSelected = y === selectedYear;
                    return (
                      <tr
                        key={y}
                        onClick={() => setSelectedYear(y)}
                        style={{
                          borderBottom: `1px solid ${palette.borderSubtle}`,
                          cursor: "pointer",
                          background: isSelected
                            ? palette.bgSubtle
                            : "transparent",
                          transition: "background 0.1s",
                        }}
                      >
                        <td style={{ ...tdStyle, fontWeight: 600 }}>
                          {y}/{(y + 1).toString().slice(-2)}
                        </td>
                        <td style={tdStyle}>
                          <ColorVal v={r.budgetary_impact.revenue_change} positive />
                        </td>
                        <td style={tdStyle}>
                          <ColorVal v={r.budgetary_impact.benefit_spending_change} positive={false} />
                        </td>
                        <td style={tdStyle}>
                          <ColorVal v={r.budgetary_impact.net_cost} positive={false} />
                        </td>
                        <td style={tdStyle}>
                          <span style={{ color: palette.positive }}>
                            {r.winners_losers.winners_pct}%
                          </span>
                        </td>
                        <td style={tdStyle}>
                          <span style={{ color: palette.negative }}>
                            {r.winners_losers.losers_pct}%
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {/* Detail for selected year */}
            {selectedResult && (
              <>
                <div
                  style={{
                    fontFamily: FF_MONO,
                    fontSize: 11,
                    color: palette.textDimmed,
                    textTransform: "uppercase",
                    letterSpacing: "0.06em",
                  }}
                >
                  Detail: {selectedYear}/{(selectedYear + 1).toString().slice(-2)}
                </div>
                <BudgetarySummary data={selectedResult.budgetary_impact} />
                {waterfallEntries.length >= 2 && (
                  <ProvisionWaterfall entries={waterfallEntries} loading={waterfallLoading} />
                )}
                <WinnersLosers data={selectedResult.winners_losers} />
                <DecileChart data={selectedResult.decile_impacts} />
              </>
            )}

            {!selectedResult && !loading && (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  height: 200,
                  fontFamily: FF_MONO,
                  fontSize: 13,
                  color: palette.textDimmed,
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Loading simulation...
              </div>
            )}
          </div>
      </div>
    </div>
  );
}

const thStyle: React.CSSProperties = {
  padding: "6px 8px",
  textAlign: "left",
  fontSize: 11,
  fontWeight: 600,
  color: palette.textDimmed,
  textTransform: "uppercase",
  letterSpacing: "0.06em",
  whiteSpace: "nowrap",
};

const tdStyle: React.CSSProperties = {
  padding: "6px 8px",
  fontVariantNumeric: "tabular-nums",
  whiteSpace: "nowrap",
};

function ColorVal({ v, positive }: { v: number; positive: boolean }) {
  const isNeutral = Math.abs(v) < 1e7;
  const isGood = positive ? v >= 0 : v <= 0;
  const color = isNeutral
    ? palette.textDimmed
    : isGood
    ? palette.positive
    : palette.negative;
  return <span style={{ color }}>{formatBnShort(v)}</span>;
}
