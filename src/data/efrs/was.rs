use std::path::Path;
use crate::data::Dataset;
use crate::data::frs::{load_table_cols, get_f64, get_i64};
use crate::engine::entities::*;
use super::rf;

// WAS Round 7 column names are all lowercased during load_table_cols.
// Key predictor columns: dvtotinc_bhcr7, numadultr7, numch18r7, etc.
// Key target columns: dvlukval_r7, dvhvaluer7, dvfnsvalr7_sum, numcarsr7, etc.

/// WAS region codes (gorr7) → Region enum.
fn was_region(code: i64) -> Region {
    match code {
        1 => Region::NorthEast,
        2 => Region::NorthWest,
        4 => Region::Yorkshire,
        5 => Region::EastMidlands,
        6 => Region::WestMidlands,
        7 => Region::EastOfEngland,
        8 => Region::London,
        9 => Region::SouthEast,
        10 => Region::SouthWest,
        11 => Region::Wales,
        12 => Region::Scotland,
        // WAS doesn't oversample NI; map to Wales as fallback
        _ => Region::Wales,
    }
}

/// Target variable names in the order we train models.
pub const WEALTH_TARGETS: &[&str] = &[
    "owned_land",
    "property_wealth",
    "corporate_wealth",
    "gross_financial_wealth",
    "net_financial_wealth",
    "main_residence_value",
    "other_residential_property_value",
    "non_residential_property_value",
    "savings",
    "num_vehicles",
];

/// Build the WAS training data: (features, targets).
/// Returns (feature_rows, target_columns) where each target column is (name, values).
fn build_was_training_data(
    was_dir: &Path,
) -> anyhow::Result<(Vec<Vec<f64>>, Vec<(&'static str, Vec<f64>)>)> {
    // Try multiple possible filenames for WAS Round 7
    let table = load_table_cols(was_dir, "was_round_7_hhold_eul_march_2022", None)
        .or_else(|_| load_table_cols(was_dir, "was_round_7_hhold_eul", None))
        .or_else(|_| {
            // Try loading any .tab file in the directory
            let mut found = None;
            if let Ok(entries) = std::fs::read_dir(was_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("was") && (name.ends_with(".tab") || name.ends_with(".csv")) {
                        let stem = name.trim_end_matches(".tab").trim_end_matches(".csv").to_string();
                        found = Some(stem);
                        break;
                    }
                }
            }
            match found {
                Some(stem) => load_table_cols(was_dir, &stem, None),
                None => anyhow::bail!("No WAS data file found in {:?}", was_dir),
            }
        })?;

    eprintln!("  Loaded {} WAS households", table.len());

    let mut features: Vec<Vec<f64>> = Vec::with_capacity(table.len());
    let mut targets: Vec<Vec<f64>> = vec![Vec::with_capacity(table.len()); WEALTH_TARGETS.len()];

    for row in &table {
        // Predictors (11 features)
        let hh_income = get_f64(row, "dvtotinc_bhcr7");
        let num_adults = get_f64(row, "numadultr7");
        let num_children = get_f64(row, "numch18r7");
        let pension_income = get_f64(row, "dvgippenr7_aggr");
        let emp_income = get_f64(row, "dvgiempr7_aggr");
        let se_income = get_f64(row, "dvgiser7_aggr");
        let capital_income = get_f64(row, "dvgiinvr7_aggr");
        let bedrooms = get_f64(row, "hbedrmr7");
        let council_tax = get_f64(row, "dvctaxamtannualr7");
        let is_renting = if get_i64(row, "dvprirntr7") == 1 { 1.0 } else { 0.0 };
        let region = was_region(get_i64(row, "gorr7")).to_rf_code();

        features.push(vec![
            hh_income, num_adults, num_children,
            pension_income, emp_income, se_income, capital_income,
            bedrooms, council_tax, is_renting, region,
        ]);

        // Targets
        let owned_land = get_f64(row, "dvlukvalr7_sum").max(0.0);
        let main_res = get_f64(row, "dvhvaluer7").max(0.0);
        let other_res = get_f64(row, "dvhsevalr7_sum").max(0.0);
        let non_res = get_f64(row, "dvbldvalr7_sum").max(0.0);
        let property_wealth = main_res + other_res + non_res + owned_land;

        // Corporate wealth: shares + ISAs + non-DB pensions
        let shares = get_f64(row, "dvfesharesr7_aggr").max(0.0);
        let isas = get_f64(row, "dvisavalr7_aggr").max(0.0);
        let total_pensions = get_f64(row, "totpenr7_aggr").max(0.0);
        let corporate_wealth = shares + isas + total_pensions;

        let gross_financial = get_f64(row, "dvfnsvalr7_aggr").max(0.0) + corporate_wealth;
        let net_financial = get_f64(row, "dvfnsvalr7_aggr"); // can be negative
        let savings = get_f64(row, "dvsavalr7_aggr").max(0.0);
        let num_vehicles = get_f64(row, "vcarnr7").max(0.0);

        targets[0].push(owned_land);
        targets[1].push(property_wealth);
        targets[2].push(corporate_wealth);
        targets[3].push(gross_financial);
        targets[4].push(net_financial);
        targets[5].push(main_res);
        targets[6].push(other_res);
        targets[7].push(non_res);
        targets[8].push(savings);
        targets[9].push(num_vehicles);
    }

    let named_targets: Vec<(&str, Vec<f64>)> = WEALTH_TARGETS
        .iter()
        .zip(targets)
        .map(|(name, vals)| (*name, vals))
        .collect();

    Ok((features, named_targets))
}

/// Build the FRS predictor matrix for wealth imputation.
/// Returns one row per household, in dataset household order.
pub fn build_frs_wealth_features(dataset: &Dataset) -> Vec<Vec<f64>> {
    dataset.households.iter().map(|hh| {
        let people: Vec<&Person> = hh.person_ids.iter().map(|&pid| &dataset.people[pid]).collect();
        let num_adults = people.iter().filter(|p| p.is_adult()).count() as f64;
        let num_children = people.iter().filter(|p| p.is_child()).count() as f64;
        let emp_income: f64 = people.iter().map(|p| p.employment_income).sum();
        let se_income: f64 = people.iter().map(|p| p.self_employment_income).sum();
        let pension_income: f64 = people.iter().map(|p| p.pension_income).sum();
        let capital_income: f64 = people.iter().map(|p| p.savings_interest_income + p.dividend_income).sum();
        let hh_income: f64 = people.iter().map(|p| p.total_income()).sum();
        let is_renting = if hh.tenure_type.is_renting() { 1.0 } else { 0.0 };

        vec![
            hh_income, num_adults, num_children,
            pension_income, emp_income, se_income, capital_income,
            hh.num_bedrooms as f64, hh.council_tax, is_renting,
            hh.region.to_rf_code(),
        ]
    }).collect()
}

/// Run the full WAS imputation pipeline: train on WAS, predict on FRS.
pub fn impute_wealth(
    dataset: &mut Dataset,
    was_dir: &Path,
) -> anyhow::Result<()> {
    eprintln!("  Training wealth models from WAS...");
    let (train_features, train_targets) = build_was_training_data(was_dir)?;

    let models = rf::train_multi_target(&train_features, &train_targets, 50, 42)?;
    eprintln!("  Trained {} wealth RF models", models.len());

    let frs_features = build_frs_wealth_features(dataset);
    let predictions = rf::predict_multi_target(&models, &frs_features)?;

    for (name, preds) in &predictions {
        for (i, &val) in preds.iter().enumerate() {
            let hh = &mut dataset.households[i];
            match name.as_str() {
                "owned_land" => hh.owned_land = val.max(0.0),
                "property_wealth" => hh.property_wealth = val.max(0.0),
                "corporate_wealth" => hh.corporate_wealth = val.max(0.0),
                "gross_financial_wealth" => hh.gross_financial_wealth = val.max(0.0),
                "net_financial_wealth" => hh.net_financial_wealth = val, // can be negative
                "main_residence_value" => hh.main_residence_value = val.max(0.0),
                "other_residential_property_value" => hh.other_residential_property_value = val.max(0.0),
                "non_residential_property_value" => hh.non_residential_property_value = val.max(0.0),
                "savings" => hh.savings = val.max(0.0),
                "num_vehicles" => hh.num_vehicles = val.max(0.0).round(),
                _ => {}
            }
        }
    }

    let mean_property: f64 = dataset.households.iter()
        .map(|h| h.weight * h.property_wealth)
        .sum::<f64>()
        / dataset.households.iter().map(|h| h.weight).sum::<f64>();
    eprintln!("  Wealth imputation complete. Mean property wealth: £{:.0}", mean_property);

    Ok(())
}
