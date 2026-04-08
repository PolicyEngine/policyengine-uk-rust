"""Build calibration targets from all sources and write to JSON.

Usage:
    python scripts/build_targets/main.py                    # build all targets
    python scripts/build_targets/main.py --year 2025        # filter to a specific year
    python scripts/build_targets/main.py --clear-cache      # re-fetch cached data
    python scripts/build_targets/main.py --output out.json  # custom output path
"""

from __future__ import annotations

import argparse
import json
import logging
import shutil
import sys
from pathlib import Path

from rich.console import Console
from rich.table import Table

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from build_targets import obr, hmrc, dwp, ons  # noqa: E402

logging.basicConfig(level=logging.INFO, format="%(levelname)s: %(message)s")
logger = logging.getLogger(__name__)

DEFAULT_OUTPUT = REPO_ROOT / "data" / "calibration_targets.json"
CACHE_DIR = REPO_ROOT / "data" / "cache"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__ or "")
    parser.add_argument("--year", type=int, help="Only include targets for this year")
    parser.add_argument(
        "--output", type=Path, default=DEFAULT_OUTPUT, help="Output JSON path"
    )
    parser.add_argument(
        "--clear-cache",
        action="store_true",
        help="Clear cached downloads before building",
    )
    args = parser.parse_args()

    if args.clear_cache and CACHE_DIR.exists():
        shutil.rmtree(CACHE_DIR)
        logger.info("Cleared cache directory")

    console = Console()
    all_targets: list[dict] = []
    sources = [
        ("OBR", obr.get_targets),
        ("HMRC", hmrc.get_targets),
        ("DWP", dwp.get_targets),
        ("ONS", ons.get_targets),
    ]

    for name, getter in sources:
        try:
            targets = getter()
            all_targets.extend(targets)
            logger.info("%s: %d targets", name, len(targets))
        except Exception as e:
            logger.error("%s: failed — %s", name, e)

    # Filter by year if requested
    if args.year:
        all_targets = [t for t in all_targets if t["year"] == args.year]

    # Deduplicate by name
    seen: dict[str, dict] = {}
    for t in all_targets:
        seen[t["name"]] = t
    all_targets = list(seen.values())

    # Write output
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({"targets": all_targets}, f, indent=2)

    # Summary table
    table = Table(title="Calibration targets")
    table.add_column("Source")
    table.add_column("Count", justify="right")
    table.add_column("Training", justify="right")
    table.add_column("Holdout", justify="right")

    by_source: dict[str, list[dict]] = {}
    for t in all_targets:
        by_source.setdefault(t["source"], []).append(t)

    for source, targets in sorted(by_source.items()):
        training = sum(1 for t in targets if not t.get("holdout"))
        holdout = sum(1 for t in targets if t.get("holdout"))
        table.add_row(source, str(len(targets)), str(training), str(holdout))

    table.add_row(
        "Total",
        str(len(all_targets)),
        str(sum(1 for t in all_targets if not t.get("holdout"))),
        str(sum(1 for t in all_targets if t.get("holdout"))),
        style="bold",
    )

    console.print(table)
    console.print(f"\nWrote {len(all_targets)} targets to {args.output}")


if __name__ == "__main__":
    main()
