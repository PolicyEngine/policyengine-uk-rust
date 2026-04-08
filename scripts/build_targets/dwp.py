"""Fetch DWP benefit statistics from the Stat-Xplore API.

Queries caseloads for UC (with subgroup breakdowns), PIP, pension credit,
carer's allowance, attendance allowance, state pension, ESA, and DLA.
Results are cached locally to avoid repeated API calls.

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
) -> dict:
    """Send a table query to stat-xplore and return the JSON response."""
    payload: dict = {
        "database": database,
        "measures": measures,
        "dimensions": dimensions,
    }
    r = requests.post(f"{API_BASE}/table", headers=_headers(), json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


def _extract_year(result: dict) -> int:
    """Extract the year from the auto-selected date field."""
    for field in result.get("fields", []):
        for item in field.get("items", []):
            for label in item.get("labels", []):
                for part in str(label).replace("-", " ").split():
                    if part.isdigit():
                        y = int(part)
                        return y if y > 100 else 2000 + y
    return 2025


def _extract_total(result: dict) -> float | None:
    """Extract the single value from a no-dimension query."""
    cubes = result.get("cubes", {})
    if not cubes:
        return None
    values = next(iter(cubes.values()))["values"]
    # Unwrap nested lists (stat-xplore wraps in [date][value])
    while isinstance(values, list) and len(values) == 1:
        values = values[0]
    return values if isinstance(values, (int, float)) else None


def _extract_breakdown(result: dict) -> list[tuple[str, float]]:
    """Extract label/value pairs from a single-dimension query.

    Stat-xplore auto-adds the date dimension, so the response has two fields:
    date (1 item = latest month) and the requested dimension (N items).
    Values are shaped [1][N].
    """
    fields = result.get("fields", [])
    cubes = result.get("cubes", {})
    if not cubes:
        return []
    vals = next(iter(cubes.values()))["values"]

    # Find the non-date dimension
    dim_field = None
    for f in fields:
        if "month" not in f["label"].lower() and "date" not in f["label"].lower():
            dim_field = f
            break
    if dim_field is None:
        return []

    items = dim_field["items"]
    # Values are [date_idx][dim_idx] — take last date row
    row = vals[-1] if isinstance(vals[0], list) else vals
    pairs = []
    for i, item in enumerate(items):
        v = row[i] if isinstance(row, list) else row
        if v is not None and v > 0:
            pairs.append((item["labels"][0], float(v)))
    return pairs


# ── Simple total caseload queries ──────────────────────────────────────────


# (database, measure, target_name, survey_variable, entity)
_SIMPLE_BENEFITS = [
    (
        "str:database:UC_Monthly",
        "str:count:UC_Monthly:V_F_UC_CASELOAD_FULL",
        "dwp/uc_total_claimants",
        "universal_credit",
        "person",
    ),
    (
        "str:database:PIP_Monthly_new",
        "str:count:PIP_Monthly_new:V_F_PIP_MONTHLY",
        "dwp/pip_total_claimants",
        "pip_daily_living",
        "person",
    ),
    (
        "str:database:PC_New",
        "str:count:PC_New:V_F_PC_CASELOAD_New",
        "dwp/pension_credit_claimants",
        "pension_credit",
        "person",
    ),
    (
        "str:database:CA_In_Payment_New",
        "str:count:CA_In_Payment_New:V_F_CA_In_Payment_New",
        "dwp/carers_allowance_claimants",
        "carers_allowance",
        "person",
    ),
    (
        "str:database:AA_In_Payment_New",
        "str:count:AA_In_Payment_New:V_F_AA_In_Payment_New",
        "dwp/attendance_allowance_claimants",
        "attendance_allowance",
        "person",
    ),
    (
        "str:database:SP_New",
        "str:count:SP_New:V_F_SP_CASELOAD_New",
        "dwp/state_pension_claimants",
        "state_pension",
        "person",
    ),
    (
        "str:database:ESA_Caseload_new",
        "str:count:ESA_Caseload_new:V_F_ESA_NEW",
        "dwp/esa_claimants",
        "esa_income",
        "person",
    ),
    (
        "str:database:DLA_In_Payment_New",
        "str:count:DLA_In_Payment_New:V_F_DLA_In_Payment_New",
        "dwp/dla_claimants",
        "dla_care",
        "person",
    ),
]


def _fetch_simple_benefits() -> list[dict]:
    """Fetch total caseload for each benefit."""
    targets = []
    for database, measure, name, variable, entity in _SIMPLE_BENEFITS:
        try:
            result = _query_table(database, [measure], [])
            total = _extract_total(result)
            if total is not None:
                year = _extract_year(result)
                targets.append(
                    {
                        "name": name,
                        "variable": variable,
                        "entity": entity,
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "value": total,
                        "source": "dwp",
                        "year": year,
                        "holdout": False,
                    }
                )
        except Exception as e:
            logger.warning("Failed to fetch %s: %s", name, e)
    return targets


# ── UC subgroup breakdowns (households) ────────────────────────────────────

_UC_HH_DB = "str:database:UC_Households"
_UC_HH_COUNT = "str:count:UC_Households:V_F_UC_HOUSEHOLDS"
_UC_HH_FIELD = "str:field:UC_Households:V_F_UC_HOUSEHOLDS"


def _fetch_uc_breakdowns() -> list[dict]:
    """Fetch UC household breakdowns by family type, entitlement elements, etc."""
    targets = []

    # UC households by family type — map to benunit_filter conditions
    try:
        result = _query_table(
            _UC_HH_DB,
            [_UC_HH_COUNT],
            [[f"{_UC_HH_FIELD}:hnfamily_type"]],
        )
        year = _extract_year(result)
        for label, value in _extract_breakdown(result):
            slug = label.lower().replace(",", "").replace(" ", "_")
            if "unknown" in slug or "missing" in slug:
                continue
            # Map family type labels to benunit filter conditions
            bf = {}
            if "single" in slug and "no_child" in slug:
                bf = {"is_couple": False, "has_children": False}
            elif "single" in slug and "child" in slug:
                bf = {"is_couple": False, "has_children": True}
            elif "couple" in slug and "no_child" in slug:
                bf = {"is_couple": True, "has_children": False}
            elif "couple" in slug and "child" in slug:
                bf = {"is_couple": True, "has_children": True}

            targets.append(
                {
                    "name": f"dwp/uc_households_{slug}",
                    "variable": "universal_credit",
                    "entity": "benunit",
                    "aggregation": "count_nonzero",
                    "filter": None,
                    "benunit_filter": bf if bf else None,
                    "value": value,
                    "source": "dwp",
                    "year": year,
                    "holdout": True,
                }
            )
    except Exception as e:
        logger.warning("Failed to fetch UC family type breakdown: %s", e)

    # UC households with child entitlement
    try:
        result = _query_table(
            _UC_HH_DB,
            [_UC_HH_COUNT],
            [[f"{_UC_HH_FIELD}:HCCHILD_ENTITLEMENT"]],
        )
        year = _extract_year(result)
        for label, value in _extract_breakdown(result):
            if label.lower() == "yes":
                targets.append(
                    {
                        "name": "dwp/uc_households_with_children",
                        "variable": "universal_credit",
                        "entity": "benunit",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "benunit_filter": {"has_children": True},
                        "value": value,
                        "source": "dwp",
                        "year": year,
                        "holdout": False,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch UC child entitlement breakdown: %s", e)

    # UC households with LCWRA entitlement (disability element)
    try:
        result = _query_table(
            _UC_HH_DB,
            [_UC_HH_COUNT],
            [[f"{_UC_HH_FIELD}:HCLCW_ENTITLEMENT"]],
        )
        year = _extract_year(result)
        for label, value in _extract_breakdown(result):
            slug = label.lower().replace(" ", "_").replace("/", "_")
            if slug == "lcwra":
                targets.append(
                    {
                        "name": "dwp/uc_households_lcwra",
                        "variable": "universal_credit",
                        "entity": "benunit",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "benunit_filter": {"has_lcwra": True},
                        "value": value,
                        "source": "dwp",
                        "year": year,
                        "holdout": False,
                    }
                )
            elif slug == "lcw":
                targets.append(
                    {
                        "name": "dwp/uc_households_lcw",
                        "variable": "universal_credit",
                        "entity": "benunit",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "benunit_filter": {"has_lcw": True},
                        "value": value,
                        "source": "dwp",
                        "year": year,
                        "holdout": True,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch UC LCW breakdown: %s", e)

    # UC households with carer entitlement
    try:
        result = _query_table(
            _UC_HH_DB,
            [_UC_HH_COUNT],
            [[f"{_UC_HH_FIELD}:HCCARER_ENTITLEMENT"]],
        )
        year = _extract_year(result)
        for label, value in _extract_breakdown(result):
            if label.lower() == "yes":
                targets.append(
                    {
                        "name": "dwp/uc_households_with_carer",
                        "variable": "universal_credit",
                        "entity": "benunit",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "benunit_filter": {"has_carer": True},
                        "value": value,
                        "source": "dwp",
                        "year": year,
                        "holdout": True,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch UC carer breakdown: %s", e)

    # UC households with housing entitlement
    try:
        result = _query_table(
            _UC_HH_DB,
            [_UC_HH_COUNT],
            [[f"{_UC_HH_FIELD}:TENURE"]],
        )
        year = _extract_year(result)
        for label, value in _extract_breakdown(result):
            if label.lower() == "yes":
                targets.append(
                    {
                        "name": "dwp/uc_households_with_housing",
                        "variable": "universal_credit",
                        "entity": "benunit",
                        "aggregation": "count_nonzero",
                        "filter": None,
                        "benunit_filter": {"has_housing": True},
                        "value": value,
                        "source": "dwp",
                        "year": year,
                        "holdout": False,
                    }
                )
    except Exception as e:
        logger.warning("Failed to fetch UC housing breakdown: %s", e)

    return targets


def get_targets() -> list[dict]:
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
    targets.extend(_fetch_simple_benefits())
    targets.extend(_fetch_uc_breakdowns())

    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    CACHE_FILE.write_text(json.dumps(targets, indent=2))
    logger.info("Cached %d DWP targets to %s", len(targets), CACHE_FILE)
    return targets
