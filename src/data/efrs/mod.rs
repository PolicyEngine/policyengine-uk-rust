pub mod rf;
pub mod was;
pub mod lcfs;
pub mod calibrate;

use std::path::Path;
use crate::data::Dataset;

/// Run the full Enhanced FRS imputation pipeline:
/// 1. WAS wealth imputation (trains RF on WAS, predicts on FRS)
/// 2. LCFS consumption imputation (trains RF on LCFS, predicts on FRS)
///    - Depends on WAS for num_vehicles → has_fuel_consumption
/// 3. NEED energy calibration (rakes electricity/gas to NEED 2023 targets)
pub fn enhance_dataset(
    dataset: &mut Dataset,
    was_dir: &Path,
    lcfs_dir: &Path,
) -> anyhow::Result<()> {
    eprintln!("Enhancing FRS with wealth and consumption imputations...");
    eprintln!("  {} households, {} people", dataset.households.len(), dataset.people.len());

    // Step 1: Wealth (must run first — provides num_vehicles)
    was::impute_wealth(dataset, was_dir)?;

    // Step 2: Consumption (uses num_vehicles for fuel indicator)
    lcfs::impute_consumption(dataset, lcfs_dir)?;

    eprintln!("Enhanced FRS complete.");
    Ok(())
}
