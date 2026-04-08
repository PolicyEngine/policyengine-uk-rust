pub mod rf;
pub mod was;
pub mod lcfs;
pub mod calibrate;

use std::path::Path;
use std::time::Instant;
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
    let t0 = Instant::now();
    eprintln!("Enhancing FRS with wealth and consumption imputations...");
    eprintln!("  {} households, {} people", dataset.households.len(), dataset.people.len());

    // Step 1: Wealth (must run first — provides num_vehicles)
    let t1 = Instant::now();
    was::impute_wealth(dataset, was_dir)?;
    eprintln!("  Wealth imputation took {:.1}s", t1.elapsed().as_secs_f64());

    // Step 2: Consumption (uses num_vehicles for fuel indicator)
    let t2 = Instant::now();
    lcfs::impute_consumption(dataset, lcfs_dir)?;
    eprintln!("  Consumption imputation took {:.1}s", t2.elapsed().as_secs_f64());

    eprintln!("Enhanced FRS complete in {:.1}s total.", t0.elapsed().as_secs_f64());
    Ok(())
}
