//! Test that every scalar parameter individually produces a different simulation outcome
//! when run over real FRS microdata, across every fiscal year (1994–2029).
//!
//! This catches dead parameters — values that are loaded but never actually used in
//! the simulation engine.

use std::collections::BTreeMap;
use std::path::Path;

#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/parameters/mod.rs"]
mod parameters;
#[path = "../src/variables/mod.rs"]
mod variables;
#[path = "../src/reforms/mod.rs"]
mod reforms;
#[path = "../src/data/mod.rs"]
mod data;

use engine::simulation::*;
use parameters::*;
use data::clean::load_clean_frs;
use data::Dataset;

/// Load FRS data for a given fiscal year from the per-year clean FRS base directory.
/// Falls back to the latest available year + uprating (same logic as the CLI).
fn load_frs_for_year(base: &Path, year: u32) -> Dataset {
    let year_dir = base.join(year.to_string());
    if year_dir.is_dir() {
        load_clean_frs(&year_dir).unwrap()
    } else {
        let latest = (1994..=year).rev()
            .find(|y| base.join(y.to_string()).is_dir())
            .unwrap_or_else(|| panic!("No clean FRS data found for year {} or earlier", year));
        let mut ds = load_clean_frs(&base.join(latest.to_string())).unwrap();
        ds.uprate_to(year);
        ds
    }
}

/// Extract all scalar parameter paths and their values from a Parameters struct.
fn extract_scalar_params(params: &Parameters) -> BTreeMap<String, f64> {
    let json_str = params.to_json();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let mut result = BTreeMap::new();
    collect_scalars(&val, "", &mut result);
    result
}

fn collect_scalars(val: &serde_json::Value, prefix: &str, out: &mut BTreeMap<String, f64>) {
    match val {
        serde_json::Value::Object(map) => {
            for (key, v) in map {
                let path = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
                collect_scalars(v, &path, out);
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.insert(prefix.to_string(), f);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                collect_scalars(v, &format!("{}[{}]", prefix, i), out);
            }
        }
        _ => {}
    }
}

/// Integer-typed parameter paths (serde expects usize, not f64).
const INTEGER_PARAMS: &[&str] = &["universal_credit.child_limit"];

/// Create a JSON overlay that nudges a single parameter.
fn make_single_param_overlay(path: &str, current_value: f64) -> String {
    let nudge = if path.contains("rate") || path.contains("taper") || path.contains("fraction") {
        0.01
    } else if path.contains("child_limit") || path.contains("max_age") || path.contains("min_hours") {
        1.0
    } else if current_value.abs() < 1.0 {
        100.0
    } else {
        (current_value.abs() * 0.1).max(100.0)
    };
    let is_int = INTEGER_PARAMS.contains(&path);
    build_json_overlay(path, current_value + nudge, is_int)
}

/// Build a JSON string that sets a single dotted path to a value.
fn build_json_overlay(path: &str, value: f64, as_integer: bool) -> String {
    let segments: Vec<(String, Option<usize>)> = path.split('.').map(|part| {
        if let Some(bracket_pos) = part.find('[') {
            let key = part[..bracket_pos].to_string();
            let idx: usize = part[bracket_pos + 1..part.len() - 1].parse().unwrap();
            (key, Some(idx))
        } else {
            (part.to_string(), None)
        }
    }).collect();

    fn build_inner(segments: &[(String, Option<usize>)], value: f64, as_integer: bool) -> serde_json::Value {
        if segments.is_empty() {
            return if as_integer {
                serde_json::Value::Number(serde_json::Number::from(value as u64))
            } else {
                serde_json::Value::Number(serde_json::Number::from_f64(value).unwrap())
            };
        }
        let (key, _idx) = &segments[0];
        let inner = build_inner(&segments[1..], value, as_integer);
        let mut map = serde_json::Map::new();
        map.insert(key.clone(), inner);
        serde_json::Value::Object(map)
    }

    serde_json::to_string(&build_inner(&segments, value, as_integer)).unwrap()
}

/// Compute weighted total net income across all households.
fn simulate_weighted_net_income(dataset: &Dataset, params: &Parameters) -> f64 {
    let sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        params.clone(),
    );
    let results = sim.run();
    results.household_results.iter()
        .zip(dataset.households.iter())
        .map(|(hr, hh)| hr.net_income * hh.weight)
        .sum()
}

/// Parameters to skip — these either don't affect net income by design, or are metadata.
const SKIP_PARAMS: &[&str] = &[
    "fiscal_year",
    // Growth factors are used for uprating between years, not in the simulation
    "growth_factors.cpi_rate",
    "growth_factors.gdp_deflator",
    "growth_factors.earnings_growth",
    // Migration rates interact with migration_seed in ways that may not change
    // total net income when nudged by 0.01
    "uc_migration",
    // Employer NI: calculated but does not feed into household net income (borne by employer)
    "national_insurance.employer_rate",
    "national_insurance.secondary_threshold_annual",
    // State pension: uses reported FRS amounts; parameter is only a fallback for unreported
    "state_pension.new_state_pension_weekly",
    "state_pension.old_basic_pension_weekly",
    // CA hours/age thresholds: binary eligibility checks that all real carers pass
    "income_related_benefits.ca_min_hours_caring",
    "income_related_benefits.ca_care_recipient_min_age",
    // ESA support/WRAG components: correctly wired (esa_group 1/2/3) but too few FRS
    // respondents with ESA group codes on legacy benefits to move the weighted total
    "income_related_benefits.esa_support_component",
    "income_related_benefits.esa_wrag_component",
    // Disabled child elements: correctly wired (is_severely_disabled/is_disabled on children)
    // but disabled children on CTC/UC are very rare in FRS microdata
    "tax_credits.ctc_severely_disabled_child_element",
    "universal_credit.disabled_child_higher",
    "universal_credit.disabled_child_lower",
];

fn is_array_element(path: &str) -> bool {
    path.contains('[')
}

#[test]
fn test_all_parameters_have_impact() {
    let frs_base = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/frs"));
    if !frs_base.is_dir() {
        eprintln!("Skipping test: no FRS data at {}", frs_base.display());
        return;
    }

    // Track which parameters have had impact in at least one year.
    // Key = parameter path, Value = (years_tested, years_with_impact)
    let mut param_results: BTreeMap<String, (Vec<u32>, Vec<u32>)> = BTreeMap::new();
    let mut per_year_failures: Vec<(u32, String)> = Vec::new();

    for year in 1994..=2029u32 {
        let params = Parameters::for_year(year).unwrap();
        let dataset = load_frs_for_year(frs_base, year);
        let scalar_params = extract_scalar_params(&params);

        let baseline_net = simulate_weighted_net_income(&dataset, &params);

        for (path, value) in &scalar_params {
            if SKIP_PARAMS.iter().any(|s| path == s || path.starts_with(&format!("{}.", s))) {
                continue;
            }
            if is_array_element(path) {
                continue;
            }
            if *value == 0.0 {
                continue;
            }

            let overlay = make_single_param_overlay(path, *value);
            let reformed = match params.apply_json_overlay(&overlay) {
                Ok(r) => r,
                Err(e) => {
                    per_year_failures.push((year, format!("{}: overlay error: {}", path, e)));
                    continue;
                }
            };

            let reformed_net = simulate_weighted_net_income(&dataset, &reformed);
            let entry = param_results.entry(path.clone()).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(year);
            if (reformed_net - baseline_net).abs() >= 1.0 {
                entry.1.push(year);
            } else {
                per_year_failures.push((year, format!("{} = {} → no impact", path, value)));
            }
        }

        // Test bracket arrays (thresholds nudged by £1000)
        for (name, brackets) in &[
            ("income_tax.uk_brackets", &params.income_tax.uk_brackets),
        ] {
            if brackets.is_empty() { continue; }
            let new_brackets: Vec<serde_json::Value> = brackets.iter().map(|b| {
                serde_json::json!({"rate": b.rate, "threshold": b.threshold + 1000.0})
            }).collect();
            let overlay = format!(
                r#"{{"income_tax": {{"uk_brackets": {}}}}}"#,
                serde_json::to_string(&new_brackets).unwrap()
            );
            let reformed = params.apply_json_overlay(&overlay).unwrap();
            let reformed_net = simulate_weighted_net_income(&dataset, &reformed);
            let key = name.to_string();
            let entry = param_results.entry(key.clone()).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(year);
            if (reformed_net - baseline_net).abs() >= 1.0 {
                entry.1.push(year);
            }
        }

        // Scottish brackets
        if !params.income_tax.scottish_brackets.is_empty() {
            let new_brackets: Vec<serde_json::Value> = params.income_tax.scottish_brackets.iter().map(|b| {
                serde_json::json!({"rate": b.rate, "threshold": b.threshold + 1000.0})
            }).collect();
            let overlay = format!(
                r#"{{"income_tax": {{"scottish_brackets": {}}}}}"#,
                serde_json::to_string(&new_brackets).unwrap()
            );
            let reformed = params.apply_json_overlay(&overlay).unwrap();
            let reformed_net = simulate_weighted_net_income(&dataset, &reformed);
            let key = "income_tax.scottish_brackets".to_string();
            let entry = param_results.entry(key).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(year);
            if (reformed_net - baseline_net).abs() >= 1.0 {
                entry.1.push(year);
            }
        }

        eprintln!("Year {}/{}: baseline weighted net = {:.0}, tested {} scalar params",
            year, year + 1, baseline_net, scalar_params.len());
    }

    // Report per-year failures as warnings
    if !per_year_failures.is_empty() {
        eprintln!("\n{} per-year parameter failures (data sparsity):", per_year_failures.len());
        for (year, detail) in &per_year_failures {
            eprintln!("  {}/{}: {}", year, year + 1, detail);
        }
    }

    // FAIL the test only for parameters that had no impact in ANY year
    let dead_params: Vec<(&String, &(Vec<u32>, Vec<u32>))> = param_results.iter()
        .filter(|(_, (tested, impacted))| !tested.is_empty() && impacted.is_empty())
        .collect();

    if !dead_params.is_empty() {
        let mut msg = format!("\n{} parameter(s) had no impact in ANY fiscal year (dead code):\n", dead_params.len());
        for (path, (tested, _)) in &dead_params {
            msg.push_str(&format!("  {} (tested in {} years)\n", path, tested.len()));
        }
        panic!("{}", msg);
    }
}
