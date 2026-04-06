use std::collections::HashMap;
use std::path::Path;
use rand::Rng;
use crate::data::Dataset;
use crate::data::frs::{load_table_cols, get_f64, get_i64};
use crate::engine::entities::*;
use super::rf;
use super::calibrate;

/// LCFS consumption target names in model order.
pub const CONSUMPTION_TARGETS: &[&str] = &[
    "food_consumption",
    "alcohol_consumption",
    "tobacco_consumption",
    "clothing_consumption",
    "housing_water_electricity_consumption",
    "furnishings_consumption",
    "health_consumption",
    "transport_consumption",
    "communication_consumption",
    "recreation_consumption",
    "education_consumption",
    "restaurants_consumption",
    "miscellaneous_consumption",
    "petrol_spending",
    "diesel_spending",
    "domestic_energy_consumption",
    "electricity_consumption",
    "gas_consumption",
];

/// Map LCFS Gorx region code to Region.
fn lcfs_region(code: i64) -> Region {
    match code {
        1 => Region::NorthEast,
        2 => Region::NorthWest,
        3 => Region::Yorkshire,
        4 => Region::EastMidlands,
        5 => Region::WestMidlands,
        6 => Region::EastOfEngland,
        7 => Region::London,
        8 => Region::SouthEast,
        9 => Region::SouthWest,
        10 => Region::Wales,
        11 => Region::Scotland,
        12 => Region::NorthernIreland,
        _ => Region::London,
    }
}

/// Map LCFS tenure code (A122) to TenureType.
fn lcfs_tenure(code: i64) -> TenureType {
    match code {
        1 => TenureType::RentFromCouncil,
        2 => TenureType::RentFromHA,
        3 => TenureType::RentPrivately,
        4 => TenureType::RentPrivately,  // rent-free
        5 => TenureType::OwnedWithMortgage,
        6 => TenureType::OwnedWithMortgage, // shared ownership
        7 => TenureType::OwnedOutright,
        _ => TenureType::Other,
    }
}

/// Map LCFS accommodation code (A121) to AccommodationType.
fn lcfs_accommodation(code: i64) -> AccommodationType {
    match code {
        1 => AccommodationType::HouseDetached,
        2 => AccommodationType::HouseSemiDetached,
        3 => AccommodationType::HouseTerraced,
        4 | 5 => AccommodationType::Flat,
        6 => AccommodationType::Mobile,
        _ => AccommodationType::Other,
    }
}

/// An LCFS household row with all needed fields.
struct LcfsHousehold {
    // Predictors
    num_adults: f64,
    num_children: f64,
    region: Region,
    employment_income: f64,
    self_employment_income: f64,
    private_pension_income: f64,
    hbai_net_income: f64,
    tenure_type: TenureType,
    accommodation_type: AccommodationType,
    has_fuel_consumption: f64,
    // Consumption targets (annual)
    food: f64,
    alcohol: f64,
    tobacco: f64,
    clothing: f64,
    housing_water_electricity: f64,
    furnishings: f64,
    health: f64,
    transport: f64,
    communication: f64,
    recreation: f64,
    education: f64,
    restaurants: f64,
    miscellaneous: f64,
    petrol: f64,
    diesel: f64,
    domestic_energy: f64,
    electricity: f64,
    gas: f64,
}

/// Derive electricity and gas consumption from LCFS interview variables.
/// Ports the Python _derive_energy_from_lcfs hierarchy.
fn derive_energy(
    b226: f64,  // electricity DD/quarterly payment (weekly)
    b489: f64,  // total energy PPM payment (weekly)
    b490: f64,  // gas PPM payment (weekly)
    p537: f64,  // aggregate domestic energy (weekly)
    mean_elec_share: f64, // mean electricity share for fallback
) -> (f64, f64) {
    if b226 > 0.0 {
        // Case 1: direct-debit billed electricity
        let elec = b226;
        let gas = (p537 - b226).max(0.0);
        (elec, gas)
    } else if b489 > 0.0 && b490 > 0.0 {
        // Case 2: both fuels on PPM meters
        let elec = (b489 - b490).max(0.0);
        let gas = b490;
        (elec, gas)
    } else if b489 > 0.0 {
        // Case 3: electricity PPM only
        let elec = b489 * mean_elec_share;
        let gas = b489 * (1.0 - mean_elec_share);
        (elec, gas)
    } else {
        // Case 4: fallback — split by mean share
        let elec = p537 * mean_elec_share;
        let gas = p537 * (1.0 - mean_elec_share);
        (elec, gas)
    }
}

/// Build LCFS training data.
fn build_lcfs_training_data(
    lcfs_dir: &Path,
) -> anyhow::Result<Vec<LcfsHousehold>> {
    // Load household-level data
    let hh_table = load_table_cols(lcfs_dir, "lcfs_2021_dvhh_ukanon", None)
        .or_else(|_| {
            // Try to find any LCFS household file
            let mut found = None;
            if let Ok(entries) = std::fs::read_dir(lcfs_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("dvhh") && (name.ends_with(".tab") || name.ends_with(".csv")) {
                        let stem = name.trim_end_matches(".tab").trim_end_matches(".csv").to_string();
                        found = Some(stem);
                        break;
                    }
                }
            }
            match found {
                Some(stem) => load_table_cols(lcfs_dir, &stem, None),
                None => anyhow::bail!("No LCFS household file found in {:?}", lcfs_dir),
            }
        })?;

    // Load person-level data
    let per_table = load_table_cols(lcfs_dir, "lcfs_2021_dvper_ukanon202122", None)
        .or_else(|_| {
            let mut found = None;
            if let Ok(entries) = std::fs::read_dir(lcfs_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.contains("dvper") && (name.ends_with(".tab") || name.ends_with(".csv")) {
                        let stem = name.trim_end_matches(".tab").trim_end_matches(".csv").to_string();
                        found = Some(stem);
                        break;
                    }
                }
            }
            match found {
                Some(stem) => load_table_cols(lcfs_dir, &stem, None),
                None => anyhow::bail!("No LCFS person file found in {:?}", lcfs_dir),
            }
        })?;

    eprintln!("  Loaded {} LCFS households, {} persons", hh_table.len(), per_table.len());

    // Aggregate person-level incomes to household level
    let mut per_income: HashMap<String, (f64, f64, f64, f64, usize, usize)> = HashMap::new();
    for row in &per_table {
        let case = row.get("case").cloned().unwrap_or_default();
        let emp = get_f64(row, "b303p");
        let se = get_f64(row, "b3262p");
        let sp = get_f64(row, "b3381");
        let pp = get_f64(row, "p049p");
        let age = get_f64(row, "a005p");
        let is_adult = if age >= 18.0 { 1 } else { 0 };
        let is_child = if age < 18.0 { 1 } else { 0 };

        let entry = per_income.entry(case).or_insert((0.0, 0.0, 0.0, 0.0, 0, 0));
        entry.0 += emp;
        entry.1 += se;
        entry.2 += sp;
        entry.3 += pp;
        entry.4 += is_adult;
        entry.5 += is_child;
    }

    // Compute mean electricity share from DD-billed households
    let mut elec_shares: Vec<f64> = Vec::new();
    for row in &hh_table {
        let b226 = get_f64(row, "b226");
        let p537 = get_f64(row, "p537");
        if b226 > 0.0 && p537 > 0.0 {
            elec_shares.push(b226 / p537);
        }
    }
    let mean_elec_share = if elec_shares.is_empty() {
        0.52
    } else {
        elec_shares.iter().sum::<f64>() / elec_shares.len() as f64
    };

    // Build household records
    let mut households = Vec::with_capacity(hh_table.len());
    for row in &hh_table {
        let case = row.get("case").cloned().unwrap_or_default();
        let (emp, se, _sp, pp, adults, children) = per_income
            .get(&case)
            .copied()
            .unwrap_or((0.0, 0.0, 0.0, 0.0, 1, 0));

        let hbai_income = get_f64(row, "p389p") * 52.0;
        let region = lcfs_region(get_i64(row, "gorx"));
        let tenure = lcfs_tenure(get_i64(row, "a122"));
        let accomm = lcfs_accommodation(get_i64(row, "a121"));

        // Energy derivation
        let b226 = get_f64(row, "b226");
        let b489 = get_f64(row, "b489");
        let b490 = get_f64(row, "b490");
        let p537 = get_f64(row, "p537");
        let (elec_weekly, gas_weekly) = derive_energy(b226, b489, b490, p537, mean_elec_share);

        households.push(LcfsHousehold {
            num_adults: adults as f64,
            num_children: children as f64,
            region,
            employment_income: emp * 52.0,
            self_employment_income: se * 52.0,
            private_pension_income: pp * 52.0,
            hbai_net_income: hbai_income,
            tenure_type: tenure,
            accommodation_type: accomm,
            has_fuel_consumption: 0.0, // set later from WAS vehicle model
            // Consumption targets (weekly → annual)
            food: get_f64(row, "p601").max(0.0) * 52.0,
            alcohol: {
                let c021 = get_f64(row, "c021");
                let p602 = get_f64(row, "p602").max(0.0);
                if c021 > 0.0 { c021.max(0.0) * 52.0 } else { p602 * 0.70 * 52.0 }
            },
            tobacco: {
                let c022 = get_f64(row, "c022");
                let p602 = get_f64(row, "p602").max(0.0);
                if c022 > 0.0 { c022.max(0.0) * 52.0 } else { p602 * 0.30 * 52.0 }
            },
            clothing: get_f64(row, "p603").max(0.0) * 52.0,
            housing_water_electricity: get_f64(row, "p604").max(0.0) * 52.0,
            furnishings: get_f64(row, "p605").max(0.0) * 52.0,
            health: get_f64(row, "p606").max(0.0) * 52.0,
            transport: get_f64(row, "p607").max(0.0) * 52.0,
            communication: get_f64(row, "p608").max(0.0) * 52.0,
            recreation: get_f64(row, "p609").max(0.0) * 52.0,
            education: get_f64(row, "p610").max(0.0) * 52.0,
            restaurants: get_f64(row, "p611").max(0.0) * 52.0,
            miscellaneous: get_f64(row, "p612").max(0.0) * 52.0,
            petrol: get_f64(row, "c72211").max(0.0) * 52.0,
            diesel: get_f64(row, "c72212").max(0.0) * 52.0,
            domestic_energy: p537.max(0.0) * 52.0,
            electricity: elec_weekly.max(0.0) * 52.0,
            gas: gas_weekly.max(0.0) * 52.0,
        });
    }

    Ok(households)
}

/// Build feature vector from an LCFS household.
fn lcfs_features(hh: &LcfsHousehold) -> Vec<f64> {
    vec![
        hh.num_adults,
        hh.num_children,
        hh.region.to_rf_code(),
        hh.employment_income,
        hh.self_employment_income,
        hh.private_pension_income,
        hh.hbai_net_income,
        hh.tenure_type.to_rf_code(),
        hh.accommodation_type.to_rf_code(),
        hh.has_fuel_consumption,
    ]
}

/// Build feature vector from an FRS household.
fn frs_features(hh: &Household, people: &[Person]) -> Vec<f64> {
    let members: Vec<&Person> = hh.person_ids.iter().map(|&pid| &people[pid]).collect();
    let num_adults = members.iter().filter(|p| p.is_adult()).count() as f64;
    let num_children = members.iter().filter(|p| p.is_child()).count() as f64;
    let emp: f64 = members.iter().map(|p| p.employment_income).sum();
    let se: f64 = members.iter().map(|p| p.self_employment_income).sum();
    let pp: f64 = members.iter().map(|p| p.pension_income).sum();
    let hbai: f64 = members.iter().map(|p| p.total_income()).sum();

    vec![
        num_adults,
        num_children,
        hh.region.to_rf_code(),
        emp, se, pp, hbai,
        hh.tenure_type.to_rf_code(),
        hh.accommodation_type.to_rf_code(),
        0.0, // has_fuel_consumption — overwritten later
    ]
}

/// Extract target values from an LCFS household in model order.
fn lcfs_target_values(hh: &LcfsHousehold) -> [f64; 18] {
    [
        hh.food, hh.alcohol, hh.tobacco, hh.clothing,
        hh.housing_water_electricity, hh.furnishings, hh.health,
        hh.transport, hh.communication, hh.recreation,
        hh.education, hh.restaurants, hh.miscellaneous,
        hh.petrol, hh.diesel, hh.domestic_energy,
        hh.electricity, hh.gas,
    ]
}

/// Train a has_fuel_consumption model from WAS vehicle data and predict on LCFS + FRS.
fn impute_has_fuel(
    dataset: &Dataset,
    lcfs_data: &mut [LcfsHousehold],
    frs_features: &mut [Vec<f64>],
) -> anyhow::Result<()> {
    // Use FRS num_vehicles (already imputed from WAS) as training signal.
    // ICE share: 90% of vehicle owners have fuel consumption (NTS 2024).
    let mut rng = rand::thread_rng();

    // For FRS: derive has_fuel directly from imputed num_vehicles
    for (i, hh) in dataset.households.iter().enumerate() {
        let has_vehicle = hh.num_vehicles >= 0.5;
        let is_ice = has_vehicle && rng.gen::<f64>() < 0.90;
        frs_features[i][9] = if is_ice { 1.0 } else { 0.0 };
    }

    // For LCFS: use a simple vehicle ownership proxy.
    // LCFS doesn't directly have vehicle counts, so we use transport
    // spending as a proxy: if transport > 0 and random < 0.78 (vehicle ownership rate)
    for hh in lcfs_data.iter_mut() {
        let has_vehicle = hh.transport > 500.0 && rng.gen::<f64>() < 0.78;
        let is_ice = has_vehicle && rng.gen::<f64>() < 0.90;
        hh.has_fuel_consumption = if is_ice { 1.0 } else { 0.0 };
    }

    Ok(())
}

/// Run the full LCFS consumption imputation pipeline.
pub fn impute_consumption(
    dataset: &mut Dataset,
    lcfs_dir: &Path,
) -> anyhow::Result<()> {
    eprintln!("  Loading LCFS data...");
    let mut lcfs_data = build_lcfs_training_data(lcfs_dir)?;

    // Build FRS feature matrix
    let mut frs_feat: Vec<Vec<f64>> = dataset.households.iter()
        .map(|hh| frs_features(hh, &dataset.people))
        .collect();

    // Impute has_fuel_consumption for both LCFS and FRS
    impute_has_fuel(dataset, &mut lcfs_data, &mut frs_feat)?;

    // Build LCFS training features and targets
    let train_features: Vec<Vec<f64>> = lcfs_data.iter().map(|hh| lcfs_features(hh)).collect();
    let n = lcfs_data.len();
    let mut target_cols: Vec<Vec<f64>> = vec![Vec::with_capacity(n); CONSUMPTION_TARGETS.len()];
    for hh in &lcfs_data {
        let vals = lcfs_target_values(hh);
        for (j, &v) in vals.iter().enumerate() {
            target_cols[j].push(v);
        }
    }

    let named_targets: Vec<(&str, Vec<f64>)> = CONSUMPTION_TARGETS
        .iter()
        .zip(target_cols)
        .map(|(name, vals)| (*name, vals))
        .collect();

    eprintln!("  Training {} consumption models...", CONSUMPTION_TARGETS.len());
    let models = rf::train_multi_target(&train_features, &named_targets, 50, 42)?;

    eprintln!("  Predicting consumption on {} FRS households...", frs_feat.len());
    let predictions = rf::predict_multi_target(&models, &frs_feat)?;

    for (name, preds) in &predictions {
        for (i, &val) in preds.iter().enumerate() {
            let hh = &mut dataset.households[i];
            let v = val.max(0.0); // consumption can't be negative
            match name.as_str() {
                "food_consumption" => hh.food_consumption = v,
                "alcohol_consumption" => hh.alcohol_consumption = v,
                "tobacco_consumption" => hh.tobacco_consumption = v,
                "clothing_consumption" => hh.clothing_consumption = v,
                "housing_water_electricity_consumption" => hh.housing_water_electricity_consumption = v,
                "furnishings_consumption" => hh.furnishings_consumption = v,
                "health_consumption" => hh.health_consumption = v,
                "transport_consumption" => hh.transport_consumption = v,
                "communication_consumption" => hh.communication_consumption = v,
                "recreation_consumption" => hh.recreation_consumption = v,
                "education_consumption" => hh.education_consumption = v,
                "restaurants_consumption" => hh.restaurants_consumption = v,
                "miscellaneous_consumption" => hh.miscellaneous_consumption = v,
                "petrol_spending" => hh.petrol_spending = v,
                "diesel_spending" => hh.diesel_spending = v,
                "domestic_energy_consumption" => hh.domestic_energy_consumption = v,
                "electricity_consumption" => hh.electricity_consumption = v,
                "gas_consumption" => hh.gas_consumption = v,
                _ => {}
            }
        }
    }

    // Zero out fuel spending for non-fuel households
    for (i, hh) in dataset.households.iter_mut().enumerate() {
        if frs_feat[i][9] < 0.5 {
            hh.petrol_spending = 0.0;
            hh.diesel_spending = 0.0;
        }
    }

    // Run NEED energy calibration
    calibrate::calibrate_energy_to_need(dataset);

    let mean_food: f64 = dataset.households.iter()
        .map(|h| h.weight * h.food_consumption)
        .sum::<f64>()
        / dataset.households.iter().map(|h| h.weight).sum::<f64>();
    eprintln!("  Consumption imputation complete. Mean food spending: £{:.0}/yr", mean_food);

    Ok(())
}
