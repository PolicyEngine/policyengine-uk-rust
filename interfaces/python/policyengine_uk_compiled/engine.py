"""Interface to the compiled PolicyEngine UK Rust binary."""

from __future__ import annotations

import io
import json
import subprocess
from pathlib import Path
from typing import Optional, Union

try:
    import pandas as pd
    HAS_PANDAS = True
except ImportError:
    HAS_PANDAS = False

from policyengine_uk_compiled.models import MicrodataResult, Parameters, SimulationResult

# The binary and parameters/ dir are bundled inside the package at build time.
_PKG_DIR = Path(__file__).resolve().parent
_BUNDLED_BINARY = _PKG_DIR / "bin" / "policyengine-uk-rust"

# Default column schemas with sensible defaults for hypothetical households.
PERSON_DEFAULTS = {
    "person_id": 0, "benunit_id": 0, "household_id": 0,
    "age": 30, "gender": "male",
    "is_benunit_head": True, "is_household_head": True,
    "employment_income": 0.0, "self_employment_income": 0.0,
    "private_pension_income": 0.0, "state_pension": 0.0,
    "savings_interest": 0.0, "dividend_income": 0.0,
    "property_income": 0.0, "maintenance_income": 0.0,
    "miscellaneous_income": 0.0, "other_income": 0.0,
    "is_in_scotland": False, "hours_worked_annual": 0.0,
}

BENUNIT_DEFAULTS = {
    "benunit_id": 0, "household_id": 0, "person_ids": "0",
    "migration_seed": 0.0, "on_uc": False, "on_legacy": False,
    "rent_monthly": 0.0, "is_lone_parent": False,
    "would_claim_uc": True, "would_claim_cb": True,
    "would_claim_hb": True, "would_claim_pc": True,
    "would_claim_ctc": True, "would_claim_wtc": True,
    "would_claim_is": True, "would_claim_esa": True,
    "would_claim_jsa": True,
}

HOUSEHOLD_DEFAULTS = {
    "household_id": 0, "benunit_ids": "0", "person_ids": "0",
    "weight": 1.0, "region": "London",
    "rent_annual": 0.0, "council_tax_annual": 0.0,
}


def _find_binary() -> str:
    """Locate the policyengine-uk-rust binary."""
    if _BUNDLED_BINARY.is_file():
        return str(_BUNDLED_BINARY)
    # Walk up from package dir to find the repo root containing target/
    candidate = _PKG_DIR.parent
    for _ in range(5):
        for subdir in ("target/release", "target/debug"):
            p = candidate / subdir / "policyengine-uk-rust"
            if p.is_file():
                return str(p)
        candidate = candidate.parent
    raise FileNotFoundError(
        "Cannot find policyengine-uk-rust binary. "
        "Install the package (`pip install policyengine-uk-compiled`) "
        "or build from source (`cargo build --release`)."
    )


def _find_cwd(binary_path: str) -> str:
    """Find the working directory that contains parameters/."""
    if (_PKG_DIR / "parameters").is_dir():
        return str(_PKG_DIR)
    binary = Path(binary_path).resolve()
    for ancestor in (binary.parent, binary.parent.parent, binary.parent.parent.parent):
        if (ancestor / "parameters").is_dir():
            return str(ancestor)
    raise FileNotFoundError("Cannot find parameters/ directory.")


def _df_to_csv(df) -> str:
    """Convert a DataFrame to CSV string."""
    return df.to_csv(index=False)


def _build_stdin_payload(persons_csv: str, benunits_csv: str, households_csv: str) -> str:
    """Build the concatenated CSV protocol payload."""
    return (
        "===PERSONS===\n" + persons_csv +
        "===BENUNITS===\n" + benunits_csv +
        "===HOUSEHOLDS===\n" + households_csv
    )


def _parse_microdata_stdout(raw: str) -> MicrodataResult:
    """Parse the concatenated CSV protocol output into a MicrodataResult."""
    sections = {}
    current_name = None
    current_lines = []
    for line in raw.split("\n"):
        if line.startswith("===") and line.endswith("==="):
            if current_name is not None:
                sections[current_name] = "\n".join(current_lines)
            current_name = line.strip("=").lower()
            current_lines = []
        else:
            current_lines.append(line)
    if current_name is not None:
        sections[current_name] = "\n".join(current_lines)
    dfs = {
        name: pd.read_csv(io.StringIO(csv_text))
        for name, csv_text in sections.items()
        if csv_text.strip()
    }
    return MicrodataResult(
        persons=dfs.get("persons", pd.DataFrame()),
        benunits=dfs.get("benunits", pd.DataFrame()),
        households=dfs.get("households", pd.DataFrame()),
    )


class Simulation:
    """Run the PolicyEngine UK microsimulation engine.

    Accepts data via DataFrames (piped to binary stdin), file paths, or
    legacy FRS-specific arguments.

    Usage::

        from policyengine_uk_compiled import Simulation, Parameters, IncomeTaxParams

        # From DataFrames (hypothetical household)
        persons, benunits, households = Simulation.single_person(
            employment_income=50000
        )
        sim = Simulation(year=2025, persons=persons, benunits=benunits,
                         households=households)
        result = sim.run()

        # From a data directory
        sim = Simulation(year=2025, data_dir="data/frs/2023")
        result = sim.run()

        # With a reform
        reform = Parameters(income_tax=IncomeTaxParams(personal_allowance=20000))
        result = sim.run(policy=reform)
    """

    def __init__(
        self,
        year: int = 2025,
        *,
        # Generic data interface
        persons=None,
        benunits=None,
        households=None,
        data_dir: Optional[Union[str, Path]] = None,
        # Legacy FRS interface
        clean_frs_base: Optional[str] = None,
        clean_frs: Optional[str] = None,
        frs_raw: Optional[str] = None,
        binary_path: Optional[str] = None,
    ):
        self.year = year
        self.binary_path = binary_path or _find_binary()

        # Determine data mode
        self._stdin_payload = None
        self._data_dir = None
        self._clean_frs_base = clean_frs_base
        self._clean_frs = clean_frs
        self._frs_raw = frs_raw

        if persons is not None and benunits is not None and households is not None:
            # DataFrame or CSV string mode
            if HAS_PANDAS and hasattr(persons, "to_csv"):
                persons_csv = _df_to_csv(persons)
                benunits_csv = _df_to_csv(benunits)
                households_csv = _df_to_csv(households)
            elif isinstance(persons, str):
                persons_csv = persons
                benunits_csv = benunits
                households_csv = households
            else:
                raise TypeError(
                    "persons/benunits/households must be pandas DataFrames or CSV strings"
                )
            self._stdin_payload = _build_stdin_payload(
                persons_csv, benunits_csv, households_csv
            )
        elif data_dir is not None:
            self._data_dir = str(data_dir)

    def _build_cmd(self, policy: Optional[Parameters] = None, extra_args: Optional[list[str]] = None) -> list[str]:
        cmd = [self.binary_path, "--year", str(self.year)]

        if self._stdin_payload is not None:
            cmd.append("--stdin-data")
        elif self._data_dir:
            cmd += ["--data", self._data_dir]
        elif self._clean_frs_base:
            cmd += ["--data", self._clean_frs_base]
        elif self._clean_frs:
            cmd += ["--data", self._clean_frs]
        elif self._frs_raw:
            cmd += ["--frs", self._frs_raw]
        else:
            # No data source specified — try auto-resolving FRS data
            from policyengine_uk_compiled.data import ensure_frs
            frs_path = ensure_frs(self.year)
            cmd += ["--data", frs_path]

        if policy:
            overlay = policy.model_dump(exclude_none=True)
            if overlay:
                cmd += ["--policy-json", json.dumps(overlay)]

        if extra_args:
            cmd += extra_args

        return cmd

    def run(self, policy: Optional[Parameters] = None, timeout: int = 120) -> SimulationResult:
        """Run the simulation and return typed results.

        Args:
            policy: Reform parameters (overlay on baseline). None = baseline only.
            timeout: Maximum seconds to wait for the binary.

        Returns:
            SimulationResult with budgetary impact, program breakdown, decile impacts, etc.
        """
        cmd = self._build_cmd(policy, extra_args=["--output", "json"])
        cwd = _find_cwd(self.binary_path)
        result = subprocess.run(
            cmd,
            input=self._stdin_payload,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
        )
        if result.returncode != 0:
            raise RuntimeError(
                f"Simulation failed (exit {result.returncode}):\n{result.stderr}"
            )
        data = json.loads(result.stdout)
        return SimulationResult(**data)

    def run_microdata(
        self, policy: Optional[Parameters] = None, timeout: int = 120
    ) -> MicrodataResult:
        """Run the simulation and return per-entity microdata as DataFrames."""
        if not HAS_PANDAS:
            raise ImportError("pandas is required for run_microdata")
        cmd = self._build_cmd(policy, extra_args=["--output-microdata-stdout"])
        cwd = _find_cwd(self.binary_path)
        result = subprocess.run(
            cmd,
            input=self._stdin_payload,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
        )
        if result.returncode != 0:
            raise RuntimeError(
                f"Simulation failed (exit {result.returncode}):\n{result.stderr}"
            )
        return _parse_microdata_stdout(result.stdout)

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

    # ── Convenience constructors for hypothetical households ──────────────

    @staticmethod
    def single_person(
        age: float = 30,
        employment_income: float = 0.0,
        self_employment_income: float = 0.0,
        pension_income: float = 0.0,
        region: str = "London",
        rent_monthly: float = 0.0,
        council_tax_annual: float = 0.0,
        **person_kwargs,
    ):
        """Build a single-person household dataset.

        Returns (persons_df, benunits_df, households_df) tuple.
        """
        if not HAS_PANDAS:
            raise ImportError("pandas is required for DataFrame construction")
        person = {
            **PERSON_DEFAULTS,
            "age": age,
            "employment_income": employment_income,
            "self_employment_income": self_employment_income,
            "private_pension_income": pension_income,
            "is_in_scotland": region == "Scotland",
            **person_kwargs,
        }
        benunit = {
            **BENUNIT_DEFAULTS,
            "rent_monthly": rent_monthly,
        }
        household = {
            **HOUSEHOLD_DEFAULTS,
            "region": region,
            "rent_annual": rent_monthly * 12,
            "council_tax_annual": council_tax_annual,
        }
        return pd.DataFrame([person]), pd.DataFrame([benunit]), pd.DataFrame([household])

    @staticmethod
    def couple(
        ages: tuple[float, float] = (30, 30),
        incomes: tuple[float, float] = (0.0, 0.0),
        children: int = 0,
        child_ages: Optional[list[float]] = None,
        region: str = "London",
        rent_monthly: float = 0.0,
        council_tax_annual: float = 0.0,
    ):
        """Build a couple household, optionally with children.

        Returns (persons_df, benunits_df, households_df) tuple.
        """
        if not HAS_PANDAS:
            raise ImportError("pandas is required for DataFrame construction")

        if child_ages is None:
            child_ages = [10.0] * children
        else:
            children = len(child_ages)

        persons = []
        n_people = 2 + children
        # Adult 1 (head)
        persons.append({
            **PERSON_DEFAULTS,
            "person_id": 0, "age": ages[0],
            "employment_income": incomes[0],
            "is_benunit_head": True, "is_household_head": True,
            "is_in_scotland": region == "Scotland",
        })
        # Adult 2
        persons.append({
            **PERSON_DEFAULTS,
            "person_id": 1, "age": ages[1],
            "employment_income": incomes[1],
            "is_benunit_head": False, "is_household_head": False,
            "is_in_scotland": region == "Scotland",
        })
        # Children
        for i, cage in enumerate(child_ages):
            persons.append({
                **PERSON_DEFAULTS,
                "person_id": 2 + i, "age": cage,
                "gender": "male",
                "is_benunit_head": False, "is_household_head": False,
                "employment_income": 0.0,
                "is_in_scotland": region == "Scotland",
            })

        person_id_str = ";".join(str(i) for i in range(n_people))
        benunit = {
            **BENUNIT_DEFAULTS,
            "person_ids": person_id_str,
            "rent_monthly": rent_monthly,
        }
        household = {
            **HOUSEHOLD_DEFAULTS,
            "benunit_ids": "0",
            "person_ids": person_id_str,
            "region": region,
            "rent_annual": rent_monthly * 12,
            "council_tax_annual": council_tax_annual,
        }
        return pd.DataFrame(persons), pd.DataFrame([benunit]), pd.DataFrame([household])
