import pytest
from fastapi import HTTPException, Request

from api.main import app
from api.modal_app import _make_fastapi_app
from api import security
from api.security import (
    RateLimiter,
    require_simulation_api_key,
)


def _make_request(
    path: str,
    client_host: str = "203.0.113.10",
    headers: dict[str, str] | None = None,
) -> Request:
    raw_headers = [(b"host", b"api.policyengine.org")]
    for key, value in (headers or {}).items():
        raw_headers.append((key.lower().encode("ascii"), value.encode("ascii")))

    scope = {
        "type": "http",
        "asgi": {"version": "3.0"},
        "http_version": "1.1",
        "method": "POST",
        "scheme": "https",
        "path": path,
        "raw_path": path.encode("ascii"),
        "query_string": b"",
        "headers": raw_headers,
        "client": (client_host, 443),
        "server": ("api.policyengine.org", 443),
    }

    async def receive():
        return {"type": "http.request", "body": b"", "more_body": False}

    return Request(scope, receive)


def test_simulation_guard_blocks_external_requests_without_key(monkeypatch):
    monkeypatch.setenv("POLICYENGINE_UK_API_KEY", "secret-key")
    request = _make_request("/api/simulate")

    with pytest.raises(HTTPException, match="Simulation API key required") as exc_info:
        require_simulation_api_key(request)

    assert exc_info.value.status_code == 401


def test_simulation_guard_allows_external_requests_with_key(monkeypatch):
    monkeypatch.setenv("POLICYENGINE_UK_API_KEY", "secret-key")
    request = _make_request(
        "/api/simulate",
        headers={"X-PolicyEngine-Api-Key": "secret-key"},
    )

    assert (
        require_simulation_api_key(
            request, x_policyengine_api_key="secret-key"
        )
        is None
    )


def test_simulation_guard_allows_local_requests_without_key(monkeypatch):
    monkeypatch.setenv("POLICYENGINE_UK_API_KEY", "secret-key")
    request = _make_request("/api/simulate", client_host="127.0.0.1")

    assert require_simulation_api_key(request) is None


def test_rate_limiter_blocks_requests_over_window(tmp_path):
    limiter = RateLimiter(
        limit=2,
        window_seconds=60,
        db_path=str(tmp_path / "rate-limit.sqlite"),
    )
    request = _make_request("/api/simulate", headers={"X-PolicyEngine-Api-Key": "secret-key"})

    limiter.check(request, now=100)
    limiter.check(request, now=120)

    with pytest.raises(HTTPException, match="Rate limit exceeded") as exc_info:
        limiter.check(request, now=130)

    assert exc_info.value.status_code == 429


def test_rate_limiter_expires_old_requests(tmp_path):
    limiter = RateLimiter(
        limit=2,
        window_seconds=60,
        db_path=str(tmp_path / "rate-limit.sqlite"),
    )
    request = _make_request("/api/simulate", headers={"X-PolicyEngine-Api-Key": "secret-key"})

    limiter.check(request, now=100)
    limiter.check(request, now=120)
    limiter.check(request, now=161)


def test_rate_limiter_shares_state_across_instances(tmp_path):
    db_path = tmp_path / "rate-limit.sqlite"
    request = _make_request("/api/simulate", headers={"X-PolicyEngine-Api-Key": "secret-key"})

    first = RateLimiter(limit=2, window_seconds=60, db_path=str(db_path))
    second = RateLimiter(limit=2, window_seconds=60, db_path=str(db_path))

    first.check(request, now=100)
    second.check(request, now=120)

    third = RateLimiter(limit=2, window_seconds=60, db_path=str(db_path))
    with pytest.raises(HTTPException, match="Rate limit exceeded") as exc_info:
        third.check(request, now=130)

    assert exc_info.value.status_code == 429


def test_rate_limiter_rebuilds_when_env_db_path_changes(monkeypatch, tmp_path):
    new_db_path = tmp_path / "shared-rate-limit.sqlite"
    monkeypatch.setenv("POLICYENGINE_UK_RATE_LIMIT_DB_PATH", str(new_db_path))

    limiter = security.get_simulation_rate_limiter()

    assert limiter.db_path == str(new_db_path)


@pytest.mark.parametrize("fastapi_app", [app, _make_fastapi_app()], ids=["main", "modal"])
def test_expensive_post_routes_are_protected(fastapi_app):
    protected_paths = {"/api/simulate", "/api/simulate-multi"}

    route_map = {
        route.path: route
        for route in fastapi_app.routes
        if getattr(route, "methods", None) and "POST" in route.methods
    }

    for path in protected_paths:
        route = route_map[path]
        dependency_names = {
            getattr(dependency.call, "__name__", "")
            for dependency in route.dependant.dependencies
        }
        assert "require_simulation_api_key" in dependency_names
        assert "enforce_simulation_rate_limit" in dependency_names
