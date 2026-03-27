"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import ParameterSlider from "@/components/ParameterSlider";
import DecileChart from "@/components/DecileChart";
import BudgetarySummary from "@/components/BudgetarySummary";
import WinnersLosers from "@/components/WinnersLosers";
import { SLIDERS } from "@/lib/constants";
import { palette, FF_MONO, FF_DISPLAY, FF_BODY } from "@/lib/theme";
import { fetchBaseline, fetchParameters, runSimulation } from "@/lib/api";
import { SimulationResult } from "@/lib/types";

const YEARS = [2025, 2026, 2027, 2028, 2029];
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

    if (slider.path[1] === "uk_brackets") {
      if (!overlay["income_tax"]) overlay["income_tax"] = {};
      const itOverlay = overlay["income_tax"] as Record<string, unknown>;
      if (!itOverlay["uk_brackets"]) {
        const baseIT = baselineParams["income_tax"] as Record<string, unknown>;
        itOverlay["uk_brackets"] = JSON.parse(
          JSON.stringify(baseIT["uk_brackets"])
        );
      }
      const brackets = itOverlay["uk_brackets"] as Array<
        Record<string, number>
      >;
      const idx = parseInt(slider.path[2]);
      brackets[idx].rate = val;
    } else {
      const section = slider.path[0];
      if (!overlay[section]) overlay[section] = {};
      const sectionObj = overlay[section] as Record<string, unknown>;
      sectionObj[slider.path[1]] = val;
    }
  }

  return overlay;
}

export default function Home() {
  const [year, setYear] = useState(2025);
  const [loading, setLoading] = useState(true);
  const [result, setResult] = useState<SimulationResult | null>(null);
  const [baselineParams, setBaselineParams] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [sliderValues, setSliderValues] = useState<Record<string, number>>({});
  const [baselineValues, setBaselineValues] = useState<Record<string, number>>(
    {}
  );
  const debounceRef = useRef<NodeJS.Timeout | null>(null);
  const [hasReform, setHasReform] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    Promise.all([fetchBaseline(year), fetchParameters(year)])
      .then(([baseline, params]) => {
        if (cancelled) return;
        setResult(baseline);
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
        console.error("Failed to load baseline:", e);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [year]);

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
        fetchBaseline(year).then(setResult);
        return;
      }

      debounceRef.current = setTimeout(() => {
        if (!baselineParams) return;
        setLoading(true);
        const overlay = buildReformOverlay(
          newValues,
          baselineValues,
          baselineParams
        );
        runSimulation(year, overlay)
          .then((res) => {
            setResult(res);
            setLoading(false);
          })
          .catch((e) => {
            console.error("Simulation error:", e);
            setLoading(false);
          });
      }, 300);
    },
    [sliderValues, baselineValues, baselineParams, year]
  );

  const resetAll = useCallback(() => {
    setSliderValues({ ...baselineValues });
    setHasReform(false);
    fetchBaseline(year).then(setResult);
  }, [baselineValues, year]);

  const numChanged = SLIDERS.filter(
    (s) =>
      Math.abs((sliderValues[s.key] ?? 0) - (baselineValues[s.key] ?? 0)) >
      s.step * 0.5
  ).length;

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

        <div
          style={{
            flex: 1,
            overflow: "hidden",
            display: "flex",
            alignItems: "stretch",
          }}
        >
          {YEARS.map((y) => {
            const isActive = year === y;
            return (
              <button
                key={y}
                onClick={() => setYear(y)}
                style={{
                  fontFamily: FF_MONO,
                  fontSize: 14,
                  color: isActive ? palette.textPrimary : palette.textMuted,
                  background: "transparent",
                  border: "none",
                  borderBottom: isActive
                    ? `3px solid ${PRIMARY}`
                    : "3px solid transparent",
                  borderTop: "3px solid transparent",
                  padding: "0 20px",
                  height: HEADER_HEIGHT,
                  cursor: "pointer",
                  whiteSpace: "nowrap",
                  transition: "color 0.15s, border-color 0.15s",
                  fontWeight: isActive ? 600 : 400,
                }}
              >
                {y}/{(y + 1).toString().slice(-2)}
              </button>
            );
          })}
        </div>

        {/* Reset all — top right */}
        {hasReform && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              paddingRight: 12,
            }}
          >
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
                transition: "all 0.15s",
              }}
            >
              Reset all ({numChanged})
            </button>
          </div>
        )}
      </div>

      {/* Loading bar */}
      {loading && (
        <div style={{ height: 2, background: palette.border, overflow: "hidden" }}>
          <div
            style={{
              height: "100%",
              background: PRIMARY,
              width: "100%",
              animation: "pulse 1s ease-in-out infinite",
            }}
          />
        </div>
      )}

      {/* Body */}
      <div
        style={{
          flex: 1,
          overflow: "hidden",
          position: "relative",
          padding: "10px 16px",
        }}
      >
        <div
          style={{
            display: "flex",
            height: "100%",
            overflow: "hidden",
            background: palette.bgApp,
            border: `1px solid ${palette.border}`,
          }}
        >
          {/* Left panel: parameters */}
          <div
            style={{
              flex: "0 0 40%",
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
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
              }}
            >
              <span
                style={{
                  fontFamily: FF_BODY,
                  fontSize: 16,
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
              {["Income Tax", "National Insurance", "Universal Credit"].map(
                (section) => (
                  <div
                    key={section}
                    style={{ borderBottom: `1px solid ${palette.borderSubtle}` }}
                  >
                    <div
                      style={{
                        padding: "6px 6px 2px",
                        margin: "0 -6px",
                      }}
                    >
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
                )
              )}
            </div>
          </div>

          {/* Right panel: results */}
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
            {result ? (
              <>
                <BudgetarySummary data={result.budgetary_impact} />
                <WinnersLosers data={result.winners_losers} />
                <DecileChart data={result.decile_impacts} />
              </>
            ) : (
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
    </div>
  );
}
