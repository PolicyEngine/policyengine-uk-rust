"""Fetch DWP benefit statistics from the Stat-Xplore API.

Queries UC caseloads by family type and region, PIP claimants, and
benefit cap statistics. Results are cached locally to avoid repeated
API calls.

Requires STAT_XPLORE_API_KEY environment variable to be set.
See: https://stat-xplore.dwp.gov.uk/webapi/online-help/Open-Data-API.html
"""

from __future__ import annotations

import json
import logging
import os
from pathlib import Path

import requests

logger = logging.getLogger(__name__)

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CACHE_DIR = REPO_ROOT / "data" / "cache"
CACHE_FILE = CACHE_DIR / "dwp_stat_xplore.json"

API_BASE = "https://stat-xplore.dwp.gov.uk/webapi/rest/v1"
API_KEY = os.environ.get("STAT_XPLORE_API_KEY", "")


def _headers() -> dict:
    return {"apiKey": API_KEY, "Content-Type": "application/json"}


def _query_table(
    database: str,
    measures: list[str],
    dimensions: list[list[str]],
    recodes: dict | None = None,
) -> dict:
    """Send a table query to stat-xplore and return the JSON response."""
    payload: dict = {
        "database": database,
        "measures": measures,
        "dimensions": dimensions,
    }
    if recodes:
        payload["recodes"] = recodes
    r = requests.post(f"{API_BASE}/table", headers=_headers(), json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


def _fetch_uc_caseloads() -> list[dict]:
    """UC caseloads by family type from stat-xplore."""
    targets = []
    try:
        result = _query_table(
            database="str:database:UC_Monthly",
            measures=["str:count:UC_Monthly:V_F_UC_HOUSEHOLD"],
            dimensions=[
                ["str:field:UC_Monthly:V_F_UC_HOUSEHOLD:FAMILY_TYPE"],
                ["str:field:UC_Monthly:F_UC_DATE:DATE_NAME"],
            ],
        )
        # Extract the latest month's data
        if "cubes" in result:
            cubes = result["cubes"]
            measure_key = list(cubes.keys())[0]
            values = cubes[measure_key]["values"]
            dims = result.get("fields", [])

            # Get family type labels
            family_types = []
            if len(dims) >= 1:
                family_types = [
                    item.get("labels", [""])[0] if isinstance(item, dict) else str(item)
                    for item in dims[0].get("items", [])
                ]

            # Sum across all dates (take latest available)
            if values and family_types:
                latest = [row[-1] if row else 0 for row in values]
                total = sum(v for v in latest if v is not None)
                targets.append(
                    {
                        "name": "dwp/uc_total_households",
                        "variable": "universal_credit",
                        "entity": "person",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "value": float(total),
                        "source": "dwp",
                        "year": 2025,
                        "holdout": False,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch UC caseloads from stat-xplore: %s", e)

    return targets


def _fetch_pip_caseloads() -> list[dict]:
    """PIP caseloads from stat-xplore."""
    targets = []
    try:
        result = _query_table(
            database="str:database:PIP_Monthly",
            measures=["str:count:PIP_Monthly:V_F_PIP_MONTHLY"],
            dimensions=[
                ["str:field:PIP_Monthly:V_F_PIP_MONTHLY:AWARD_TYPE"],
                ["str:field:PIP_Monthly:F_PIP_DATE:DATE_NAME"],
            ],
        )
        if "cubes" in result:
            cubes = result["cubes"]
            measure_key = list(cubes.keys())[0]
            values = cubes[measure_key]["values"]
            if values:
                # Total PIP claimants (sum all award types, latest month)
                total = sum(row[-1] for row in values if row and row[-1] is not None)
                targets.append(
                    {
                        "name": "dwp/pip_total_claimants",
                        "variable": "pip_daily_living",
                        "entity": "person",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "value": float(total),
                        "source": "dwp",
                        "year": 2025,
                        "holdout": False,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch PIP caseloads from stat-xplore: %s", e)

    return targets


def get_targets() -> list[dict]:
    # Try loading from cache first
    if CACHE_FILE.exists():
        logger.info("Using cached DWP targets: %s", CACHE_FILE)
        return json.loads(CACHE_FILE.read_text())

    if not API_KEY:
        logger.warning(
            "STAT_XPLORE_API_KEY not set — skipping DWP targets. "
            "Set the env var and re-run to fetch from stat-xplore."
        )
        return []

    targets = []
    targets.extend(_fetch_uc_caseloads())
    targets.extend(_fetch_pip_caseloads())

    # Cache results
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    CACHE_FILE.write_text(json.dumps(targets, indent=2))
    logger.info("Cached %d DWP targets to %s", len(targets), CACHE_FILE)
    return targets
