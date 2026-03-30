"""
Upload clean FRS data to the Modal Volume.

Usage:
    python api/upload_frs.py data/frs

The directory should contain per-year subdirectories (1994/, 1995/, ..., 2023/)
each with persons.csv, benunits.csv, households.csv.

Generate with:
    for year in $(seq 1994 2023); do
        ./target/release/policyengine-uk-rust --year $year --frs <tab-dir> --extract-frs data/frs/$year
    done

This only needs to be run once (or when the FRS data changes).
"""

import sys
from pathlib import Path
import modal

VOLUME_NAME = "policyengine-uk-frs"


def upload(local_dir: Path) -> None:
    year_dirs = sorted(d for d in local_dir.iterdir() if d.is_dir() and d.name.isdigit())
    if not year_dirs:
        print(f"Error: no year directories found in {local_dir}", file=sys.stderr)
        sys.exit(1)

    volume = modal.Volume.from_name(VOLUME_NAME, create_if_missing=True)
    print(f"Uploading {len(year_dirs)} years from {local_dir} → Modal Volume '{VOLUME_NAME}'")

    with volume.batch_upload(force=True) as batch:
        for year_dir in year_dirs:
            csvs = list(year_dir.glob("*.csv"))
            if not csvs:
                print(f"  WARNING: no CSVs in {year_dir.name}, skipping")
                continue
            print(f"  {year_dir.name}: {len(csvs)} files")
            for f in csvs:
                batch.put_file(str(f), f"{year_dir.name}/{f.name}")

    print("Done.")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: python {sys.argv[0]} <frs-clean-base-dir>", file=sys.stderr)
        sys.exit(1)

    local_dir = Path(sys.argv[1])
    if not local_dir.is_dir():
        print(f"Error: {local_dir} is not a directory", file=sys.stderr)
        sys.exit(1)

    upload(local_dir)
