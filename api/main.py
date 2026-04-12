"""FastAPI backend for PolicyEngine UK microsimulation."""

import json
import os
import subprocess
from typing import Any, Optional

from fastapi import Depends, FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from api.security import (
    enforce_simulation_rate_limit,
    require_simulation_api_key,
)

app = FastAPI(title="PolicyEngine UK API")

ALLOWED_ORIGINS = [
    "http://localhost:3000",
    # GitHub Pages frontend
    "https://policyengine.github.io",
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_methods=["*"],
    allow_headers=["*"],
)

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST_BINARY = os.path.join(ROOT_DIR, "target", "release", "policyengine-uk-rust")
CLEAN_FRS_DIR = os.path.join(ROOT_DIR, "data", "frs_clean")
AVAILABLE_YEARS = [2023, 2024, 2025, 2026, 2027, 2028, 2029]

baseline_cache: dict[int, dict] = {}
params_cache: dict[int, dict] = {}


def _data_args() -> list[str]:
    if os.path.isdir(CLEAN_FRS_DIR):
        return ["--clean-frs", CLEAN_FRS_DIR]
    return []


def run_simulation(year: int, reform_json: Optional[str] = None) -> dict:
    cmd = [RUST_BINARY, "--year", str(year), "--output", "json"] + _data_args()
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
    child_benefit: Optional[dict[str, Any]] = None
    benefit_cap: Optional[dict[str, Any]] = None
    housing_benefit: Optional[dict[str, Any]] = None
    tax_credits: Optional[dict[str, Any]] = None
    council_tax_reduction: Optional[dict[str, Any]] = None
    scottish_child_payment: Optional[dict[str, Any]] = None
    pension_credit: Optional[dict[str, Any]] = None
    state_pension: Optional[dict[str, Any]] = None


class SimulateMultiYearRequest(BaseModel):
    years: list[int] = [2025, 2026, 2027, 2028, 2029]
    income_tax: Optional[dict[str, Any]] = None
    national_insurance: Optional[dict[str, Any]] = None
    universal_credit: Optional[dict[str, Any]] = None
    child_benefit: Optional[dict[str, Any]] = None
    benefit_cap: Optional[dict[str, Any]] = None
    housing_benefit: Optional[dict[str, Any]] = None
    tax_credits: Optional[dict[str, Any]] = None
    council_tax_reduction: Optional[dict[str, Any]] = None
    scottish_child_payment: Optional[dict[str, Any]] = None
    pension_credit: Optional[dict[str, Any]] = None
    state_pension: Optional[dict[str, Any]] = None


REFORM_SECTIONS = [
    "income_tax", "national_insurance", "universal_credit",
    "child_benefit", "benefit_cap", "housing_benefit",
    "tax_credits", "council_tax_reduction", "scottish_child_payment",
    "pension_credit", "state_pension",
]


def _extract_overlay(req) -> dict[str, Any]:
    overlay: dict[str, Any] = {}
    for section in REFORM_SECTIONS:
        val = getattr(req, section, None)
        if val:
            overlay[section] = val
    return overlay


@app.get("/api/baseline/{year}")
async def get_baseline(year: int):
    if year not in baseline_cache:
        raise HTTPException(404, detail=f"Year {year} not available")
    return baseline_cache[year]


@app.get("/api/baselines")
async def get_all_baselines():
    """Return baseline results for all cached years."""
    return {str(y): baseline_cache[y] for y in sorted(baseline_cache.keys())}


@app.get("/api/parameters/{year}")
async def get_parameters(year: int):
    if year not in params_cache:
        raise HTTPException(404, detail=f"Year {year} not available")
    return params_cache[year]


@app.get("/api/years")
async def get_years():
    return {"years": AVAILABLE_YEARS}


@app.post(
    "/api/simulate",
    dependencies=[
        Depends(require_simulation_api_key),
        Depends(enforce_simulation_rate_limit),
    ],
)
async def simulate(req: SimulateRequest):
    if req.year not in AVAILABLE_YEARS:
        raise HTTPException(400, detail=f"Year {req.year} not available")

    overlay = _extract_overlay(req)
    if not overlay:
        return baseline_cache.get(req.year, run_simulation(req.year))
    return run_simulation(req.year, json.dumps(overlay))


@app.post(
    "/api/simulate-multi",
    dependencies=[
        Depends(require_simulation_api_key),
        Depends(enforce_simulation_rate_limit),
    ],
)
async def simulate_multi(req: SimulateMultiYearRequest):
    """Run the same reform across multiple years. Returns {year: result}."""
    overlay = _extract_overlay(req)
    results = {}
    for year in req.years:
        if year not in AVAILABLE_YEARS:
            continue
        if not overlay:
            results[str(year)] = baseline_cache.get(year, run_simulation(year))
        else:
            results[str(year)] = run_simulation(year, json.dumps(overlay))
    return results


@app.get("/api/health")
async def health():
    return {
        "status": "ok",
        "binary": os.path.exists(RUST_BINARY),
        "frs_data": os.path.isdir(CLEAN_FRS_DIR),
    }
