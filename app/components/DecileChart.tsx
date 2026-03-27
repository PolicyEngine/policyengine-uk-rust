"use client";

import { useEffect, useRef } from "react";
import * as d3 from "d3";
import { DecileImpact } from "@/lib/types";
import { palette, FF_MONO } from "@/lib/theme";

interface Props {
  data: DecileImpact[];
}

const DECILE_LABELS = [
  "1st",
  "2nd",
  "3rd",
  "4th",
  "5th",
  "6th",
  "7th",
  "8th",
  "9th",
  "10th",
];

const MARGIN = { top: 12, right: 16, bottom: 28, left: 62 };

function formatGBP(v: number): string {
  if (Math.abs(v) >= 1000) {
    return `${v >= 0 ? "+" : ""}£${(v / 1000).toFixed(1)}k`;
  }
  return `${v >= 0 ? "+" : ""}£${Math.round(v)}`;
}

function niceDomain(data: DecileImpact[]): [number, number] {
  const maxAbs = d3.max(data, (d) => Math.abs(d.avg_change)) ?? 0;
  if (maxAbs < 10) return [-100, 100];
  const mag = Math.pow(10, Math.floor(Math.log10(maxAbs)));
  const nice = Math.ceil((maxAbs * 1.15) / mag) * mag;
  return [-nice, nice];
}

export default function DecileChart({ data }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const hasInitialised = useRef(false);

  useEffect(() => {
    const container = containerRef.current;
    const svg = svgRef.current;
    if (!container || !svg) return;

    const width = container.clientWidth;
    const height = 300;
    const innerW = width - MARGIN.left - MARGIN.right;
    const innerH = height - MARGIN.top - MARGIN.bottom;

    const [yMin, yMax] = niceDomain(data);

    const x = d3
      .scaleBand<string>()
      .domain(DECILE_LABELS)
      .range([0, innerW])
      .padding(0.3);

    const y = d3.scaleLinear().domain([yMin, yMax]).range([innerH, 0]);

    const sel = d3.select(svg);
    sel.attr("width", width).attr("height", height);

    // First render — create structure
    if (!hasInitialised.current) {
      hasInitialised.current = true;
      sel.selectAll("*").remove();

      const g = sel
        .append("g")
        .attr("class", "chart")
        .attr("transform", `translate(${MARGIN.left},${MARGIN.top})`);

      // Grid lines
      g.append("g").attr("class", "grid");

      // Zero line
      g.append("line").attr("class", "zero-line");

      // Bars group
      g.append("g").attr("class", "bars");

      // X axis
      g.append("g").attr("class", "x-axis");

      // Y axis
      g.append("g").attr("class", "y-axis");

      // Tooltip
      sel
        .append("g")
        .attr("class", "tooltip-g")
        .style("pointer-events", "none")
        .style("opacity", 0);
    }

    const g = sel.select<SVGGElement>("g.chart");

    // Update grid
    const gridTicks = y.ticks(5);
    const gridSel = g
      .select<SVGGElement>("g.grid")
      .selectAll<SVGLineElement, number>("line")
      .data(gridTicks);

    gridSel
      .join("line")
      .attr("x1", 0)
      .attr("x2", innerW)
      .attr("y1", (d) => y(d))
      .attr("y2", (d) => y(d))
      .attr("stroke", palette.gridLine)
      .attr("stroke-dasharray", "3 3");

    // Zero line
    g.select("line.zero-line")
      .attr("x1", 0)
      .attr("x2", innerW)
      .attr("y1", y(0))
      .attr("y2", y(0))
      .attr("stroke", palette.zeroLine)
      .attr("stroke-width", 1);

    // Bars
    const barData = data.map((d, i) => ({
      ...d,
      label: DECILE_LABELS[i],
    }));

    const bars = g
      .select<SVGGElement>("g.bars")
      .selectAll<SVGRectElement, (typeof barData)[0]>("rect")
      .data(barData, (d) => d.label);

    bars
      .join(
        (enter) =>
          enter
            .append("rect")
            .attr("x", (d) => x(d.label)!)
            .attr("width", x.bandwidth())
            .attr("y", y(0))
            .attr("height", 0)
            .attr("fill", (d) =>
              d.avg_change >= 0 ? palette.positive : palette.negative
            )
            .attr("fill-opacity", 0.8),
        (update) => update,
        (exit) => exit.remove()
      )
      .transition()
      .duration(350)
      .ease(d3.easeCubicOut)
      .attr("x", (d) => x(d.label)!)
      .attr("width", x.bandwidth())
      .attr("y", (d) => (d.avg_change >= 0 ? y(d.avg_change) : y(0)))
      .attr("height", (d) => Math.abs(y(d.avg_change) - y(0)))
      .attr("fill", (d) =>
        d.avg_change >= 0 ? palette.positive : palette.negative
      );

    // X axis
    g.select<SVGGElement>("g.x-axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(
        d3
          .axisBottom(x)
          .tickSize(0)
          .tickPadding(8)
      )
      .call((g) => g.select(".domain").attr("stroke", palette.border))
      .call((g) =>
        g
          .selectAll("text")
          .attr("fill", palette.axisText)
          .attr("font-family", FF_MONO)
          .attr("font-size", 11)
      );

    // Y axis — hide domain line permanently via display:none to avoid flash
    const yAxisG = g.select<SVGGElement>("g.y-axis");
    yAxisG
      .transition()
      .duration(350)
      .call(
        d3
          .axisLeft(y)
          .ticks(5)
          .tickFormat((d) => formatGBP(d as number))
          .tickSize(0)
          .tickPadding(8)
      );
    yAxisG.select(".domain").style("display", "none");
    yAxisG
      .selectAll("text")
      .attr("fill", palette.axisText)
      .attr("font-family", FF_MONO)
      .attr("font-size", 11);

    // Hover interaction via invisible overlay rects
    const overlayData = barData;
    const overlayG = g.select<SVGGElement>("g.bars");

    // Remove old overlays
    overlayG.selectAll("rect.overlay").remove();

    const tooltipG = sel.select<SVGGElement>("g.tooltip-g");

    overlayG
      .selectAll<SVGRectElement, (typeof barData)[0]>("rect.overlay")
      .data(overlayData, (d) => d.label)
      .join("rect")
      .attr("class", "overlay")
      .attr("x", (d) => x(d.label)!)
      .attr("width", x.bandwidth())
      .attr("y", 0)
      .attr("height", innerH)
      .attr("fill", "transparent")
      .style("cursor", "default")
      .on("mouseenter", function (_event, d) {
        const bx = x(d.label)! + x.bandwidth() / 2 + MARGIN.left;
        const by = MARGIN.top + (d.avg_change >= 0 ? y(d.avg_change) - 8 : y(0) + Math.abs(y(d.avg_change) - y(0)) + 16);

        tooltipG
          .style("opacity", 1)
          .attr("transform", `translate(${bx},${by})`);

        tooltipG.selectAll("*").remove();

        tooltipG
          .append("rect")
          .attr("x", -50)
          .attr("y", -28)
          .attr("width", 100)
          .attr("height", 24)
          .attr("fill", palette.tooltipBg)
          .attr("stroke", palette.tooltipBorder)
          .attr("stroke-width", 1);

        tooltipG
          .append("text")
          .attr("text-anchor", "middle")
          .attr("y", -12)
          .attr("font-family", FF_MONO)
          .attr("font-size", 12)
          .attr("font-weight", 600)
          .attr("fill", d.avg_change >= 0 ? palette.positive : palette.negative)
          .text(formatGBP(d.avg_change));
      })
      .on("mouseleave", function () {
        tooltipG.style("opacity", 0);
      });
  }, [data]);

  return (
    <div
      style={{
        border: `1px solid ${palette.border}`,
        padding: "16px 20px",
      }}
    >
      <div
        style={{
          fontFamily: FF_MONO,
          fontSize: 11,
          color: palette.textDimmed,
          textTransform: "uppercase",
          letterSpacing: "0.06em",
          marginBottom: 4,
        }}
      >
        Impact by income decile
      </div>
      <div
        style={{
          fontFamily: "var(--font-body)",
          fontSize: 13,
          color: palette.textMuted,
          marginBottom: 16,
        }}
      >
        Average annual change in household net income
      </div>
      <div ref={containerRef} style={{ width: "100%" }}>
        <svg ref={svgRef} style={{ display: "block", width: "100%", height: 300 }} />
      </div>
    </div>
  );
}
