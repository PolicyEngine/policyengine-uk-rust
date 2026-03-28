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
    #[serde(default)]
    pub benefit_cap: Option<BenefitCapParams>,
    #[serde(default)]
    pub housing_benefit: Option<HousingBenefitParams>,
    #[serde(default)]
    pub tax_credits: Option<TaxCreditsParams>,
    #[serde(default)]
    pub scottish_child_payment: Option<ScottishChildPaymentParams>,
    #[serde(default = "TakeUpRates::default")]
    pub take_up: TakeUpRates,
    #[serde(default = "UcMigrationRates::default")]
    pub uc_migration: UcMigrationRates,
    /// Disability premiums for IS/HB/ESA applicable amounts.
    /// Source: Income Support (General) Regs 1987 (SI 1987/1967) Sch.2
    #[serde(default)]
    pub disability_premiums: Option<DisabilityPremiumParams>,
    /// Income-related benefits: ESA(IR), JSA(IB), Carers Allowance.
    #[serde(default)]
    pub income_related_benefits: Option<IncomeRelatedBenefitParams>,
    /// Baseline mode: if true, benefits are only awarded to reported claimants.
    /// Take-up rates and ENR logic are disabled. Set to false for reform simulations.
    #[serde(default = "Parameters::default_baseline_mode", skip_serializing)]
    pub baseline_mode: bool,
}

impl Parameters {
    fn default_baseline_mode() -> bool { true }
}

/// Take-up rates for means-tested benefits.
///
/// Legacy benefits (HB, CTC, WTC, IS) are received only by reported claimants —
/// no new entrants to the legacy system under current policy. Their take-up
/// rates are therefore not modelled here.
///
/// Source: DWP Income-Related Benefits Estimates of Take-Up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeUpRates {
    pub universal_credit: f64,
    pub child_benefit: f64,
    pub pension_credit: f64,
    /// Take-up rate for genuinely new entrants (not ENRs) when a reform expands
    /// UC eligibility. Models partial behavioural response to new entitlement.
    #[serde(default = "TakeUpRates::default_new_entrant_rate")]
    pub new_entrant_rate: f64,
}

impl Default for TakeUpRates {
    fn default() -> Self {
        TakeUpRates {
            universal_credit: 0.80,
            child_benefit: 0.93,
            pension_credit: 0.63,
            new_entrant_rate: 0.3,
        }
    }
}

impl TakeUpRates {
    fn default_new_entrant_rate() -> f64 { 0.3 }
}

/// UC managed migration rates by legacy benefit type.
/// Fraction of legacy claimants who have been migrated to UC by the modelled year.
/// Pensioner HB is always 0 (pensioners are ineligible for UC).
/// Source: DWP UC managed migration statistics, extrapolated to 2025/26.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UcMigrationRates {
    /// Working-age HB claimants migrated to UC
    pub housing_benefit: f64,
    /// CTC/WTC claimants migrated to UC
    pub tax_credits: f64,
    /// Income support claimants migrated to UC
    pub income_support: f64,
}

impl Default for UcMigrationRates {
    fn default() -> Self {
        // Year-specific values are set in parameters/<year>.yaml
        UcMigrationRates {
            housing_benefit: 0.0,
            tax_credits: 0.0,
            income_support: 0.0,
        }
    }
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
    /// Marriage Allowance: fraction of PA transferable (default 10%)
    #[serde(default = "default_ma_fraction")]
    pub marriage_allowance_max_fraction: f64,
    /// Rounding increment for marriage allowance (default £10)
    #[serde(default = "default_ma_rounding")]
    pub marriage_allowance_rounding: f64,
}

fn default_ma_fraction() -> f64 { 0.10 }
fn default_ma_rounding() -> f64 { 10.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NationalInsuranceParams {
    // Class 1 employee (primary)
    pub primary_threshold_annual: f64,
    pub upper_earnings_limit_annual: f64,
    pub main_rate: f64,
    pub additional_rate: f64,
    // Class 1 employer (secondary)
    #[serde(default = "default_secondary_threshold")]
    pub secondary_threshold_annual: f64,
    #[serde(default = "default_employer_rate")]
    pub employer_rate: f64,
    // Class 2 (self-employed flat rate)
    #[serde(default = "default_class2_flat_rate")]
    pub class2_flat_rate_weekly: f64,
    #[serde(default = "default_class2_spt")]
    pub class2_small_profits_threshold: f64,
    // Class 4 (self-employed)
    pub class4_lower_profits_limit: f64,
    pub class4_upper_profits_limit: f64,
    pub class4_main_rate: f64,
    pub class4_additional_rate: f64,
}

fn default_secondary_threshold() -> f64 { 5000.0 }
fn default_employer_rate() -> f64 { 0.15 }
// Class 2 abolished from 6 April 2024 (NIC Act 2024); default to 0 for post-2024 years
fn default_class2_flat_rate() -> f64 { 0.0 }
fn default_class2_spt() -> f64 { 0.0 }

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
pub struct BenefitCapParams {
    pub single_london: f64,
    pub single_outside_london: f64,
    pub non_single_london: f64,
    pub non_single_outside_london: f64,
    /// Net earned income threshold for exemption (annual)
    pub earnings_exemption_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HousingBenefitParams {
    /// Taper/withdrawal rate (65%)
    pub withdrawal_rate: f64,
    /// Personal allowances for applicable amount (weekly)
    pub personal_allowance_single_under25: f64,
    pub personal_allowance_single_25_plus: f64,
    pub personal_allowance_couple: f64,
    pub child_allowance: f64,
    pub family_premium: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxCreditsParams {
    // WTC elements (annual)
    pub wtc_basic_element: f64,
    pub wtc_couple_element: f64,
    pub wtc_lone_parent_element: f64,
    pub wtc_30_hour_element: f64,
    // CTC elements (annual)
    pub ctc_child_element: f64,
    pub ctc_family_element: f64,
    pub ctc_disabled_child_element: f64,
    pub ctc_severely_disabled_child_element: f64,
    // Income thresholds and taper
    pub income_threshold: f64,
    pub taper_rate: f64,
    /// Minimum hours per week to qualify for WTC
    pub wtc_min_hours_single: f64,
    pub wtc_min_hours_couple: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScottishChildPaymentParams {
    /// Weekly amount per eligible child
    pub weekly_amount: f64,
    /// Maximum age of child
    pub max_age: f64,
}

/// Disability premiums added to IS/HB/ESA applicable amounts.
///
/// Source: Income Support (General) Regs 1987 (SI 1987/1967) Sch.2 paras 11-14,
/// as uprated annually by the Social Security Benefits Up-rating Orders.
/// All amounts are weekly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisabilityPremiumParams {
    /// Disability Premium (DP): person has lower-rate PIP/DLA or is in WRAG.
    /// Sch.2 para.11: £42.50/wk (single), £60.60/wk (couple) in 2025/26.
    pub disability_premium_single: f64,
    pub disability_premium_couple: f64,
    /// Enhanced Disability Premium (EDP): highest-rate DLA care or enhanced PIP DL.
    /// Sch.2 para.13: £27.90/wk (single), £39.85/wk (couple) in 2025/26.
    pub enhanced_disability_premium_single: f64,
    pub enhanced_disability_premium_couple: f64,
    /// Severe Disability Premium (SDP): enhanced PIP DL/highest DLA care, lives alone,
    /// no carer receiving CA. Sch.2 para.14: £81.50/wk in 2025/26.
    pub severe_disability_premium: f64,
    /// Carer Premium: for persons receiving CA. Sch.2 para.14D: £46.00/wk in 2025/26.
    pub carer_premium: f64,
}

/// Income-related benefits: ESA(IR), JSA(IB), Carers Allowance.
///
/// ESA(IR) is structurally Income Support + a work-related component.
/// JSA(IB) is structurally Income Support conditioned on availability for work.
///
/// Sources:
///   - Welfare Reform Act 2007 c.5 s.2-4 (ESA)
///   - Employment and Support Allowance Regs 2008 (SI 2008/794)
///   - Jobseekers Act 1995 c.18 s.4-5 (JSA)
///   - Social Security Contributions and Benefits Act 1992 s.70 (CA)
///   - SS (Carers Allowance) Regs 2002 (SI 2002/2690)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeRelatedBenefitParams {
    /// ESA personal allowances (weekly) — same as IS personal allowances.
    /// ESA Regs 2008 reg.67 / Sch.4.
    pub esa_allowance_single_under25: f64,
    pub esa_allowance_single_25_plus: f64,
    pub esa_allowance_couple: f64,
    /// ESA work-related activity component (WRAG): SI 2008/794 reg.67 Sch.4 col.2.
    /// £35.95/wk in 2025/26.
    pub esa_wrag_component: f64,
    /// ESA support component (support group): SI 2008/794 reg.67 Sch.4 col.2.
    /// £48.95/wk in 2025/26.
    pub esa_support_component: f64,
    /// JSA(IB) personal allowances (weekly) — same as IS.
    /// Jobseeker's Allowance Regs 1996 (SI 1996/207) Sch.1.
    pub jsa_allowance_single_under25: f64,
    pub jsa_allowance_single_25_plus: f64,
    pub jsa_allowance_couple: f64,
    /// Carers Allowance: weekly flat rate.
    /// SSCBA 1992 s.70(1); SS (CA) Regs 2002 reg.4.
    /// £81.90/wk in 2025/26.
    pub carers_allowance_weekly: f64,
    /// CA earnings disregard: SS (CA) Regs 2002 reg.8.
    /// £151.00/wk net earnings after deductions in 2025/26.
    pub ca_earnings_disregard_weekly: f64,
    /// Minimum hours of caring per week to qualify for CA.
    /// SSCBA 1992 s.70(1)(b): 35 hours/week.
    pub ca_min_hours_caring: f64,
    /// Minimum age of care recipient for CA: 16.
    pub ca_care_recipient_min_age: f64,
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

    /// Serialise parameters to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Apply a JSON overlay (reform) on top of these parameters.
    pub fn apply_json_overlay(&self, json_str: &str) -> anyhow::Result<Self> {
        let json_val: serde_json::Value = serde_json::from_str(json_str)?;
        let yaml_str = serde_yaml::to_string(&json_val)?;
        self.apply_yaml_overlay(&yaml_str)
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
        // 1994/95 through 2029/30
        (1994..=2029).collect()
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
    fn test_load_all_years() {
        for year in 1994..=2029 {
            let params = Parameters::for_year(year);
            assert!(params.is_ok(), "Failed to load parameters for {}/{}", year, year + 1);
        }
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
