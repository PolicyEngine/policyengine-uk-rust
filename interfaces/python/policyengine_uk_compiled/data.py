"""Auto-download microdata from private GCS bucket using HMAC credentials."""

from __future__ import annotations

import base64
import hashlib
import hmac
import os
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

GCS_BUCKET = "policyengine-uk-microdata"
GCS_HOST = "storage.googleapis.com"
ENV_TOKEN = "POLICYENGINE_UK_DATA_TOKEN"
LOCAL_CACHE = Path.home() / ".policyengine-uk-data"


def _sign_request(method: str, path: str, access_key: str, secret_key: str) -> dict:
    """Sign a GCS XML API request using HMAC-SHA1."""
    date = datetime.now(timezone.utc).strftime("%a, %d %b %Y %H:%M:%S GMT")
    string_to_sign = f"{method}\n\n\n{date}\n/{GCS_BUCKET}{path}"
    signature = hmac.new(
        secret_key.encode(), string_to_sign.encode(), hashlib.sha1
    ).digest()
    sig_b64 = base64.b64encode(signature).decode()
    return {
        "Date": date,
        "Authorization": f"GOOG1 {access_key}:{sig_b64}",
    }


def _download_object(key: str, dest: Path, access_key: str, secret_key: str):
    """Download a single object from the bucket."""
    path = f"/{key}"
    headers = _sign_request("GET", path, access_key, secret_key)
    url = f"https://{GCS_HOST}/{GCS_BUCKET}{path}"
    req = urllib.request.Request(url, headers=headers)
    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(req) as resp:
        with open(dest, "wb") as f:
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)


def _get_credentials() -> tuple[str, str]:
    """Get HMAC credentials from the single token env var.

    Token format: {access_key}:{secret_key}
    """
    token = os.environ.get(ENV_TOKEN)
    if not token or ":" not in token:
        raise EnvironmentError(
            f"Set {ENV_TOKEN} to download data from gs://{GCS_BUCKET}. "
            f"Format: ACCESS_KEY:SECRET_KEY"
        )
    return token.split(":", 1)


DATASETS = ("frs", "lcfs", "spi", "was")


def ensure_dataset_year(dataset: str, year: int) -> Path:
    """Ensure clean CSVs for a dataset/year are available locally, downloading if needed.

    Returns the path to the year directory (e.g. ~/.policyengine-uk-data/frs/2026/).
    """
    year_dir = LOCAL_CACHE / dataset / str(year)
    expected_files = ["persons.csv", "benunits.csv", "households.csv"]
    if all((year_dir / f).exists() for f in expected_files):
        return year_dir

    access_key, secret_key = _get_credentials()
    year_dir.mkdir(parents=True, exist_ok=True)
    for f in expected_files:
        key = f"{dataset}/{year}/{f}"
        dest = year_dir / f
        if dest.exists():
            continue
        print(f"  Downloading {key}...", end="", flush=True)
        _download_object(key, dest, access_key, secret_key)
        print(" done")

    return year_dir


# Keep old name for backwards compatibility
def ensure_year(year: int) -> Path:
    return ensure_dataset_year("frs", year)


def ensure_frs(year: int, clean_frs_base: str | None = None) -> str:
    """Return a path to FRS data base dir, downloading the needed year if missing."""
    if clean_frs_base:
        year_dir = Path(clean_frs_base) / str(year)
        if year_dir.is_dir():
            return clean_frs_base

    local_base = LOCAL_CACHE / "frs"
    year_dir = local_base / str(year)
    expected = ["persons.csv", "benunits.csv", "households.csv"]
    if all((year_dir / f).exists() for f in expected):
        return str(local_base)

    if not os.environ.get(ENV_TOKEN):
        raise FileNotFoundError(
            f"No FRS data found for {year}. Either pass clean_frs_base= pointing to "
            f"a directory with a {year}/ subdirectory, or set {ENV_TOKEN} to "
            f"auto-download from GCS."
        )
    ensure_dataset_year("frs", year)
    return str(local_base)


def ensure_dataset(dataset: str, year: int) -> str:
    """Return a path to a dataset base dir, downloading the needed year if missing.

    Supports: frs, lcfs, spi, was.
    """
    if dataset not in DATASETS:
        raise ValueError(f"Unknown dataset {dataset!r}. Choose from: {DATASETS}")

    local_base = LOCAL_CACHE / dataset
    year_dir = local_base / str(year)
    expected = ["persons.csv", "benunits.csv", "households.csv"]
    if all((year_dir / f).exists() for f in expected):
        return str(local_base)

    if not os.environ.get(ENV_TOKEN):
        raise FileNotFoundError(
            f"No {dataset.upper()} data found for {year}. Set {ENV_TOKEN} to auto-download."
        )
    ensure_dataset_year(dataset, year)
    return str(local_base)


def download_all(force: bool = False, datasets: tuple = DATASETS) -> None:
    """Download all available years for the given datasets (default: all)."""
    import re
    access_key, secret_key = _get_credentials()

    for dataset in datasets:
        keys = []
        marker = ""
        while True:
            path = f"/?prefix={dataset}/&marker={marker}"
            headers = _sign_request("GET", "/", access_key, secret_key)
            url = f"https://{GCS_HOST}/{GCS_BUCKET}{path}"
            req = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(req) as resp:
                body = resp.read().decode()
            found = re.findall(r"<Key>([^<]+)</Key>", body)
            if not found:
                break
            keys.extend(found)
            if "<IsTruncated>true</IsTruncated>" not in body:
                break
            marker = found[-1]

        total = len(keys)
        for i, key in enumerate(keys, 1):
            rel = key[len(f"{dataset}/"):]
            if not rel:
                continue
            dest = LOCAL_CACHE / dataset / rel
            if dest.exists() and not force:
                continue
            _download_object(key, dest, access_key, secret_key)
            print(f"\r  Downloading {dataset}: {i}/{total}", end="", flush=True)
        if keys:
            print()
