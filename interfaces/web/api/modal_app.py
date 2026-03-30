"""
Modal deployment for PolicyEngine UK microsimulation API.

Architecture:
  - Rust binary is compiled at image build time and bundled into the Python package.
  - Modal Volume (`policyengine-uk-frs`) holds per-year clean FRS CSVs (1994/-2023/).
    Upload with: python interfaces/web/api/upload_frs.py data/frs
  - FastAPI app is served via modal.asgi_app().

Deploy:
    modal deploy interfaces/web/api/modal_app.py

Serve locally (with hot-reload):
    modal serve interfaces/web/api/modal_app.py
"""

import modal

# ---------------------------------------------------------------------------
# Volumes
# ---------------------------------------------------------------------------
frs_volume = modal.Volume.from_name("policyengine-uk-frs", create_if_missing=True)
FRS_MOUNT = "/data/frs"

# ---------------------------------------------------------------------------
# Image — Debian base, install Rust toolchain, clone repo, compile, install package
# ---------------------------------------------------------------------------
image = (
    modal.Image.debian_slim(python_version="3.12")
    .apt_install("curl", "build-essential", "pkg-config")
    .run_commands(
        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable",
        "echo 'source $HOME/.cargo/env' >> ~/.bashrc",
    )
    .pip_install("fastapi>=0.115", "uvicorn[standard]>=0.30", "pydantic>=2.0", "build", "wheel")
    .add_local_dir(".", remote_path="/app", copy=True,
                   ignore=["data/", "target/", ".git/", "interfaces/web/app/node_modules/", "interfaces/web/app/.next/"])
    .run_commands(
        # Build binary, stage into package, install
        "cd /app && $HOME/.cargo/bin/cargo build --release 2>&1",
        "cd /app && bash interfaces/python/build_package.sh",
        "cd /app && pip install .",
        # Smoke-test
        "python -c 'from policyengine_uk_compiled import Simulation; s = Simulation(year=2025); print(s.get_baseline_params()[\"fiscal_year\"])'",
    )
)

# ---------------------------------------------------------------------------
# Modal App
# ---------------------------------------------------------------------------
app = modal.App("policyengine-uk", image=image)


# ---------------------------------------------------------------------------
# FastAPI application
# ---------------------------------------------------------------------------
def _make_fastapi_app():
    import json
    import os
    from typing import Any, Optional

    from fastapi import FastAPI, HTTPException
    from fastapi.middleware.cors import CORSMiddleware
    from pydantic import BaseModel

    from policyengine_uk_compiled import Simulation, Parameters as PolicyParams

    FRS_BASE_DIR = FRS_MOUNT
    AVAILABLE_YEARS = list(range(1994, 2030))

    fastapi_app = FastAPI(title="PolicyEngine UK API")

    fastapi_app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
    )

    baseline_cache: dict[int, dict] = {}
    params_cache: dict[int, dict] = {}

    def _data_kwargs() -> dict:
        if os.path.isdir(FRS_BASE_DIR) and os.listdir(FRS_BASE_DIR):
            return {"clean_frs_base": FRS_BASE_DIR}
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

    @fastapi_app.on_event("startup")
    async def cache_baselines():
        for year in AVAILABLE_YEARS:
            try:
                baseline_cache[year] = run_simulation(year)
                params_cache[year] = get_baseline_params(year)
                print(f"  Cached baseline for {year}/{year + 1}")
            except Exception as e:
                print(f"  Warning: Failed to cache {year}/{year + 1}: {e}")

    REFORM_SECTIONS = [
        "income_tax", "national_insurance", "universal_credit",
        "child_benefit", "benefit_cap", "housing_benefit",
        "tax_credits", "council_tax_reduction", "scottish_child_payment",
        "pension_credit", "state_pension",
    ]

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

    def _extract_overlay(req) -> dict[str, Any]:
        overlay: dict[str, Any] = {}
        for section in REFORM_SECTIONS:
            val = getattr(req, section, None)
            if val:
                overlay[section] = val
        return overlay

    @fastapi_app.get("/api/baseline/{year}")
    async def get_baseline(year: int):
        if year not in baseline_cache:
            raise HTTPException(404, detail=f"Year {year} not available")
        return baseline_cache[year]

    @fastapi_app.get("/api/baselines")
    async def get_all_baselines():
        return {str(y): baseline_cache[y] for y in sorted(baseline_cache.keys())}

    @fastapi_app.get("/api/parameters/{year}")
    async def get_parameters(year: int):
        if year not in params_cache:
            raise HTTPException(404, detail=f"Year {year} not available")
        return params_cache[year]

    @fastapi_app.get("/api/years")
    async def get_years():
        return {"years": AVAILABLE_YEARS}

    @fastapi_app.post("/api/simulate")
    async def simulate(req: SimulateRequest):
        if req.year not in AVAILABLE_YEARS:
            raise HTTPException(400, detail=f"Year {req.year} not available")
        overlay = _extract_overlay(req)
        if not overlay:
            return baseline_cache.get(req.year, run_simulation(req.year))
        return run_simulation(req.year, json.dumps(overlay))

    @fastapi_app.post("/api/simulate-multi")
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

    @fastapi_app.get("/api/health")
    async def health():
        return {
            "status": "ok",
            "frs_data": os.path.isdir(FRS_BASE_DIR) and bool(os.listdir(FRS_BASE_DIR)),
            "cached_years": sorted(baseline_cache.keys()),
        }

    return fastapi_app


@app.function(
    volumes={FRS_MOUNT: frs_volume},
    memory=8192,
    cpu=4,
    timeout=600,
    region="eu-west-1",
)
@modal.concurrent(max_inputs=10)
@modal.asgi_app()
def api():
    return _make_fastapi_app()
