use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::ensemble::random_forest_regressor::{
    RandomForestRegressor, RandomForestRegressorParameters,
};

/// A trained single-target random forest model.
pub struct TrainedRF {
    model: RandomForestRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
    pub target_name: String,
}

/// Train a random forest regressor for a single target variable.
///
/// `features` is n_samples x n_features (outer vec = rows).
/// `target` is n_samples.
pub fn train_rf(
    features: &[Vec<f64>],
    target: &[f64],
    target_name: &str,
    n_trees: u16,
    seed: u64,
) -> anyhow::Result<TrainedRF> {
    let feat_vec: Vec<Vec<f64>> = features.to_vec();
    let x = DenseMatrix::from_2d_vec(&feat_vec)
        .map_err(|e| anyhow::anyhow!("Matrix construction failed for {}: {:?}", target_name, e))?;
    let y = target.to_vec();
    let params = RandomForestRegressorParameters::default()
        .with_n_trees(n_trees as usize)
        .with_seed(seed);
    let model = RandomForestRegressor::fit(&x, &y, params)
        .map_err(|e| anyhow::anyhow!("RF training failed for {}: {:?}", target_name, e))?;
    Ok(TrainedRF {
        model,
        target_name: target_name.to_string(),
    })
}

/// Predict target values for new feature rows.
pub fn predict_rf(model: &TrainedRF, features: &[Vec<f64>]) -> anyhow::Result<Vec<f64>> {
    let feat_vec: Vec<Vec<f64>> = features.to_vec();
    let x = DenseMatrix::from_2d_vec(&feat_vec)
        .map_err(|e| anyhow::anyhow!("Matrix construction failed for {}: {:?}", model.target_name, e))?;
    model
        .model
        .predict(&x)
        .map_err(|e| anyhow::anyhow!("RF prediction failed for {}: {:?}", model.target_name, e))
}

/// Train multiple RF models (one per target column) on the same feature matrix.
pub fn train_multi_target(
    features: &[Vec<f64>],
    targets: &[(&str, Vec<f64>)],
    n_trees: u16,
    seed: u64,
) -> anyhow::Result<Vec<TrainedRF>> {
    targets
        .iter()
        .map(|(name, values)| train_rf(features, values, name, n_trees, seed))
        .collect()
}

/// Predict all models and return a vec of (target_name, predictions).
pub fn predict_multi_target(
    models: &[TrainedRF],
    features: &[Vec<f64>],
) -> anyhow::Result<Vec<(String, Vec<f64>)>> {
    models
        .iter()
        .map(|m| {
            let preds = predict_rf(m, features)?;
            Ok((m.target_name.clone(), preds))
        })
        .collect()
}
