use serde::{Deserialize, Serialize};
use std::path::Path;

/// UK tax-benefit system parameters for a given fiscal year.
///
/// All monetary values are annual unless noted otherwise.
/// UC amounts are monthly (per assessment period) as in legislation.
/// Child benefit and state pension are weekly as in legislation.
///
/// Sources: UK legislation via Lex API, OBR March 2026 EFO for growth factors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    pub fiscal_year: String,
    pub income_tax: IncomeTaxParams,
    pub national_insurance: NationalInsuranceParams,
    pub universal_credit: UniversalCreditParams,
    pub child_benefit: ChildBenefitParams,
    pub state_pension: StatePensionParams,
    pub pension_credit: PensionCreditParams,
    pub growth_factors: GrowthFactors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxBracket {
    pub rate: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeTaxParams {
    pub personal_allowance: f64,
    pub pa_taper_threshold: f64,
    pub pa_taper_rate: f64,
    pub uk_brackets: Vec<TaxBracket>,
    pub scottish_brackets: Vec<TaxBracket>,
    pub dividend_allowance: f64,
    pub dividend_basic_rate: f64,
    pub dividend_higher_rate: f64,
    pub dividend_additional_rate: f64,
    pub savings_starter_rate_band: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NationalInsuranceParams {
    pub primary_threshold_annual: f64,
    pub upper_earnings_limit_annual: f64,
    pub main_rate: f64,
    pub additional_rate: f64,
    // Class 4 (self-employed)
    pub class4_lower_profits_limit: f64,
    pub class4_upper_profits_limit: f64,
    pub class4_main_rate: f64,
    pub class4_additional_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalCreditParams {
    /// Monthly standard allowance amounts (per assessment period)
    pub standard_allowance_single_under25: f64,
    pub standard_allowance_single_over25: f64,
    pub standard_allowance_couple_under25: f64,
    pub standard_allowance_couple_over25: f64,
    /// Monthly child element amounts
    pub child_element_first: f64,
    pub child_element_subsequent: f64,
    pub disabled_child_lower: f64,
    pub disabled_child_higher: f64,
    /// LCWRA and carer elements (monthly)
    pub lcwra_element: f64,
    pub carer_element: f64,
    /// Taper rate and work allowances (monthly)
    pub taper_rate: f64,
    pub work_allowance_higher: f64,
    pub work_allowance_lower: f64,
    pub child_limit: usize,
    pub housing_cost_contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildBenefitParams {
    /// Weekly rates
    pub eldest_weekly: f64,
    pub additional_weekly: f64,
    pub hicbc_threshold: f64,
    pub hicbc_taper_end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePensionParams {
    /// Weekly rates
    pub new_state_pension_weekly: f64,
    pub old_basic_pension_weekly: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PensionCreditParams {
    /// Weekly rates
    pub standard_minimum_single: f64,
    pub standard_minimum_couple: f64,
    pub savings_credit_threshold_single: f64,
    pub savings_credit_threshold_couple: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthFactors {
    pub cpi_rate: f64,
    pub gdp_deflator: f64,
    pub earnings_growth: f64,
}

/// Convert a fiscal year start year (e.g. 2029) to the YAML filename format
fn fiscal_year_filename(year: u32) -> String {
    format!("{}_{:02}.yaml", year, (year + 1) % 100)
}

impl Parameters {
    /// Load parameters for a given fiscal year from the embedded YAML files.
    /// `year` is the start year of the fiscal year, e.g. 2029 for FY 2029/30.
    pub fn for_year(year: u32) -> anyhow::Result<Self> {
        let filename = fiscal_year_filename(year);

        // Try loading from the parameters/ directory relative to the executable,
        // then from cargo manifest dir (for development)
        let paths_to_try = vec![
            format!("parameters/{}", filename),
            format!("{}/parameters/{}", env!("CARGO_MANIFEST_DIR"), filename),
        ];

        for path_str in &paths_to_try {
            let path = Path::new(path_str);
            if path.exists() {
                let contents = std::fs::read_to_string(path)?;
                let params: Parameters = serde_yaml::from_str(&contents)?;
                return Ok(params);
            }
        }

        anyhow::bail!(
            "No parameter file found for fiscal year {}/{}. Looked for: {}",
            year, year + 1, paths_to_try.join(", ")
        )
    }

    /// Load parameters from a YAML string.
    #[allow(dead_code)]
    pub fn from_yaml(yaml_str: &str) -> anyhow::Result<Self> {
        let params: Parameters = serde_yaml::from_str(yaml_str)?;
        Ok(params)
    }

    /// Serialise parameters to YAML for human-readable reform files.
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }

    /// Apply a YAML overlay (reform) on top of these parameters.
    /// Only the fields specified in the overlay are changed.
    pub fn apply_yaml_overlay(&self, overlay_yaml: &str) -> anyhow::Result<Self> {
        let base_value = serde_yaml::to_value(self)?;
        let overlay_value: serde_yaml::Value = serde_yaml::from_str(overlay_yaml)?;
        let merged = merge_yaml(base_value, &overlay_value);
        let merged_params: Parameters = serde_yaml::from_value(merged)?;
        Ok(merged_params)
    }

    /// Available fiscal years (hardcoded list of embedded parameter files).
    #[allow(dead_code)]
    pub fn available_years() -> Vec<u32> {
        vec![2023, 2024, 2025, 2026, 2027, 2028, 2029]
    }
}

/// Deep-merge two YAML values. `overlay` wins on conflict.
fn merge_yaml(mut base: serde_yaml::Value, overlay: &serde_yaml::Value) -> serde_yaml::Value {
    match (&mut base, overlay) {
        (serde_yaml::Value::Mapping(base_map), serde_yaml::Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                if let Some(base_val) = base_map.get(key).cloned() {
                    let merged = merge_yaml(base_val, overlay_val);
                    base_map.insert(key.clone(), merged);
                } else {
                    base_map.insert(key.clone(), overlay_val.clone());
                }
            }
            base
        }
        (_, overlay) => overlay.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_2025_26() {
        let params = Parameters::for_year(2025).unwrap();
        assert_eq!(params.fiscal_year, "2025/26");
        assert!((params.income_tax.personal_allowance - 12570.0).abs() < 0.01);
        assert!((params.national_insurance.main_rate - 0.08).abs() < 0.001);
    }

    #[test]
    fn test_load_2029_30() {
        let params = Parameters::for_year(2029).unwrap();
        assert_eq!(params.fiscal_year, "2029/30");
        assert!(params.income_tax.personal_allowance > 12570.0); // Should be uprated
    }

    #[test]
    fn test_yaml_overlay() {
        let base = Parameters::for_year(2025).unwrap();
        let overlay = "income_tax:\n  personal_allowance: 20000.0\n";
        let reformed = base.apply_yaml_overlay(overlay).unwrap();
        assert!((reformed.income_tax.personal_allowance - 20000.0).abs() < 0.01);
        // Other values should be unchanged
        assert!((reformed.national_insurance.main_rate - 0.08).abs() < 0.001);
    }
}
