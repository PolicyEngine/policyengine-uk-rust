"""FastAPI backend for PolicyEngine UK microsimulation."""

import json
import os
from typing import Any, Optional

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from policyengine_uk_compiled import Simulation, Parameters as PolicyParams

app = FastAPI(title="PolicyEngine UK API")

ALLOWED_ORIGINS = [
    "http://localhost:3000",
    "https://policyengine.github.io",
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_methods=["*"],
    allow_headers=["*"],
)

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLEAN_FRS_DIR = os.path.join(ROOT_DIR, "data", "frs")
FRS_RAW_DIR = os.path.join(ROOT_DIR, "data", "frs_raw")
AVAILABLE_YEARS = list(range(1994, 2030))

baseline_cache: dict[int, dict] = {}
params_cache: dict[int, dict] = {}


def _data_kwargs() -> dict:
    if os.path.isdir(CLEAN_FRS_DIR):
        return {"clean_frs_base": CLEAN_FRS_DIR}
    if os.path.isdir(FRS_RAW_DIR):
        return {"frs_raw": FRS_RAW_DIR}
    return {}


def run_simulation(year: int, reform_json: Optional[str] = None) -> dict:
    sim = Simulation(year=year, **_data_kwargs())
    policy = None
    if reform_json:
        policy = PolicyParams(**json.loads(reform_json))
    result = sim.run(policy=policy)
    return result.model_dump()


def get_baseline_params(year: int) -> dict:
    sim = Simulation(year=year)
    return sim.get_baseline_params()


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
    return {str(y): baseline_cache[y] for y in sorted(baseline_cache.keys())}


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

    overlay = _extract_overlay(req)
    if not overlay:
        return baseline_cache.get(req.year, run_simulation(req.year))
    return run_simulation(req.year, json.dumps(overlay))


@app.post("/api/simulate-multi")
async def simulate_multi(req: SimulateMultiYearRequest):
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
        "frs_data": os.path.isdir(CLEAN_FRS_DIR),
    }
