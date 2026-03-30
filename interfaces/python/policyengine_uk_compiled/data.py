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


def ensure_year(year: int) -> Path:
    """Ensure FRS data for a specific year is available locally, downloading if needed.

    Returns the path to the year directory (e.g. ~/.policyengine-uk-data/frs/2023/).
    """
    year_dir = LOCAL_CACHE / "frs" / str(year)
    expected_files = ["persons.csv", "benunits.csv", "households.csv"]
    if all((year_dir / f).exists() for f in expected_files):
        return year_dir

    access_key, secret_key = _get_credentials()
    year_dir.mkdir(parents=True, exist_ok=True)
    for f in expected_files:
        key = f"frs/{year}/{f}"
        dest = year_dir / f
        if dest.exists():
            continue
        print(f"  Downloading {key}...", end="", flush=True)
        _download_object(key, dest, access_key, secret_key)
        print(" done")

    return year_dir


def ensure_frs(year: int, clean_frs_base: str | None = None) -> str:
    """Return a path to FRS data base dir, downloading the needed year if missing.

    Args:
        year: The fiscal year to ensure data for.
        clean_frs_base: Explicit path. If it exists with data for this year, returned as-is.

    Returns:
        Path string to the FRS base directory (containing year subdirs).
    """
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
    ensure_year(year)
    return str(local_base)


def download_all(force: bool = False) -> Path:
    """Download all available FRS years. Returns the base frs directory."""
    import re
    access_key, secret_key = _get_credentials()

    # List all objects to discover years
    keys = []
    marker = ""
    while True:
        path = f"/?prefix=frs/&marker={marker}"
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
        rel = key[len("frs/"):]
        if not rel:
            continue
        dest = LOCAL_CACHE / "frs" / rel
        if dest.exists() and not force:
            continue
        _download_object(key, dest, access_key, secret_key)
        print(f"\r  Downloading frs: {i}/{total}", end="", flush=True)
    print()
    return LOCAL_CACHE / "frs"
