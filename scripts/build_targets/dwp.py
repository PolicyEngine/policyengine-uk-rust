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


def _extract_total(result: dict) -> float | None:
    """Extract the single value from a no-dimension stat-xplore query."""
    cubes = result.get("cubes", {})
    if not cubes:
        return None
    values = next(iter(cubes.values()))["values"]
    # With no explicit dimensions, stat-xplore returns the latest month
    # as a single-element list
    if isinstance(values, list) and len(values) == 1:
        return values[0]
    return values if isinstance(values, (int, float)) else None


def _extract_year(result: dict) -> int:
    """Extract the year from the auto-selected date field."""
    for field in result.get("fields", []):
        for item in field.get("items", []):
            for label in item.get("labels", []):
                # Labels like "February 2026" or "Jan-26"
                for part in str(label).replace("-", " ").split():
                    if part.isdigit():
                        y = int(part)
                        return y if y > 100 else 2000 + y
    return 2025


def _fetch_uc_caseloads() -> list[dict]:
    """Total UC claimants (people) from stat-xplore."""
    targets = []
    try:
        result = _query_table(
            database="str:database:UC_Monthly",
            measures=["str:count:UC_Monthly:V_F_UC_CASELOAD_FULL"],
            dimensions=[],
        )
        total = _extract_total(result)
        if total is not None:
            year = _extract_year(result)
            targets.append(
                {
                    "name": "dwp/uc_total_claimants",
                    "variable": "universal_credit",
                    "entity": "person",
                    "aggregation": "count_nonzero",
                    "filter": None,
                    "value": float(total),
                    "source": "dwp",
                    "year": year,
                    "holdout": False,
                }
            )
    except Exception as e:
        logger.warning("Failed to fetch UC caseloads from stat-xplore: %s", e)

    return targets


def _fetch_pip_caseloads() -> list[dict]:
    """Total PIP claimants from stat-xplore (post-2019 database)."""
    targets = []
    try:
        result = _query_table(
            database="str:database:PIP_Monthly_new",
            measures=["str:count:PIP_Monthly_new:V_F_PIP_MONTHLY"],
            dimensions=[],
        )
        total = _extract_total(result)
        if total is not None:
            year = _extract_year(result)
            targets.append(
                {
                    "name": "dwp/pip_total_claimants",
                    "variable": "pip_daily_living",
                    "entity": "person",
                    "aggregation": "count_nonzero",
                    "filter": None,
                    "value": float(total),
                    "source": "dwp",
                    "year": year,
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
