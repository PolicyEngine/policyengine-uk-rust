"""Interface to the compiled PolicyEngine UK Rust binary."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Optional

from policyengine_uk_compiled.models import Parameters, SimulationConfig, SimulationResult

# The binary and parameters/ dir are bundled inside the package at build time.
_PKG_DIR = Path(__file__).resolve().parent
_BUNDLED_BINARY = _PKG_DIR / "bin" / "policyengine-uk-rust"


def _find_binary() -> str:
    """Locate the policyengine-uk-rust binary.

    Search order:
      1. Bundled inside the installed package (policyengine_uk_compiled/bin/)
      2. Development: target/release or target/debug relative to repo root
    """
    if _BUNDLED_BINARY.is_file():
        return str(_BUNDLED_BINARY)

    root = _PKG_DIR.parent
    for subdir in ("target/release", "target/debug"):
        p = root / subdir / "policyengine-uk-rust"
        if p.is_file():
            return str(p)

    raise FileNotFoundError(
        "Cannot find policyengine-uk-rust binary. "
        "Install the package (`pip install policyengine-uk-compiled`) "
        "or build from source (`cargo build --release`)."
    )


def _find_cwd(binary_path: str) -> str:
    """Find the working directory that contains parameters/.

    The binary resolves parameters/ relative to cwd.
    Search order:
      1. Bundled inside the package (policyengine_uk_compiled/)
      2. Repo root (for development)
    """
    if (_PKG_DIR / "parameters").is_dir():
        return str(_PKG_DIR)

    binary = Path(binary_path).resolve()
    for ancestor in (binary.parent, binary.parent.parent, binary.parent.parent.parent):
        if (ancestor / "parameters").is_dir():
            return str(ancestor)

    raise FileNotFoundError("Cannot find parameters/ directory.")


class Simulation:
    """Run the PolicyEngine UK microsimulation engine.

    Usage::

        from policyengine_uk_compiled import Simulation, Parameters, IncomeTaxParams

        sim = Simulation(year=2025, clean_frs_base="data/frs")
        result = sim.run()
        print(result.budgetary_impact.net_cost)

        # With a reform:
        reform = Parameters(income_tax=IncomeTaxParams(personal_allowance=20000))
        result = sim.run(policy=reform)
    """

    def __init__(
        self,
        year: int = 2025,
        *,
        clean_frs_base: Optional[str] = None,
        clean_frs: Optional[str] = None,
        frs_raw: Optional[str] = None,
        binary_path: Optional[str] = None,
    ):
        self.year = year
        self.clean_frs_base = clean_frs_base
        self.clean_frs = clean_frs
        self.frs_raw = frs_raw
        self.binary_path = binary_path or _find_binary()

    @classmethod
    def from_config(cls, config: SimulationConfig) -> "Simulation":
        return cls(
            year=config.year,
            clean_frs_base=config.clean_frs_base,
            clean_frs=config.clean_frs,
            frs_raw=config.frs_raw,
            binary_path=config.binary_path,
        )

    def _build_cmd(self, policy: Optional[Parameters] = None) -> list[str]:
        cmd = [self.binary_path, "--year", str(self.year), "--output", "json"]

        if self.clean_frs_base:
            cmd += ["--clean-frs-base", self.clean_frs_base]
        elif self.clean_frs:
            cmd += ["--clean-frs", self.clean_frs]
        elif self.frs_raw:
            cmd += ["--frs-raw", self.frs_raw]

        if policy:
            overlay = policy.model_dump(exclude_none=True)
            if overlay:
                cmd += ["--policy-json", json.dumps(overlay)]

        return cmd

    def run(self, policy: Optional[Parameters] = None, timeout: int = 120) -> SimulationResult:
        """Run the simulation and return typed results.

        Args:
            policy: Reform parameters (overlay on baseline). None = baseline only.
            timeout: Maximum seconds to wait for the binary.

        Returns:
            SimulationResult with budgetary impact, program breakdown, decile impacts, etc.
        """
        cmd = self._build_cmd(policy)
        cwd = _find_cwd(self.binary_path)
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=cwd,
        )
        if result.returncode != 0:
            raise RuntimeError(
                f"Simulation failed (exit {result.returncode}):\n{result.stderr}"
            )
        data = json.loads(result.stdout)
        return SimulationResult(**data)

    def get_baseline_params(self, timeout: int = 10) -> dict:
        """Export the baseline parameters for the configured year as a dict."""
        cmd = [self.binary_path, "--year", str(self.year), "--export-params-json"]
        cwd = _find_cwd(self.binary_path)
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=cwd,
        )
        if result.returncode != 0:
            raise RuntimeError(f"Failed to export params: {result.stderr}")
        return json.loads(result.stdout)
