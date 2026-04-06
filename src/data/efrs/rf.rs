use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::ensemble::random_forest_regressor::{
    RandomForestRegressor, RandomForestRegressorParameters,
};
use rayon::prelude::*;

/// A trained single-target random forest model.
pub struct TrainedRF {
    model: RandomForestRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
    pub target_name: String,
}

/// Train multiple RF models (one per target column) on the same feature matrix.
/// Models are trained in parallel using Rayon since they are independent.
pub fn train_multi_target(
    features: &[Vec<f64>],
    targets: &[(&str, Vec<f64>)],
    n_trees: u16,
    seed: u64,
) -> anyhow::Result<Vec<TrainedRF>> {
    // Build the DenseMatrix once — shared across all parallel model fits
    let feat_vec: Vec<Vec<f64>> = features.to_vec();
    let x = DenseMatrix::from_2d_vec(&feat_vec)
        .map_err(|e| anyhow::anyhow!("Matrix construction failed: {:?}", e))?;

    targets
        .par_iter()
        .enumerate()
        .map(|(i, (name, values))| {
            let y = values.clone();
            // Vary seed per target so trees aren't identical across models
            let params = RandomForestRegressorParameters::default()
                .with_n_trees(n_trees as usize)
                .with_seed(seed + i as u64);
            let model = RandomForestRegressor::fit(&x, &y, params)
                .map_err(|e| anyhow::anyhow!("RF training failed for {}: {:?}", name, e))?;
            Ok(TrainedRF {
                model,
                target_name: name.to_string(),
            })
        })
        .collect()
}

/// Predict all models and return a vec of (target_name, predictions).
/// Predictions run in parallel since models are independent.
pub fn predict_multi_target(
    models: &[TrainedRF],
    features: &[Vec<f64>],
) -> anyhow::Result<Vec<(String, Vec<f64>)>> {
    // Build the DenseMatrix once for all predictions
    let feat_vec: Vec<Vec<f64>> = features.to_vec();
    let x = DenseMatrix::from_2d_vec(&feat_vec)
        .map_err(|e| anyhow::anyhow!("Matrix construction failed: {:?}", e))?;

    models
        .par_iter()
        .map(|m| {
            let preds = m.model.predict(&x)
                .map_err(|e| anyhow::anyhow!("RF prediction failed for {}: {:?}", m.target_name, e))?;
            Ok((m.target_name.clone(), preds))
        })
        .collect()
}
