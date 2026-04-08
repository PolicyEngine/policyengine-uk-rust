"""ONS demographic calibration targets.

Population by age group, total households, and regional distribution.
These are from ONS mid-year population estimates and household projections.

Sources:
- ONS mid-year population estimates 2023
- ONS household projections
"""

from __future__ import annotations


# ONS mid-2023 population estimates (UK), rounded to nearest 1000.
# Source: ONS MYE 2023.
_POPULATION = {
    "children_0_15": 12_200_000,
    "working_age_16_64": 42_100_000,
    "pensioners_65_plus": 12_600_000,
    "total": 66_900_000,
}

# ONS household projections 2023 (England + Scotland + Wales + NI)
_TOTAL_HOUSEHOLDS = 28_200_000

# Regional population shares (2023 mid-year estimates)
# Regions match the FRS region codes.
_REGIONAL_POPULATION = {
    "north_east": 2_650_000,
    "north_west": 7_400_000,
    "yorkshire": 5_550_000,
    "east_midlands": 4_900_000,
    "west_midlands": 5_950_000,
    "east_of_england": 6_350_000,
    "london": 8_800_000,
    "south_east": 9_300_000,
    "south_west": 5_700_000,
    "wales": 3_100_000,
    "scotland": 5_450_000,
    "northern_ireland": 1_900_000,
}


def get_targets() -> list[dict]:
    """Generate ONS demographic targets for all calibration years.

    Population changes slowly year-to-year, so we emit the same targets for
    each year in the calibration range. This ensures they bind regardless of
    which --year is passed to calibration.
    """
    targets = []

    # Emit for all plausible calibration years
    for year in range(2024, 2031):
        # Age group population counts
        for group, count in _POPULATION.items():
            if group == "total":
                continue
            if group == "children_0_15":
                age_filter = {"variable": "age", "min": 0, "max": 16}
            elif group == "working_age_16_64":
                age_filter = {"variable": "age", "min": 16, "max": 65}
            else:  # pensioners
                age_filter = {"variable": "age", "min": 65, "max": 200}

            targets.append(
                {
                    "name": f"ons/population_{group}/{year}",
                    "variable": "age",
                    "entity": "person",
                    "aggregation": "count",
                    "filter": age_filter,
                    "value": float(count),
                    "source": "ons",
                    "year": year,
                    "holdout": False,
                }
            )

        # Total population
        targets.append(
            {
                "name": f"ons/total_population/{year}",
                "variable": "age",
                "entity": "person",
                "aggregation": "count",
                "filter": None,
                "value": float(_POPULATION["total"]),
                "source": "ons",
                "year": year,
                "holdout": False,
            }
        )

        # Total households
        targets.append(
            {
                "name": f"ons/total_households/{year}",
                "variable": "household_id",
                "entity": "household",
                "aggregation": "count",
                "filter": None,
                "value": float(_TOTAL_HOUSEHOLDS),
                "source": "ons",
                "year": year,
                "holdout": False,
            }
        )

    return targets
