"""Generate uprated FRS datasets for 2024-2029 from 2023 base, then upload all to GCS."""

import csv
import shutil
from pathlib import Path

BASE_YEAR = 2023
TARGET_YEARS = range(2024, 2030)
FRS_DIR = Path("data/frs")

# ── Uprating indices (matching src/data/mod.rs) ──────────────────────────────
# YoY growth rates by (year, rate) — rate applies to transition INTO that year

EARNINGS = {2022: 0.0614, 2023: 0.0622, 2024: 0.0493, 2025: 0.0517, 2026: 0.0333, 2027: 0.0225, 2028: 0.0210, 2029: 0.0221, 2030: 0.0232}
CPI = {2022: 0.0907, 2023: 0.0730, 2024: 0.0253, 2025: 0.0345, 2026: 0.0248, 2027: 0.0202, 2028: 0.0204, 2029: 0.0204, 2030: 0.0200}
GDP_PC = {2022: 0.1019, 2023: 0.0532, 2024: 0.0372, 2025: 0.0418, 2026: 0.0327, 2027: 0.0326, 2028: 0.0302, 2029: 0.0294, 2030: 0.0306}
MIXED_PC = {2022: 0.0296, 2023: -0.0060, 2024: 0.0273, 2025: 0.0024, 2026: 0.0362, 2027: 0.0374, 2028: 0.0351, 2029: 0.0358, 2030: 0.0364}
RENT = {2022: 0.0347, 2023: 0.0575, 2024: 0.0716, 2025: 0.0546, 2026: 0.0334, 2027: 0.0311, 2028: 0.0243, 2029: 0.0234, 2030: 0.0254}
COUNCIL_TAX = {2023: 0.051, 2024: 0.051, 2025: 0.0781, 2026: 0.0530, 2027: 0.0579, 2028: 0.0565, 2029: 0.0547, 2030: 0.0542}
POPULATION = {2022: 0.0093, 2023: 0.0131, 2024: 0.0107, 2025: 0.0072, 2026: 0.0038, 2027: 0.0037, 2028: 0.0040, 2029: 0.0044, 2030: 0.0045}
INTEREST = {2022: 1.210, 2023: 0.987, 2024: 0.142, 2025: 0.0519, 2026: 0.0565, 2027: 0.0474, 2028: 0.0364, 2029: 0.0302, 2030: 0.0292}

# Default long-run rates
DEFAULTS = {
    "earnings": 0.0383, "cpi": 0.0200, "gdp_pc": 0.0306,
    "mixed_pc": 0.0364, "rent": 0.0254, "council_tax": 0.0542,
    "population": 0.0045, "interest": 0.0292,
}

def cumulative(rates: dict, default: float, base_year: int, target_year: int) -> float:
    factor = 1.0
    for y in range(base_year + 1, target_year + 1):
        factor *= 1.0 + rates.get(y, default)
    return factor

# Person field → index mapping
PERSON_EARNINGS = {"employment_income", "employee_pension_contributions", "personal_pension_contributions"}
PERSON_MIXED = {"self_employment_income"}
PERSON_GDP = {"private_pension_income", "dividend_income", "property_income", "maintenance_income", "miscellaneous_income", "other_income"}
PERSON_INTEREST = {"savings_interest"}
PERSON_CPI = {
    "state_pension", "child_benefit", "housing_benefit", "income_support",
    "pension_credit", "child_tax_credit", "working_tax_credit", "universal_credit",
    "dla_care", "dla_mobility", "pip_daily_living", "pip_mobility",
    "carers_allowance", "attendance_allowance", "esa_income", "esa_contributory",
    "jsa_income", "jsa_contributory", "other_benefits",
    "adp_daily_living", "adp_mobility", "cdp_care", "cdp_mobility",
    "childcare_expenses",
}


def uprate_persons(src: Path, dest: Path, base_year: int, target_year: int):
    e = cumulative(EARNINGS, DEFAULTS["earnings"], base_year, target_year)
    c = cumulative(CPI, DEFAULTS["cpi"], base_year, target_year)
    g = cumulative(GDP_PC, DEFAULTS["gdp_pc"], base_year, target_year)
    m = cumulative(MIXED_PC, DEFAULTS["mixed_pc"], base_year, target_year)
    i = cumulative(INTEREST, DEFAULTS["interest"], base_year, target_year)

    with open(src) as fin, open(dest, "w", newline="") as fout:
        reader = csv.DictReader(fin)
        writer = csv.DictWriter(fout, fieldnames=reader.fieldnames)
        writer.writeheader()
        for row in reader:
            for field in reader.fieldnames:
                if field in PERSON_EARNINGS:
                    row[field] = str(float(row[field]) * e)
                elif field in PERSON_MIXED:
                    row[field] = str(float(row[field]) * m)
                elif field in PERSON_GDP:
                    row[field] = str(float(row[field]) * g)
                elif field in PERSON_INTEREST:
                    row[field] = str(float(row[field]) * i)
                elif field in PERSON_CPI:
                    row[field] = str(float(row[field]) * c)
            writer.writerow(row)


def uprate_households(src: Path, dest: Path, base_year: int, target_year: int):
    r = cumulative(RENT, DEFAULTS["rent"], base_year, target_year)
    ct = cumulative(COUNCIL_TAX, DEFAULTS["council_tax"], base_year, target_year)
    p = cumulative(POPULATION, DEFAULTS["population"], base_year, target_year)

    with open(src) as fin, open(dest, "w", newline="") as fout:
        reader = csv.DictReader(fin)
        writer = csv.DictWriter(fout, fieldnames=reader.fieldnames)
        writer.writeheader()
        for row in reader:
            row["rent_annual"] = str(float(row["rent_annual"]) * r)
            row["council_tax_annual"] = str(float(row["council_tax_annual"]) * ct)
            row["weight"] = str(float(row["weight"]) * p)
            writer.writerow(row)


def main():
    for year in TARGET_YEARS:
        print(f"Generating {year}...", end=" ", flush=True)
        dest_dir = FRS_DIR / str(year)
        dest_dir.mkdir(parents=True, exist_ok=True)

        uprate_persons(FRS_DIR / str(BASE_YEAR) / "persons.csv", dest_dir / "persons.csv", BASE_YEAR, year)
        # benunits have no monetary fields to uprate
        shutil.copy2(FRS_DIR / str(BASE_YEAR) / "benunits.csv", dest_dir / "benunits.csv")
        uprate_households(FRS_DIR / str(BASE_YEAR) / "households.csv", dest_dir / "households.csv", BASE_YEAR, year)
        print("done")

    print("\nUploading all years to GCS...")
    import subprocess
    subprocess.run(
        ["gcloud", "storage", "cp", "-r", "data/frs/*", "gs://policyengine-uk-microdata/frs/"],
        check=True,
    )
    print("Done!")


if __name__ == "__main__":
    main()
