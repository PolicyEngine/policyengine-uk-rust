#!/usr/bin/env python3
"""Validate selected policy scenarios against the PolicyEngine UK Python model."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Metric:
    rust_collection: str
    rust_field: str
    policyengine_variable: str
    tolerance: float = 2.0


METRICS: dict[str, Metric] = {
    "income_tax": Metric("person_results", "income_tax", "income_tax"),
    "national_insurance": Metric(
        "person_results", "national_insurance", "national_insurance"
    ),
    "ni_employer": Metric("person_results", "employer_ni", "ni_employer"),
    "child_benefit": Metric("benunit_results", "child_benefit", "child_benefit"),
    "universal_credit": Metric(
        "benunit_results", "universal_credit", "universal_credit"
    ),
    "scottish_child_payment": Metric(
        "benunit_results", "scottish_child_payment", "scottish_child_payment"
    ),
    "state_pension": Metric("benunit_results", "state_pension", "state_pension"),
    "pension_credit": Metric("benunit_results", "pension_credit", "pension_credit"),
}


def _adult_hours(person: dict[str, Any]) -> float:
    if person.get("hours_worked") is not None:
        return float(person["hours_worked"])
    earned_income = float(person.get("employment_income", 0.0)) + float(
        person.get("self_employment_income", 0.0)
    )
    return 37.5 * 52.0 if earned_income > 0 else 0.0


def build_case(
    *,
    name: str,
    year: int,
    people: list[dict[str, Any]],
    benunit_flags: dict[str, Any] | None = None,
    housing_costs: float = 0.0,
    country: str | None = None,
    metrics: list[str],
) -> dict[str, Any]:
    benunit_flags = benunit_flags or {}

    person_names = [person["name"] for person in people]
    person_ids = {name: idx for idx, name in enumerate(person_names)}

    rust_people = []
    pe_people: dict[str, dict[str, dict[int, Any]]] = {}
    adult_count = 0
    child_count = 0
    is_scotland = country == "SCOTLAND"

    for idx, person in enumerate(people):
        age = float(person["age"])
        if age >= 18:
            adult_count += 1
        else:
            child_count += 1

        rust_person = {
            "id": idx,
            "benunit_id": 0,
            "household_id": 0,
            "age": age,
            "hours_worked": _adult_hours(person),
            "is_in_scotland": is_scotland,
            "is_benunit_head": idx == 0,
            "is_household_head": idx == 0,
        }
        for field in (
            "gender",
            "employment_income",
            "self_employment_income",
            "pension_income",
            "state_pension_reported",
            "savings_interest_income",
            "dividend_income",
            "property_income",
            "is_disabled",
            "is_enhanced_disabled",
            "is_severely_disabled",
            "is_carer",
            "would_claim_marriage_allowance",
        ):
            if field in person:
                rust_person[field] = person[field]
        rust_people.append(rust_person)

        pe_person: dict[str, dict[int, Any]] = {"age": {year: age}}
        for source_field, target_field in (
            ("employment_income", "employment_income"),
            ("self_employment_income", "self_employment_income"),
            ("state_pension_reported", "state_pension_reported"),
            ("would_claim_marriage_allowance", "would_claim_marriage_allowance"),
            ("would_claim_scp", "would_claim_scp"),
        ):
            if source_field in person:
                pe_person[target_field] = {year: person[source_field]}
        if person.get("is_disabled"):
            pe_person["is_disabled_for_benefits"] = {year: True}
        pe_people[person["name"]] = pe_person

    rust_benunit = {
        "id": 0,
        "household_id": 0,
        "person_ids": list(range(len(rust_people))),
        "take_up_seed": float(benunit_flags.get("take_up_seed", 0.0)),
        "rent_monthly": housing_costs / 12.0,
        "is_lone_parent": adult_count == 1 and child_count > 0,
    }
    for field in (
        "on_uc",
        "on_legacy",
        "reported_cb",
        "reported_uc",
        "reported_hb",
        "reported_pc",
        "reported_ctc",
        "reported_wtc",
        "reported_is",
        "is_enr_uc",
        "is_enr_hb",
        "is_enr_pc",
        "is_enr_cb",
        "is_enr_ctc",
        "is_enr_wtc",
    ):
        if field in benunit_flags:
            rust_benunit[field] = benunit_flags[field]

    pe_benunit = {"members": person_names}
    for field in (
        "would_claim_uc",
        "would_claim_child_benefit",
        "would_claim_pc",
    ):
        if field in benunit_flags:
            pe_benunit[field] = {year: benunit_flags[field]}

    rust_household = {
        "id": 0,
        "benunit_ids": [0],
        "person_ids": list(range(len(rust_people))),
        "weight": 1.0,
        "region": "scotland" if is_scotland else "north_east",
        "rent": housing_costs,
        "council_tax": 0.0,
    }

    pe_household: dict[str, Any] = {"members": person_names}
    if housing_costs:
        pe_household["housing_costs"] = {year: housing_costs}
    if country:
        pe_household["country"] = {year: country}

    return {
        "name": name,
        "year": year,
        "metrics": metrics,
        "rust_input": {
            "people": rust_people,
            "benunits": [rust_benunit],
            "households": [rust_household],
        },
        "policyengine_situation": {
            "people": pe_people,
            "benunits": {"benunit": pe_benunit},
            "households": {"household": pe_household},
        },
    }


CASES = [
    build_case(
        name="single_basic_rate_2025",
        year=2025,
        people=[{"name": "person", "age": 30, "employment_income": 30_000}],
        metrics=["income_tax", "national_insurance", "ni_employer"],
    ),
    build_case(
        name="marriage_allowance_couple_2025",
        year=2025,
        people=[
            {
                "name": "transferor",
                "age": 35,
                "employment_income": 5_000,
                "would_claim_marriage_allowance": True,
            },
            {
                "name": "recipient",
                "age": 35,
                "employment_income": 30_000,
                "would_claim_marriage_allowance": True,
            },
        ],
        metrics=["income_tax", "national_insurance"],
    ),
    build_case(
        name="universal_credit_single_2025",
        year=2025,
        people=[{"name": "adult", "age": 30, "employment_income": 0}],
        benunit_flags={
            "on_uc": True,
            "reported_uc": True,
            "would_claim_uc": True,
        },
        metrics=["universal_credit"],
    ),
    build_case(
        name="child_benefit_two_children_2025",
        year=2025,
        people=[
            {"name": "adult", "age": 30, "employment_income": 8_000},
            {"name": "child1", "age": 5},
            {"name": "child2", "age": 3},
        ],
        benunit_flags={
            "would_claim_child_benefit": True,
        },
        metrics=["child_benefit"],
    ),
    build_case(
        name="scottish_child_payment_2025",
        year=2025,
        people=[
            {"name": "adult", "age": 30, "employment_income": 0},
            {"name": "child1", "age": 5, "would_claim_scp": True},
        ],
        benunit_flags={
            "on_uc": True,
            "reported_uc": True,
            "would_claim_uc": True,
        },
        country="SCOTLAND",
        metrics=["scottish_child_payment"],
    ),
    build_case(
        name="scottish_child_payment_2026",
        year=2026,
        people=[
            {"name": "adult", "age": 30, "employment_income": 0},
            {"name": "child1", "age": 5, "would_claim_scp": True},
        ],
        benunit_flags={
            "on_uc": True,
            "reported_uc": True,
            "would_claim_uc": True,
        },
        country="SCOTLAND",
        metrics=["scottish_child_payment"],
    ),
    build_case(
        name="scottish_child_payment_2029",
        year=2029,
        people=[
            {"name": "adult", "age": 30, "employment_income": 0},
            {"name": "child1", "age": 5, "would_claim_scp": True},
        ],
        benunit_flags={
            "on_uc": True,
            "reported_uc": True,
            "would_claim_uc": True,
        },
        country="SCOTLAND",
        metrics=["scottish_child_payment"],
    ),
    build_case(
        name="state_pension_reported_2025",
        year=2025,
        people=[
            {"name": "adult", "age": 70, "state_pension_reported": 5_000},
        ],
        metrics=["state_pension"],
    ),
]


def _add_policyengine_uk_to_path(explicit_path: str | None) -> None:
    candidate = explicit_path or os.environ.get("POLICYENGINE_UK_PATH")
    if candidate:
        sys.path.insert(0, candidate)


def run_rust_case(case: dict[str, Any], rust_binary: Path) -> dict[str, float]:
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False, encoding="utf-8"
    ) as handle:
        json.dump(case["rust_input"], handle)
        scenario_path = Path(handle.name)

    try:
        result = subprocess.run(
            [
                str(rust_binary),
                "--year",
                str(case["year"]),
                "--scenario-json",
                str(scenario_path),
                "--output",
                "json",
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=REPO_ROOT,
        )
    finally:
        scenario_path.unlink(missing_ok=True)

    payload = json.loads(result.stdout)
    values: dict[str, float] = {}
    for metric_name in case["metrics"]:
        metric = METRICS[metric_name]
        values[metric_name] = float(
            sum(item[metric.rust_field] for item in payload[metric.rust_collection])
        )
    return values


def run_policyengine_case(case: dict[str, Any], simulation_cls: Any) -> dict[str, float]:
    sim = simulation_cls(situation=case["policyengine_situation"])
    values: dict[str, float] = {}
    for metric_name in case["metrics"]:
        metric = METRICS[metric_name]
        values[metric_name] = float(sim.calculate(metric.policyengine_variable, case["year"]).sum())
    return values


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--rust-binary",
        type=Path,
        default=REPO_ROOT / "target" / "debug" / "policyengine-uk-rust",
        help="Path to the Rust CLI binary.",
    )
    parser.add_argument(
        "--policyengine-uk-path",
        help="Optional local checkout of policyengine-uk to import instead of the installed package.",
    )
    parser.add_argument(
        "--case",
        action="append",
        dest="cases",
        help="Run only the named validation case. Can be supplied multiple times.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.rust_binary.exists():
        print(f"Rust binary not found at {args.rust_binary}", file=sys.stderr)
        return 1

    _add_policyengine_uk_to_path(args.policyengine_uk_path)
    from policyengine_uk import Simulation  # pylint: disable=import-error

    selected_cases = CASES
    if args.cases:
        wanted = set(args.cases)
        selected_cases = [case for case in CASES if case["name"] in wanted]
        missing = wanted.difference(case["name"] for case in selected_cases)
        if missing:
            print(f"Unknown validation case(s): {', '.join(sorted(missing))}", file=sys.stderr)
            return 1

    failures: list[str] = []
    for case in selected_cases:
        rust_values = run_rust_case(case, args.rust_binary)
        policyengine_values = run_policyengine_case(case, Simulation)
        print(f"[{case['name']}]")
        for metric_name in case["metrics"]:
            metric = METRICS[metric_name]
            rust_value = rust_values[metric_name]
            policy_value = policyengine_values[metric_name]
            diff = abs(rust_value - policy_value)
            print(
                f"  {metric_name}: rust={rust_value:.2f} policyengine={policy_value:.2f} diff={diff:.2f}"
            )
            if diff > metric.tolerance:
                failures.append(
                    f"{case['name']} {metric_name} diff {diff:.2f} exceeds tolerance {metric.tolerance:.2f}"
                )

    if failures:
        print("\nValidation failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(f"\nValidated {len(selected_cases)} case(s) against policyengine-uk.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
