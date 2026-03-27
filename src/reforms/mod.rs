use crate::parameters::Parameters;
use std::path::Path;

/// A reform is a named set of parameter overrides applied on top of baseline law.
///
/// Reforms are defined as YAML files that mirror the parameter structure.
/// Only the fields you want to change need to be specified — everything else
/// inherits from the baseline.
///
/// # Example reform file (raise_pa.yaml):
///
/// ```yaml
/// income_tax:
///   personal_allowance: 20000.0
/// ```
///
/// # Structural reforms can modify brackets:
///
/// ```yaml
/// income_tax:
///   personal_allowance: 20000.0
///   uk_brackets:
///     - { rate: 0.20, threshold: 0.0 }
///     - { rate: 0.40, threshold: 37700.0 }
///     - { rate: 0.45, threshold: 125140.0 }
///     - { rate: 0.50, threshold: 250000.0 }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Reform {
    pub name: String,
    pub parameters: Parameters,
}

impl Reform {
    /// Create a reform by overlaying YAML parameter overrides onto baseline.
    pub fn from_yaml(name: &str, yaml_str: &str, baseline: &Parameters) -> anyhow::Result<Self> {
        let parameters = baseline.apply_yaml_overlay(yaml_str)?;
        Ok(Reform {
            name: name.to_string(),
            parameters,
        })
    }

    /// Load reform from a YAML file.
    pub fn from_file(path: &Path, baseline: &Parameters) -> anyhow::Result<Self> {
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("reform")
            .to_string();
        let contents = std::fs::read_to_string(path)?;
        Self::from_yaml(&name, &contents, baseline)
    }

    /// Convenience: create the "PA to £20k" reform.
    pub fn personal_allowance_20k(baseline: &Parameters) -> Self {
        let yaml = "income_tax:\n  personal_allowance: 20000.0\n";
        Self::from_yaml("Personal Allowance to £20,000", yaml, baseline).unwrap()
    }
}
