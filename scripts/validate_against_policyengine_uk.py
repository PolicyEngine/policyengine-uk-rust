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


@dataclass(frozen=True)
class CaseMetric:
    name: str
    reducer: str = "sum"


METRICS: dict[str, Metric] = {
    "income_tax": Metric("person_results", "income_tax", "income_tax"),
    "national_insurance": Metric(
        "person_results", "national_insurance", "national_insurance"
    ),
    "ni_employer": Metric("person_results", "employer_ni", "ni_employer"),
    "child_benefit": Metric("benunit_results", "child_benefit", "child_benefit"),
    "housing_benefit": Metric(
        "benunit_results", "housing_benefit", "housing_benefit"
    ),
    "child_tax_credit": Metric(
        "benunit_results", "child_tax_credit", "child_tax_credit"
    ),
    "working_tax_credit": Metric(
        "benunit_results", "working_tax_credit", "working_tax_credit"
    ),
    "income_support": Metric(
        "benunit_results", "income_support", "income_support"
    ),
    "universal_credit": Metric(
        "benunit_results", "universal_credit", "universal_credit"
    ),
    "scottish_child_payment": Metric(
        "benunit_results", "scottish_child_payment", "scottish_child_payment"
    ),
    "state_pension": Metric("benunit_results", "state_pension", "state_pension"),
    "pension_credit": Metric("benunit_results", "pension_credit", "pension_credit"),
    "benefit_cap_reduction": Metric(
        "benunit_results", "benefit_cap_reduction", "benefit_cap_reduction"
    ),
}


def _adult_hours(person: dict[str, Any]) -> float:
    if person.get("hours_worked") is not None:
        return float(person["hours_worked"])
    if person.get("weekly_hours") is not None:
        return float(person["weekly_hours"]) * 52.0
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
    region: str | None = None,
    metrics: list[str | CaseMetric],
    rust_reform: dict[str, Any] | None = None,
    policyengine_scenario: dict[str, Any] | None = None,
    known_failure: str | None = None,
    tags: list[str] | None = None,
) -> dict[str, Any]:
    benunit_flags = benunit_flags or {}
    tags = tags or []

    person_names = [person["name"] for person in people]
    person_ids = {name: idx for idx, name in enumerate(person_names)}

    rust_people = []
    pe_people: dict[str, dict[str, dict[int, Any]]] = {}
    adult_count = 0
    child_count = 0
    household_region = region or ("SCOTLAND" if country == "SCOTLAND" else "NORTH_EAST")
    is_scotland = country == "SCOTLAND" or household_region == "SCOTLAND"

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
            "child_benefit_reported",
            "housing_benefit_reported",
            "income_support_reported",
            "pension_credit_reported",
            "child_tax_credit_reported",
            "working_tax_credit_reported",
            "universal_credit_reported",
            "is_disabled",
            "is_enhanced_disabled",
            "is_severely_disabled",
            "is_carer",
            "would_claim_marriage_allowance",
        ):
            if field in person:
                rust_person[field] = person[field]
        rust_people.append(rust_person)

        weekly_hours = person.get("weekly_hours")
        if weekly_hours is None:
            weekly_hours = rust_person["hours_worked"] / 52.0

        pe_person: dict[str, dict[int, Any]] = {
            "age": {year: age},
            "weekly_hours": {year: weekly_hours},
        }
        for source_field, target_field in (
            ("employment_income", "employment_income"),
            ("self_employment_income", "self_employment_income"),
            ("state_pension_reported", "state_pension_reported"),
            ("child_benefit_reported", "child_benefit_reported"),
            ("housing_benefit_reported", "housing_benefit_reported"),
            ("income_support_reported", "income_support_reported"),
            ("pension_credit_reported", "pension_credit_reported"),
            ("child_tax_credit_reported", "child_tax_credit_reported"),
            ("working_tax_credit_reported", "working_tax_credit_reported"),
            ("universal_credit_reported", "universal_credit_reported"),
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
        "would_claim_housing_benefit",
        "would_claim_CTC",
        "would_claim_WTC",
        "would_claim_IS",
    ):
        if field in benunit_flags:
            pe_benunit[field] = {year: benunit_flags[field]}

    rust_household = {
        "id": 0,
        "benunit_ids": [0],
        "person_ids": list(range(len(rust_people))),
        "weight": 1.0,
        "region": household_region.lower(),
        "rent": housing_costs,
        "council_tax": 0.0,
    }

    pe_household: dict[str, Any] = {"members": person_names}
    if housing_costs:
        pe_household["housing_costs"] = {year: housing_costs}
    pe_household["region"] = {year: household_region}
    if country:
        pe_household["country"] = {year: country}

    return {
        "name": name,
        "year": year,
        "known_failure": known_failure,
        "tags": tags,
        "metrics": [
            metric if isinstance(metric, CaseMetric) else CaseMetric(metric)
            for metric in metrics
        ],
        "entity_labels": {
            "person_results": person_names,
            "benunit_results": ["benunit"],
            "household_results": ["household"],
        },
        "rust_input": {
            "people": rust_people,
            "benunits": [rust_benunit],
            "households": [rust_household],
        },
        "rust_reform": rust_reform,
        "policyengine_situation": {
            "people": pe_people,
            "benunits": {"benunit": pe_benunit},
            "households": {"household": pe_household},
        },
        "policyengine_scenario": policyengine_scenario,
    }


CASES = [
    build_case(
        name="single_basic_rate_2025",
        year=2025,
        people=[{"name": "person", "age": 30, "employment_income": 30_000}],
        tags=["baseline", "tax"],
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
        tags=["baseline", "tax", "sequence"],
        metrics=[
            CaseMetric("income_tax", reducer="sequence"),
            CaseMetric("national_insurance", reducer="sequence"),
        ],
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
        tags=["baseline", "uc"],
        metrics=["universal_credit"],
    ),
    build_case(
        name="income_tax_basic_rate_reform_2025",
        year=2025,
        people=[{"name": "person", "age": 30, "employment_income": 30_000}],
        tags=["reform", "tax"],
        rust_reform={
            "income_tax": {
                "uk_brackets": [
                    {"rate": 0.25, "threshold": 0.0},
                    {"rate": 0.40, "threshold": 37700.0},
                    {"rate": 0.45, "threshold": 125140.0},
                ]
            }
        },
        policyengine_scenario={"gov.hmrc.income_tax.rates.uk[0].rate": 0.25},
        metrics=["income_tax"],
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
        tags=["baseline", "child_benefit"],
        metrics=["child_benefit"],
    ),
    build_case(
        name="legacy_tax_credits_lone_parent_2025",
        year=2025,
        people=[
            {
                "name": "adult",
                "age": 30,
                "employment_income": 15_000,
                "weekly_hours": 35,
                "child_tax_credit_reported": 1.0,
                "working_tax_credit_reported": 1.0,
            },
            {"name": "child", "age": 5},
        ],
        benunit_flags={
            "take_up_seed": 0.99,
            "on_legacy": True,
            "reported_ctc": True,
            "reported_wtc": True,
            "would_claim_uc": False,
            "would_claim_child_benefit": True,
            "would_claim_CTC": True,
            "would_claim_WTC": True,
        },
        tags=["baseline", "legacy", "tax_credits"],
        metrics=["child_tax_credit", "working_tax_credit"],
    ),
    build_case(
        name="legacy_income_support_lone_parent_2025",
        year=2025,
        people=[
            {
                "name": "adult",
                "age": 30,
                "employment_income": 0,
                "income_support_reported": 1.0,
            },
            {"name": "child1", "age": 5},
            {"name": "child2", "age": 4},
        ],
        benunit_flags={
            "take_up_seed": 0.99,
            "on_legacy": True,
            "reported_is": True,
            "would_claim_uc": False,
            "would_claim_child_benefit": True,
            "would_claim_IS": True,
        },
        tags=["baseline", "legacy", "income_support"],
        metrics=["income_support"],
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
        tags=["baseline", "scp"],
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
        tags=["baseline", "scp"],
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
        tags=["baseline", "scp"],
        metrics=["scottish_child_payment"],
    ),
    build_case(
        name="state_pension_reported_2025",
        year=2025,
        people=[
            {"name": "adult", "age": 70, "state_pension_reported": 5_000},
        ],
        tags=["baseline", "pension"],
        metrics=["state_pension"],
    ),
    build_case(
        name="pension_credit_single_2025",
        year=2025,
        people=[
            {"name": "adult", "age": 75, "state_pension_reported": 5_000},
        ],
        benunit_flags={
            "reported_pc": True,
            "would_claim_pc": True,
        },
        tags=["baseline", "pension"],
        metrics=["pension_credit"],
    ),
]


def _add_policyengine_uk_to_path(explicit_path: str | None) -> None:
    candidate = explicit_path or os.environ.get("POLICYENGINE_UK_PATH")
    if candidate:
        sys.path.insert(0, candidate)


def _sequence_labels(case: dict[str, Any], metric: Metric, length: int) -> list[str]:
    labels = case["entity_labels"].get(metric.rust_collection, [])
    if len(labels) == length:
        return labels
    return [f"item_{index}" for index in range(length)]


def _format_sequence(values: list[float], labels: list[str]) -> str:
    pairs = [f"{label}={value:.2f}" for label, value in zip(labels, values)]
    return "[" + ", ".join(pairs) + "]"


def run_rust_case(case: dict[str, Any], rust_binary: Path) -> dict[str, float]:
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False, encoding="utf-8"
    ) as handle:
        json.dump(case["rust_input"], handle)
        scenario_path = Path(handle.name)

    try:
        cmd = [
            str(rust_binary),
            "--year",
            str(case["year"]),
            "--scenario-json",
            str(scenario_path),
            "--output",
            "json",
        ]
        if case.get("rust_reform") is not None:
            cmd.extend(["--reform-json", json.dumps(case["rust_reform"])])

        result = subprocess.run(
            cmd,
            check=True,
            capture_output=True,
            text=True,
            cwd=REPO_ROOT,
        )
    finally:
        scenario_path.unlink(missing_ok=True)

    payload = json.loads(result.stdout)
    values: dict[str, float | list[float]] = {}
    for case_metric in case["metrics"]:
        metric = METRICS[case_metric.name]
        raw_values = [
            float(item[metric.rust_field]) for item in payload[metric.rust_collection]
        ]
        if case_metric.reducer == "sequence":
            values[case_metric.name] = raw_values
        else:
            values[case_metric.name] = float(sum(raw_values))
    return values


def run_policyengine_case(
    case: dict[str, Any], simulation_cls: Any, scenario_cls: Any
) -> dict[str, float | list[float]]:
    scenario = None
    if case.get("policyengine_scenario") is not None:
        scenario = scenario_cls(parameter_changes=case["policyengine_scenario"])

    sim = simulation_cls(situation=case["policyengine_situation"], scenario=scenario)
    values: dict[str, float | list[float]] = {}
    for case_metric in case["metrics"]:
        metric = METRICS[case_metric.name]
        result = sim.calculate(metric.policyengine_variable, case["year"])
        if case_metric.reducer == "sequence":
            values[case_metric.name] = [float(value) for value in result]
        else:
            values[case_metric.name] = float(result.sum())
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
    parser.add_argument(
        "--tag",
        action="append",
        dest="tags",
        help="Run only cases containing all supplied tags. Can be supplied multiple times.",
    )
    parser.add_argument(
        "--list-cases",
        action="store_true",
        help="List available validation cases and exit.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.rust_binary.exists():
        print(f"Rust binary not found at {args.rust_binary}", file=sys.stderr)
        return 1

    if args.list_cases:
        for case in CASES:
            suffix = " [expected-failure]" if case.get("known_failure") else ""
            tags = ",".join(case.get("tags", []))
            tag_suffix = f" [{tags}]" if tags else ""
            print(f"{case['name']}{suffix}{tag_suffix}")
        return 0

    _add_policyengine_uk_to_path(args.policyengine_uk_path)
    from policyengine_uk import Scenario, Simulation  # pylint: disable=import-error

    selected_cases = CASES
    if args.cases:
        wanted = set(args.cases)
        selected_cases = [case for case in CASES if case["name"] in wanted]
        missing = wanted.difference(case["name"] for case in selected_cases)
        if missing:
            print(f"Unknown validation case(s): {', '.join(sorted(missing))}", file=sys.stderr)
            return 1
    if args.tags:
        required_tags = set(args.tags)
        selected_cases = [
            case
            for case in selected_cases
            if required_tags.issubset(set(case.get("tags", [])))
        ]
        if not selected_cases:
            print(
                f"No validation cases matched tag filter: {', '.join(sorted(required_tags))}",
                file=sys.stderr,
            )
            return 1

    failures: list[str] = []
    expected_failures: list[str] = []
    unexpected_passes: list[str] = []
    for case in selected_cases:
        rust_values = run_rust_case(case, args.rust_binary)
        policyengine_values = run_policyengine_case(case, Simulation, Scenario)
        print(f"[{case['name']}]")
        case_failures: list[str] = []
        for case_metric in case["metrics"]:
            metric = METRICS[case_metric.name]
            rust_value = rust_values[case_metric.name]
            policy_value = policyengine_values[case_metric.name]
            if isinstance(rust_value, list):
                labels = _sequence_labels(case, metric, len(rust_value))
                if len(rust_value) != len(policy_value):
                    case_failures.append(
                        f"{case['name']} {case_metric.name} length mismatch {len(rust_value)} != {len(policy_value)}"
                    )
                    print(
                        f"  {case_metric.name}: rust={rust_value} policyengine={policy_value} diff=length-mismatch"
                    )
                    continue
                diffs = [abs(a - b) for a, b in zip(rust_value, policy_value)]
                print(
                    "  "
                    f"{case_metric.name}: "
                    f"rust={_format_sequence(rust_value, labels)} "
                    f"policyengine={_format_sequence(policy_value, labels)} "
                    f"diffs={[round(diff, 2) for diff in diffs]}"
                )
                for index, diff in enumerate(diffs):
                    if diff > metric.tolerance:
                        case_failures.append(
                            f"{case['name']} {case_metric.name}[{labels[index]}] diff {diff:.2f} exceeds tolerance {metric.tolerance:.2f}"
                        )
            else:
                diff = abs(rust_value - policy_value)
                print(
                    f"  {case_metric.name}: rust={rust_value:.2f} policyengine={policy_value:.2f} diff={diff:.2f}"
                )
                if diff > metric.tolerance:
                    case_failures.append(
                        f"{case['name']} {case_metric.name} diff {diff:.2f} exceeds tolerance {metric.tolerance:.2f}"
                    )

        if case.get("known_failure"):
            if case_failures:
                expected_failures.append(case["name"])
                print(f"  expected failure: {case['known_failure']}")
            else:
                unexpected_passes.append(
                    f"{case['name']} unexpectedly passed; review and remove known_failure"
                )
        else:
            failures.extend(case_failures)

    if failures or unexpected_passes:
        print("\nValidation failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        for unexpected_pass in unexpected_passes:
            print(f"  - {unexpected_pass}", file=sys.stderr)
        return 1

    print(
        "\nValidated "
        f"{len(selected_cases) - len(expected_failures)} passing case(s) "
        f"against policyengine-uk with {len(expected_failures)} expected failure(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
