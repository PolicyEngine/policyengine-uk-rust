"""Structural reform hooks and Python-side aggregation.

A StructuralReform holds two optional callables:

    pre(year, persons, benunits, households) -> (persons, benunits, households)
        Runs before the Rust binary sees the data.  Use to mutate input
        columns — add a new income source, change household composition,
        set benefit eligibility flags, etc.

    post(year, persons, benunits, households) -> (persons, benunits, households)
        Runs after the binary produces microdata output.  All
        baseline_*/reform_* columns are populated at this point.  Use to
        apply a new tax on top of simulated results, offset a benefit,
        impose a cap, etc.  Aggregation is then done in Python rather than
        by the binary.

Both hooks receive and must return all three DataFrames even if only one is
modified, so the caller can always unpack a consistent triple.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    import pandas as pd

# Type alias for the hook signature
HookFn = Callable[
    [int, "pd.DataFrame", "pd.DataFrame", "pd.DataFrame"],
    tuple["pd.DataFrame", "pd.DataFrame", "pd.DataFrame"],
]


@dataclass
class StructuralReform:
    """Container for pre- and post-simulation structural reform hooks.

    Both hooks are optional.  Omit whichever you don't need.

    Hook signature (same for pre and post):

        def hook(
            year: int,
            persons: pd.DataFrame,
            benunits: pd.DataFrame,
            households: pd.DataFrame,
        ) -> tuple[pd.DataFrame, pd.DataFrame, pd.DataFrame]:
            ...
            return persons, benunits, households

    Example — add a £50/wk UBI to every adult's reform net income::

        def ubi_post(year, persons, benunits, households):
            ubi_annual = 50 * 52
            mask = persons["age"] >= 18
            persons.loc[mask, "reform_income_tax"] = 0  # illustrative
            households["reform_net_income"] += ubi_annual  # per-household
            return persons, benunits, households

        reform = StructuralReform(post=ubi_post)

    Example — replace employment income with a flat wage in 2025 only::

        def flat_wage_pre(year, persons, benunits, households):
            if year == 2025:
                persons["employment_income"] = persons["employment_income"].clip(upper=50_000)
            return persons, benunits, households

        reform = StructuralReform(pre=flat_wage_pre)
    """

    pre: Optional[HookFn] = field(default=None)
    post: Optional[HookFn] = field(default=None)


# ── Python-side aggregation ───────────────────────────────────────────────────
#
# Used whenever a post-hook is present (or for persons-only datasets).
# Reads the microdata columns produced by the Rust binary and aggregates
# them into a SimulationResult.  Column names mirror write_microdata_csv_* in
# src/data/clean.rs.


def aggregate_microdata(
    persons: "pd.DataFrame",
    benunits: "pd.DataFrame",
    households: "pd.DataFrame",
    year: int,
) -> "SimulationResult":  # noqa: F821 – imported lazily to avoid circular import
    """Aggregate post-simulation microdata DataFrames into a SimulationResult.

    This mirrors the aggregation logic in src/main.rs but runs in Python,
    allowing post-hooks to modify result columns before the final roll-up.

    Deciles and winners/losers are based on reform_net_income (equivalised by
    equivalisation_factor where available).  This approximates the Rust engine's
    use of extended_net_income; the difference only matters for VAT/stamp duty/
    wealth-tax reforms, which are unlikely to be applied as post-hooks.
    """
    import numpy as np
    from policyengine_uk_compiled.models import (
        BudgetaryImpact, IncomeBreakdown, ProgramBreakdown, Caseloads,
        DecileImpact, WinnersLosers, SimulationResult,
        HbaiIncomes, PovertyHeadcounts,
    )

    w = households["weight"].values

    # ── Budgetary impact ──────────────────────────────────────────────────────
    baseline_revenue = (w * households["baseline_total_tax"].values).sum()
    reform_revenue   = (w * households["reform_total_tax"].values).sum()
    baseline_benefits = (w * households["baseline_total_benefits"].values).sum()
    reform_benefits   = (w * households["reform_total_benefits"].values).sum()
    revenue_change   = reform_revenue - baseline_revenue
    benefit_change   = reform_benefits - baseline_benefits
    net_cost         = -revenue_change + benefit_change

    # ── Income breakdown (from person-level inputs) ───────────────────────────
    # Persons need to be joined to household weights via household_id
    p_with_w = persons.merge(
        households[["household_id", "weight"]], on="household_id", how="left"
    )
    pw = p_with_w["weight"].fillna(1.0).values

    def _wsum(col: str) -> float:
        return float((pw * p_with_w[col].fillna(0.0).values).sum()) if col in p_with_w.columns else 0.0

    income_breakdown = IncomeBreakdown(
        employment_income=_wsum("employment_income"),
        self_employment_income=_wsum("self_employment_income"),
        pension_income=_wsum("private_pension_income"),
        savings_interest_income=_wsum("savings_interest"),
        dividend_income=_wsum("dividend_income"),
        property_income=_wsum("property_income"),
        other_income=_wsum("other_income"),
    )

    # ── Program breakdown (benunit-level benefits, weighted by household) ─────
    bu_with_w = benunits.merge(
        households[["household_id", "weight"]], on="household_id", how="left"
    )
    bw = bu_with_w["weight"].fillna(1.0).values

    def _bwsum(col: str) -> float:
        return float((bw * bu_with_w[col].fillna(0.0).values).sum()) if col in bu_with_w.columns else 0.0

    # Person-level tax totals
    it_reform  = float((pw * p_with_w["reform_income_tax"].fillna(0.0).values).sum()) if "reform_income_tax" in p_with_w.columns else 0.0
    eni_reform = float((pw * p_with_w["reform_employee_ni"].fillna(0.0).values).sum()) if "reform_employee_ni" in p_with_w.columns else 0.0
    enr_reform = float((pw * p_with_w["reform_employer_ni"].fillna(0.0).values).sum()) if "reform_employer_ni" in p_with_w.columns else 0.0

    program_breakdown = ProgramBreakdown(
        income_tax=it_reform,
        employee_ni=eni_reform,
        employer_ni=enr_reform,
        universal_credit=_bwsum("reform_universal_credit"),
        child_benefit=_bwsum("reform_child_benefit"),
        state_pension=_bwsum("reform_state_pension"),
        pension_credit=_bwsum("reform_pension_credit"),
        housing_benefit=_bwsum("reform_housing_benefit"),
        child_tax_credit=_bwsum("reform_child_tax_credit"),
        working_tax_credit=_bwsum("reform_working_tax_credit"),
        income_support=_bwsum("reform_income_support"),
        esa_income_related=_bwsum("reform_esa_income_related"),
        jsa_income_based=_bwsum("reform_jsa_income_based"),
        carers_allowance=_bwsum("reform_carers_allowance"),
        scottish_child_payment=_bwsum("reform_scottish_child_payment"),
        benefit_cap_reduction=_bwsum("reform_benefit_cap_reduction"),
        passthrough_benefits=_bwsum("reform_passthrough_benefits"),
    )

    # ── Caseloads ─────────────────────────────────────────────────────────────
    caseloads = Caseloads(
        income_tax_payers=float((pw * (p_with_w.get("reform_income_tax", 0) > 0)).sum()) if "reform_income_tax" in p_with_w.columns else 0.0,
        ni_payers=float((pw * (p_with_w.get("reform_employee_ni", 0) > 0)).sum()) if "reform_employee_ni" in p_with_w.columns else 0.0,
        employer_ni_payers=float((pw * (p_with_w.get("reform_employer_ni", 0) > 0)).sum()) if "reform_employer_ni" in p_with_w.columns else 0.0,
        universal_credit=float((bw * (bu_with_w.get("reform_universal_credit", 0) > 0)).sum()) if "reform_universal_credit" in bu_with_w.columns else 0.0,
        child_benefit=float((bw * (bu_with_w.get("reform_child_benefit", 0) > 0)).sum()) if "reform_child_benefit" in bu_with_w.columns else 0.0,
        state_pension=float((bw * (bu_with_w.get("reform_state_pension", 0) > 0)).sum()) if "reform_state_pension" in bu_with_w.columns else 0.0,
        pension_credit=float((bw * (bu_with_w.get("reform_pension_credit", 0) > 0)).sum()) if "reform_pension_credit" in bu_with_w.columns else 0.0,
        housing_benefit=float((bw * (bu_with_w.get("reform_housing_benefit", 0) > 0)).sum()) if "reform_housing_benefit" in bu_with_w.columns else 0.0,
        child_tax_credit=float((bw * (bu_with_w.get("reform_child_tax_credit", 0) > 0)).sum()) if "reform_child_tax_credit" in bu_with_w.columns else 0.0,
        working_tax_credit=float((bw * (bu_with_w.get("reform_working_tax_credit", 0) > 0)).sum()) if "reform_working_tax_credit" in bu_with_w.columns else 0.0,
        income_support=float((bw * (bu_with_w.get("reform_income_support", 0) > 0)).sum()) if "reform_income_support" in bu_with_w.columns else 0.0,
        esa_income_related=float((bw * (bu_with_w.get("reform_esa_income_related", 0) > 0)).sum()) if "reform_esa_income_related" in bu_with_w.columns else 0.0,
        jsa_income_based=float((bw * (bu_with_w.get("reform_jsa_income_based", 0) > 0)).sum()) if "reform_jsa_income_based" in bu_with_w.columns else 0.0,
        carers_allowance=float((bw * (bu_with_w.get("reform_carers_allowance", 0) > 0)).sum()) if "reform_carers_allowance" in bu_with_w.columns else 0.0,
        scottish_child_payment=float((bw * (bu_with_w.get("reform_scottish_child_payment", 0) > 0)).sum()) if "reform_scottish_child_payment" in bu_with_w.columns else 0.0,
        benefit_cap_affected=float((bw * (bu_with_w.get("reform_benefit_cap_reduction", 0) < 0)).sum()) if "reform_benefit_cap_reduction" in bu_with_w.columns else 0.0,
    )

    # ── Decile impacts ────────────────────────────────────────────────────────
    # Rank households by baseline equivalised net income; measure change on
    # reform equivalised net income.
    eq = households["baseline_equivalisation_factor"].clip(lower=1e-9) if "baseline_equivalisation_factor" in households.columns else 1.0
    bl_equiv = households["baseline_net_income"].values / (eq.values if hasattr(eq, "values") else eq)
    rf_equiv = households["reform_net_income"].values  / (eq.values if hasattr(eq, "values") else eq)

    order = np.argsort(bl_equiv)
    bl_sorted = bl_equiv[order]
    rf_sorted = rf_equiv[order]

    n = len(order)
    decile_size = n // 10
    decile_impacts = []
    for d in range(10):
        start = d * decile_size
        end = n if d == 9 else (d + 1) * decile_size
        bl_sl = bl_sorted[start:end]
        rf_sl = rf_sorted[start:end]
        count = len(bl_sl)
        if count == 0:
            decile_impacts.append(DecileImpact(decile=d + 1))
            continue
        avg_base  = float(bl_sl.mean())
        avg_ref   = float(rf_sl.mean())
        avg_chg   = avg_ref - avg_base
        pct_chg   = 100.0 * avg_chg / avg_base if avg_base != 0 else 0.0
        decile_impacts.append(DecileImpact(
            decile=d + 1,
            avg_baseline_income=round(avg_base, 2),
            avg_reform_income=round(avg_ref, 2),
            avg_change=round(avg_chg, 2),
            pct_change=round(pct_chg, 2),
        ))

    # ── Winners and losers ────────────────────────────────────────────────────
    change = households["reform_net_income"].values - households["baseline_net_income"].values
    winners_w   = float((w * (change >  1.0)).sum())
    losers_w    = float((w * (change < -1.0)).sum())
    unchanged_w = float((w * (np.abs(change) <= 1.0)).sum())
    total_gain  = float((w * change * (change >  1.0)).sum())
    total_loss  = float((w * np.abs(change) * (change < -1.0)).sum())
    total_w     = winners_w + losers_w + unchanged_w

    winners_losers = WinnersLosers(
        winners_pct=round(100.0 * winners_w / total_w, 1) if total_w > 0 else 0.0,
        losers_pct=round(100.0 * losers_w / total_w, 1) if total_w > 0 else 0.0,
        unchanged_pct=round(100.0 * unchanged_w / total_w, 1) if total_w > 0 else 0.0,
        avg_gain=round(total_gain / winners_w) if winners_w > 0 else 0.0,
        avg_loss=round(total_loss / losers_w) if losers_w > 0 else 0.0,
    )

    fiscal_year = f"{year}/{(year + 1) % 100:02d}"

    return SimulationResult(
        fiscal_year=fiscal_year,
        budgetary_impact=BudgetaryImpact(
            baseline_revenue=float(baseline_revenue),
            reform_revenue=float(reform_revenue),
            revenue_change=float(revenue_change),
            baseline_benefits=float(baseline_benefits),
            reform_benefits=float(reform_benefits),
            benefit_spending_change=float(benefit_change),
            net_cost=float(net_cost),
        ),
        income_breakdown=income_breakdown,
        program_breakdown=program_breakdown,
        caseloads=caseloads,
        decile_impacts=decile_impacts,
        winners_losers=winners_losers,
        hbai_incomes=HbaiIncomes(
            mean_equiv_bhc=0.0, mean_equiv_ahc=0.0,
            mean_bhc=0.0, mean_ahc=0.0,
            median_equiv_bhc=0.0, median_equiv_ahc=0.0,
        ),
        baseline_poverty=PovertyHeadcounts(
            relative_bhc_children=0.0, relative_bhc_working_age=0.0, relative_bhc_pensioners=0.0,
            relative_ahc_children=0.0, relative_ahc_working_age=0.0, relative_ahc_pensioners=0.0,
            absolute_bhc_children=0.0, absolute_bhc_working_age=0.0, absolute_bhc_pensioners=0.0,
            absolute_ahc_children=0.0, absolute_ahc_working_age=0.0, absolute_ahc_pensioners=0.0,
        ),
        reform_poverty=PovertyHeadcounts(
            relative_bhc_children=0.0, relative_bhc_working_age=0.0, relative_bhc_pensioners=0.0,
            relative_ahc_children=0.0, relative_ahc_working_age=0.0, relative_ahc_pensioners=0.0,
            absolute_bhc_children=0.0, absolute_bhc_working_age=0.0, absolute_bhc_pensioners=0.0,
            absolute_ahc_children=0.0, absolute_ahc_working_age=0.0, absolute_ahc_pensioners=0.0,
        ),
        cpi_index=100.0,
    )
