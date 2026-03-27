"""FastAPI backend for PolicyEngine UK microsimulation."""

import json
import os
import subprocess
from typing import Any, Optional

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

app = FastAPI(title="PolicyEngine UK API")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000"],
    allow_methods=["*"],
    allow_headers=["*"],
)

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST_BINARY = os.path.join(ROOT_DIR, "target", "release", "policyengine-uk-rust")
AVAILABLE_YEARS = [2023, 2024, 2025, 2026, 2027, 2028, 2029]

# Cache baseline results per year
baseline_cache: dict[int, dict] = {}
params_cache: dict[int, dict] = {}


def run_simulation(year: int, reform_json: Optional[str] = None) -> dict:
    cmd = [RUST_BINARY, "--year", str(year), "--output", "json"]
    if reform_json:
        cmd += ["--reform-json", reform_json]
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=30, cwd=ROOT_DIR
        )
    except subprocess.TimeoutExpired:
        raise HTTPException(504, detail="Simulation timed out")
    if result.returncode != 0:
        raise HTTPException(500, detail=f"Simulation failed: {result.stderr}")
    return json.loads(result.stdout)


def get_baseline_params(year: int) -> dict:
    cmd = [RUST_BINARY, "--year", str(year), "--export-params-json"]
    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=10, cwd=ROOT_DIR
    )
    if result.returncode != 0:
        raise HTTPException(500, detail=f"Failed to load params: {result.stderr}")
    return json.loads(result.stdout)


@app.on_event("startup")
async def cache_baselines():
    for year in AVAILABLE_YEARS:
        try:
            baseline_cache[year] = run_simulation(year)
            params_cache[year] = get_baseline_params(year)
            print(f"  Cached baseline for {year}/{year+1}")
        except Exception as e:
            print(f"  Warning: Failed to cache {year}/{year+1}: {e}")


class SimulateRequest(BaseModel):
    year: int = 2025
    income_tax: Optional[dict[str, Any]] = None
    national_insurance: Optional[dict[str, Any]] = None
    universal_credit: Optional[dict[str, Any]] = None


@app.get("/api/baseline/{year}")
async def get_baseline(year: int):
    if year not in baseline_cache:
        raise HTTPException(404, detail=f"Year {year} not available")
    return baseline_cache[year]


@app.get("/api/parameters/{year}")
async def get_parameters(year: int):
    if year not in params_cache:
        raise HTTPException(404, detail=f"Year {year} not available")
    return params_cache[year]


@app.get("/api/years")
async def get_years():
    return {"years": AVAILABLE_YEARS}


@app.post("/api/simulate")
async def simulate(req: SimulateRequest):
    if req.year not in AVAILABLE_YEARS:
        raise HTTPException(400, detail=f"Year {req.year} not available")

    overlay: dict[str, Any] = {}
    if req.income_tax:
        overlay["income_tax"] = req.income_tax
    if req.national_insurance:
        overlay["national_insurance"] = req.national_insurance
    if req.universal_credit:
        overlay["universal_credit"] = req.universal_credit

    if not overlay:
        return baseline_cache.get(req.year, run_simulation(req.year))

    return run_simulation(req.year, json.dumps(overlay))


@app.get("/api/health")
async def health():
    return {"status": "ok", "binary": os.path.exists(RUST_BINARY)}
