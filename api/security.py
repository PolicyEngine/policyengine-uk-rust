"""Shared security helpers for the UK simulation API."""

import os
import sqlite3
import tempfile
import time
from pathlib import Path
from typing import Annotated

from fastapi import Header, HTTPException, Request, status

_LOCAL_CLIENT_HOSTS = {"127.0.0.1", "::1", "localhost", "testclient"}
_RATE_LIMIT_DB_ENV = "POLICYENGINE_UK_RATE_LIMIT_DB_PATH"


def _default_rate_limit_db_path() -> str:
    return str(Path(tempfile.gettempdir()) / "policyengine-uk-rate-limit.sqlite")


class RateLimiter:
    """SQLite-backed sliding-window limiter keyed by client host."""

    def __init__(
        self,
        limit: int,
        window_seconds: int,
        db_path: str | None = None,
    ):
        if window_seconds <= 0:
            raise ValueError("window_seconds must be positive")
        self.limit = limit
        self.window_seconds = window_seconds
        self.db_path = db_path or os.getenv(
            _RATE_LIMIT_DB_ENV, _default_rate_limit_db_path()
        )
        self._initialize_storage()

    def _initialize_storage(self) -> None:
        db_dir = os.path.dirname(self.db_path)
        if db_dir:
            os.makedirs(db_dir, exist_ok=True)

        with sqlite3.connect(self.db_path, timeout=5.0) as conn:
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS simulation_rate_limit_events (
                    client_host TEXT NOT NULL,
                    requested_at REAL NOT NULL
                )
                """
            )
            conn.execute(
                """
                CREATE INDEX IF NOT EXISTS idx_simulation_rate_limit_events
                ON simulation_rate_limit_events (client_host, requested_at)
                """
            )

    def check(self, request: Request, now: float | None = None) -> None:
        client_host = getattr(getattr(request, "client", None), "host", None)
        if client_host in _LOCAL_CLIENT_HOSTS:
            return

        now = time.time() if now is None else now
        cutoff = now - self.window_seconds
        client_key = client_host or "unknown"

        with sqlite3.connect(self.db_path, timeout=5.0, isolation_level=None) as conn:
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute("BEGIN IMMEDIATE")
            conn.execute(
                """
                DELETE FROM simulation_rate_limit_events
                WHERE client_host = ? AND requested_at <= ?
                """,
                (client_key, cutoff),
            )
            current_count = conn.execute(
                """
                SELECT COUNT(*)
                FROM simulation_rate_limit_events
                WHERE client_host = ?
                """,
                (client_key,),
            ).fetchone()[0]
            if current_count >= self.limit:
                raise HTTPException(
                    status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                    detail="Rate limit exceeded for simulation endpoints",
                )
            conn.execute(
                """
                INSERT INTO simulation_rate_limit_events (client_host, requested_at)
                VALUES (?, ?)
                """,
                (client_key, now),
            )


SIMULATION_RATE_LIMITER = RateLimiter(
    limit=int(os.getenv("POLICYENGINE_UK_RATE_LIMIT", "10")),
    window_seconds=int(os.getenv("POLICYENGINE_UK_RATE_LIMIT_WINDOW_SECONDS", "60")),
    db_path=os.getenv(_RATE_LIMIT_DB_ENV, _default_rate_limit_db_path()),
)


_SIMULATION_RATE_LIMITER_CONFIG = (
    SIMULATION_RATE_LIMITER.limit,
    SIMULATION_RATE_LIMITER.window_seconds,
    SIMULATION_RATE_LIMITER.db_path,
)


def get_simulation_rate_limiter() -> RateLimiter:
    """Return the configured shared limiter, rebuilding it if config changed."""

    global SIMULATION_RATE_LIMITER, _SIMULATION_RATE_LIMITER_CONFIG

    config = (
        int(os.getenv("POLICYENGINE_UK_RATE_LIMIT", "10")),
        int(os.getenv("POLICYENGINE_UK_RATE_LIMIT_WINDOW_SECONDS", "60")),
        os.getenv(_RATE_LIMIT_DB_ENV, _default_rate_limit_db_path()),
    )
    if _SIMULATION_RATE_LIMITER_CONFIG != config:
        SIMULATION_RATE_LIMITER = RateLimiter(
            limit=config[0],
            window_seconds=config[1],
            db_path=config[2],
        )
        _SIMULATION_RATE_LIMITER_CONFIG = config

    return SIMULATION_RATE_LIMITER


def require_simulation_api_key(
    request: Request,
    x_policyengine_api_key: Annotated[
        str | None, Header(alias="X-PolicyEngine-Api-Key")
    ] = None,
) -> None:
    """Require the shared API key for non-local simulation requests."""

    client_host = getattr(getattr(request, "client", None), "host", None)
    if client_host in _LOCAL_CLIENT_HOSTS:
        return

    expected_key = os.getenv("POLICYENGINE_UK_API_KEY", "").strip()
    if expected_key and x_policyengine_api_key == expected_key:
        return

    raise HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Simulation API key required",
    )


def enforce_simulation_rate_limit(request: Request) -> None:
    """Apply the shared sqlite-backed limiter to simulation endpoints."""

    get_simulation_rate_limiter().check(request)
