"""Rebuild every clean dataset from raw UKDS files held on GCS.

Pipeline per job: download raw tab files from gs://policyengine-uk-microdata/ukds/
→ run the Rust extraction → upload clean CSVs to gs://policyengine-uk-microdata/<dataset>/<year>/.

Raw files are cached in data/raw/ inside the repo so re-runs skip the download step.
Clean outputs land in data/clean/. Pass --work-dir to override both.

Assumes:
  - `gcloud storage` CLI is authenticated and can read/write the bucket.
  - `cargo` is on PATH and the workspace builds cleanly.

Usage:
    python scripts/rebuild_all.py                    # rebuild everything
    python scripts/rebuild_all.py --only lcfs        # rebuild just LCFS years
    python scripts/rebuild_all.py --only frs --year 2023
    python scripts/rebuild_all.py --only efrs        # rebuild EFRS for all FRS years we have
    python scripts/rebuild_all.py --work-dir /tmp/pe # use a custom work dir
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

BUCKET = "gs://policyengine-uk-microdata"
RAW_PREFIX = f"{BUCKET}/ukds"
REPO_ROOT = Path(__file__).resolve().parent.parent

# Extra search paths for gcloud/cargo that might not be on the default subprocess PATH.
_EXTRA_PATHS = [
    Path.home() / ".cargo" / "bin",
    Path.home() / "Downloads" / "google-cloud-sdk" / "bin",
    Path("/opt/homebrew/bin"),
    Path("/usr/local/bin"),
]
for _p in _EXTRA_PATHS:
    if _p.is_dir() and str(_p) not in os.environ.get("PATH", ""):
        os.environ["PATH"] = f"{_p}:{os.environ.get('PATH', '')}"


def _require(tool: str) -> None:
    if shutil.which(tool) is None:
        raise SystemExit(
            f"{tool!r} not found on PATH. Install it or add it to PATH before running."
        )


@dataclass
class ExtractJob:
    """One raw survey → clean CSV extraction."""
    dataset: str       # frs | lcfs | spi | was
    year: int          # target fiscal year for the clean output directory
    raw_ref: str       # path under ukds/ (e.g. "frs/2023", "was/round_7")
    rust_flag: str     # --frs | --lcfs | --spi | --was


# Manifest of everything we can rebuild. Extend as new raw years arrive on the bucket.
JOBS: list[ExtractJob] = [
    ExtractJob("frs",  2022, "frs/2022",     "--frs"),
    ExtractJob("frs",  2023, "frs/2023",     "--frs"),
    ExtractJob("lcfs", 2019, "lcfs/2019",    "--lcfs"),
    ExtractJob("lcfs", 2021, "lcfs/2021",    "--lcfs"),
    ExtractJob("lcfs", 2022, "lcfs/2022",    "--lcfs"),
    ExtractJob("spi",  2021, "spi/2021",     "--spi"),
    ExtractJob("spi",  2022, "spi/2022",     "--spi"),
    ExtractJob("was",  2020, "was/round_7",  "--was"),
    ExtractJob("was",  2022, "was/round_8",  "--was"),
]

# EFRS pipeline: (fiscal_year, frs_year, was_ref, lcfs_ref)
# Picks the raw references it composes from.
EFRS_JOBS: list[tuple[int, int, str, str]] = [
    (2023, 2023, "was/round_7", "lcfs/2021"),
]


def run(cmd: list, cwd: Path | None = None) -> None:
    print(f"  $ {' '.join(str(c) for c in cmd)}", flush=True)
    subprocess.run([str(c) for c in cmd], cwd=cwd, check=True)


def gcs_copy_in(ref: str, dest: Path) -> None:
    """Download everything under ukds/<ref>/ into dest/."""
    dest.mkdir(parents=True, exist_ok=True)
    # gcloud storage cp -r copies the listed objects verbatim.
    run(["gcloud", "storage", "cp", "-r", f"{RAW_PREFIX}/{ref}/*", str(dest)])


def gcs_copy_out(local_dir: Path, dataset: str, year: int) -> None:
    dest = f"{BUCKET}/{dataset}/{year}/"
    # Upload the three clean CSVs only; ignore any stray files.
    files = sorted(local_dir.glob("*.csv"))
    if not files:
        raise SystemExit(f"No CSV files in {local_dir}; extraction probably failed")
    run(["gcloud", "storage", "cp", *[str(f) for f in files], dest])


def ensure_raw(ref: str, work: Path) -> Path:
    """Download raw ukds/<ref> to work/raw/<ref>, caching if already present."""
    raw_dir = work / "raw" / ref
    if raw_dir.is_dir() and any(raw_dir.iterdir()):
        print(f"  (cached) {raw_dir}")
        return raw_dir
    gcs_copy_in(ref, raw_dir)
    return raw_dir


def extract_one(job: ExtractJob, work: Path) -> Path:
    print(f"\n=== {job.dataset.upper()} {job.year} ===")
    raw_dir = ensure_raw(job.raw_ref, work)
    clean_dir = work / "clean" / job.dataset / str(job.year)
    clean_dir.mkdir(parents=True, exist_ok=True)
    run(
        [
            "cargo", "run", "--release", "--quiet", "--",
            job.rust_flag, str(raw_dir),
            "--year", str(job.year),
            "--extract", str(clean_dir),
        ],
        cwd=REPO_ROOT,
    )
    gcs_copy_out(clean_dir, job.dataset, job.year)
    return clean_dir


def extract_efrs(fiscal_year: int, frs_year: int, was_ref: str, lcfs_ref: str, work: Path) -> None:
    print(f"\n=== EFRS {fiscal_year} (from FRS {frs_year}, {was_ref}, {lcfs_ref}) ===")

    # Need clean FRS as the base: if we already extracted it in this run it's on disk;
    # otherwise download the clean files from the bucket into work/clean/frs/<year>/.
    frs_clean = work / "clean" / "frs" / str(frs_year)
    if not frs_clean.is_dir() or not (frs_clean / "households.csv").exists():
        frs_clean.mkdir(parents=True, exist_ok=True)
        run([
            "gcloud", "storage", "cp",
            f"{BUCKET}/frs/{frs_year}/persons.csv",
            f"{BUCKET}/frs/{frs_year}/benunits.csv",
            f"{BUCKET}/frs/{frs_year}/households.csv",
            str(frs_clean) + "/",
        ])

    frs_base = work / "clean" / "frs"  # parent dir with YYYY/ subdirs
    was_raw = ensure_raw(was_ref, work)
    lcfs_raw = ensure_raw(lcfs_ref, work)

    efrs_out = work / "clean" / "efrs" / str(fiscal_year)
    efrs_out.mkdir(parents=True, exist_ok=True)
    run(
        [
            "cargo", "run", "--release", "--quiet", "--",
            "--extract-efrs", str(efrs_out),
            "--data", str(frs_base),
            "--year", str(fiscal_year),
            "--was-dir", str(was_raw),
            "--lcfs-dir", str(lcfs_raw),
        ],
        cwd=REPO_ROOT,
    )
    gcs_copy_out(efrs_out, "efrs", fiscal_year)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__ or "")
    parser.add_argument(
        "--only",
        choices=["frs", "lcfs", "spi", "was", "efrs"],
        help="Only rebuild one dataset family",
    )
    parser.add_argument("--year", type=int, help="Only rebuild this fiscal year")
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=REPO_ROOT / "data",
        help="Working directory for raw downloads and clean outputs (default: data/)",
    )
    args = parser.parse_args()

    _require("gcloud")
    _require("cargo")

    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    print(f"Working directory: {work}")

    selected_jobs = JOBS
    if args.only and args.only != "efrs":
        selected_jobs = [j for j in JOBS if j.dataset == args.only]
    if args.year is not None:
        selected_jobs = [j for j in selected_jobs if j.year == args.year]

    run_efrs = args.only in (None, "efrs")

    if args.only != "efrs":
        for job in selected_jobs:
            extract_one(job, work)

    if run_efrs:
        for fiscal_year, frs_year, was_ref, lcfs_ref in EFRS_JOBS:
            if args.year is not None and fiscal_year != args.year:
                continue
            extract_efrs(fiscal_year, frs_year, was_ref, lcfs_ref, work)

    print("\nAll done.")


if __name__ == "__main__":
    sys.exit(main())
