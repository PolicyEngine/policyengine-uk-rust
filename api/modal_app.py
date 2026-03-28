"""
Modal deployment for PolicyEngine UK microsimulation API.

Architecture:
  - Rust binary is compiled at image build time (baked into the image layer).
  - Modal Volume (`policyengine-uk-frs`) holds per-year clean FRS CSVs (1994/-2023/).
    Upload with: python api/upload_frs.py data/frs_clean_all
  - FastAPI app is served via modal.asgi_app().

Deploy:
    modal deploy api/modal_app.py

Serve locally (with hot-reload):
    modal serve api/modal_app.py
"""

import modal

# ---------------------------------------------------------------------------
# Volumes
# ---------------------------------------------------------------------------
frs_volume = modal.Volume.from_name("policyengine-uk-frs", create_if_missing=True)
FRS_MOUNT = "/data/frs_clean"

# ---------------------------------------------------------------------------
# Image — Debian base, install Rust toolchain, clone repo, compile binary
# ---------------------------------------------------------------------------
image = (
    modal.Image.debian_slim(python_version="3.12")
    .apt_install("curl", "build-essential", "pkg-config")
    # Install Rust (stable) without interactive prompts
    .run_commands(
        "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable",
        "echo 'source $HOME/.cargo/env' >> ~/.bashrc",
    )
    .pip_install("fastapi>=0.115", "uvicorn[standard]>=0.30", "pydantic>=2.0")
    # Copy the repo source into the image (exclude FRS data — it stays on the Volume)
    .add_local_dir(".", remote_path="/app", copy=True,
                   ignore=["data/", "target/", ".git/", "app/node_modules/", "app/.next/"])
    .run_commands(
        "cd /app && $HOME/.cargo/bin/cargo build --release 2>&1",
        "cp /app/target/release/policyengine-uk-rust /usr/local/bin/policyengine-uk",
        "chmod +x /usr/local/bin/policyengine-uk",
        # Smoke-test: export params for 2025 (fast, no data needed)
        "policyengine-uk --year 2025 --export-params-json > /dev/null",
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
    import subprocess
    from typing import Any, Optional

    from fastapi import FastAPI, HTTPException
    from fastapi.middleware.cors import CORSMiddleware
    from pydantic import BaseModel

    RUST_BINARY = "policyengine-uk"
    FRS_BASE_DIR = FRS_MOUNT
    AVAILABLE_YEARS = list(range(1994, 2030))

    fastapi_app = FastAPI(title="PolicyEngine UK API")

    fastapi_app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],  # Locked down by Modal's HTTPS — open for GH Pages
        allow_methods=["*"],
        allow_headers=["*"],
    )

    baseline_cache: dict[int, dict] = {}
    params_cache: dict[int, dict] = {}

    def _data_args() -> list[str]:
        if os.path.isdir(FRS_BASE_DIR) and os.listdir(FRS_BASE_DIR):
            return ["--clean-frs-base", FRS_BASE_DIR]
        return []

    def run_simulation(year: int, reform_json: Optional[str] = None) -> dict:
        cmd = [
            RUST_BINARY,
            "--year", str(year),
            "--output", "json",
        ] + _data_args()
        if reform_json:
            cmd += ["--reform-json", reform_json]
        try:
            result = subprocess.run(
                cmd, capture_output=True, text=True, timeout=120,
                cwd="/app",  # binary resolves parameters/ relative to cwd
            )
        except subprocess.TimeoutExpired:
            raise HTTPException(504, detail="Simulation timed out")
        if result.returncode != 0:
            raise HTTPException(500, detail=f"Simulation failed: cmd={cmd} stderr={result.stderr[:2000]}")
        return json.loads(result.stdout)

    def get_baseline_params(year: int) -> dict:
        result = subprocess.run(
            [RUST_BINARY, "--year", str(year), "--export-params-json"],
            capture_output=True, text=True, timeout=10,
            cwd="/app",
        )
        if result.returncode != 0:
            raise HTTPException(500, detail=f"Failed to load params: {result.stderr[:500]}")
        return json.loads(result.stdout)

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
        import shutil
        return {
            "status": "ok",
            "binary": bool(shutil.which("policyengine-uk")),
            "frs_data": os.path.isdir(FRS_BASE_DIR) and bool(os.listdir(FRS_BASE_DIR)),
            "cached_years": sorted(baseline_cache.keys()),
        }

    return fastapi_app


@app.function(
    volumes={FRS_MOUNT: frs_volume},
    # Startup caches 36 year baselines — needs time and memory
    memory=8192,
    cpu=4,
    timeout=600,
    # EU West (Ireland) for lower latency from UK callers
    region="eu-west-1",
)
@modal.concurrent(max_inputs=10)
@modal.asgi_app()
def api():
    return _make_fastapi_app()
