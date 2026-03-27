import { API_BASE } from "./constants";
import { SimulationResult } from "./types";

export async function fetchBaseline(year: number): Promise<SimulationResult> {
  const res = await fetch(`${API_BASE}/api/baseline/${year}`);
  if (!res.ok) throw new Error(`Failed to fetch baseline: ${res.statusText}`);
  return res.json();
}

export async function fetchParameters(year: number): Promise<Record<string, unknown>> {
  const res = await fetch(`${API_BASE}/api/parameters/${year}`);
  if (!res.ok) throw new Error(`Failed to fetch parameters: ${res.statusText}`);
  return res.json();
}

export async function runSimulation(
  year: number,
  reform: Record<string, unknown>
): Promise<SimulationResult> {
  const res = await fetch(`${API_BASE}/api/simulate`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ year, ...reform }),
  });
  if (!res.ok) throw new Error(`Simulation failed: ${res.statusText}`);
  return res.json();
}
