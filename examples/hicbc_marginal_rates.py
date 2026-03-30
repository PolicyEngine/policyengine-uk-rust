"""Demo: How widening the HICBC taper window affects marginal tax rates.

Batches all income levels into a single simulation call per scenario
for fast execution (~20ms per scenario instead of ~5s).
"""

import json
import time
import pandas as pd
from policyengine_uk_compiled import Simulation, Parameters, ChildBenefitParams
from policyengine_uk_compiled.engine import PERSON_DEFAULTS, BENUNIT_DEFAULTS, HOUSEHOLD_DEFAULTS

YEAR = 2025
INCOMES = list(range(40_000, 120_001, 1_000))
DELTA = 100  # £100 income increment for MTR calculation

SCENARIOS = {
    "Baseline (£60k-£80k)": 80_000,
    "£60k-£100k": 100_000,
    "£60k-£120k": 120_000,
}


def build_batched_dataset(incomes: list[float]):
    """Build a dataset with one household per income level, each with a parent + child.

    For MTR calculation we need two households per income: one at income, one at income+delta.
    So we create 2*len(incomes) households.
    """
    persons = []
    benunits = []
    households = []

    for i, income in enumerate(incomes):
        for j, inc in enumerate([float(income), float(income) + DELTA]):
            hh_id = i * 2 + j
            adult_id = hh_id * 2
            child_id = hh_id * 2 + 1

            # Adult
            persons.append({
                **PERSON_DEFAULTS,
                "person_id": adult_id,
                "benunit_id": hh_id,
                "household_id": hh_id,
                "age": 35,
                "employment_income": inc,
                "is_benunit_head": True,
                "is_household_head": True,
            })
            # Child
            persons.append({
                **PERSON_DEFAULTS,
                "person_id": child_id,
                "benunit_id": hh_id,
                "household_id": hh_id,
                "age": 5,
                "employment_income": 0.0,
                "is_benunit_head": False,
                "is_household_head": False,
            })

            benunits.append({
                **BENUNIT_DEFAULTS,
                "benunit_id": hh_id,
                "household_id": hh_id,
                "person_ids": f"{adult_id};{child_id}",
                "is_lone_parent": True,
            })

            households.append({
                **HOUSEHOLD_DEFAULTS,
                "household_id": hh_id,
                "benunit_ids": str(hh_id),
                "person_ids": f"{adult_id};{child_id}",
            })

    return pd.DataFrame(persons), pd.DataFrame(benunits), pd.DataFrame(households)


def compute_mtrs(taper_end: float) -> list[float]:
    """Compute marginal tax rates across all income levels in one batched call."""
    policy = None
    if taper_end != 80_000:
        policy = Parameters(child_benefit=ChildBenefitParams(hicbc_taper_end=taper_end))

    persons, benunits, households_df = build_batched_dataset(INCOMES)
    sim = Simulation(year=YEAR, persons=persons, benunits=benunits, households=households_df)

    t0 = time.perf_counter()
    result = sim.run_microdata(policy=policy)
    elapsed = time.perf_counter() - t0

    hh = result.households
    # Use reform columns when a reform policy is applied, baseline otherwise
    net_col = "reform_net_income" if policy else "baseline_net_income"
    # Households are in pairs: (income, income+delta) for each income level
    mtrs = []
    for i in range(len(INCOMES)):
        net1 = hh.loc[i * 2, net_col]
        net2 = hh.loc[i * 2 + 1, net_col]
        mtr = 1.0 - (net2 - net1) / DELTA
        mtrs.append(round(mtr * 100, 2))

    return mtrs, elapsed


print("Computing marginal tax rates (batched)...")
data = {"incomes": [inc / 1000 for inc in INCOMES]}
for label, taper_end in SCENARIOS.items():
    mtrs, elapsed = compute_mtrs(taper_end)
    data[label] = mtrs
    n_hh = len(INCOMES) * 2
    print(f"  {label}: {elapsed*1000:.0f}ms for {n_hh} households ({elapsed/n_hh*1000:.2f}ms/hh)")

with open("examples/hicbc_data.json", "w") as f:
    json.dump(data, f)
print(f"\nSaved to examples/hicbc_data.json")
