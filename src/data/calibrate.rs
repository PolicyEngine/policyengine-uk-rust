//! Calibration / reweighting of household survey data to match administrative targets.
//!
//! Loads calibration targets from a JSON file, builds a matrix of household-level
//! contributions to each target, and optimises household weights using Adam in
//! log-space to minimise mean squared relative error.

use std::path::Path;

use colored::Colorize;
use comfy_table::{Table, ContentArrangement, presets, Cell, Color};
use rand::Rng;
use rayon::prelude::*;
use serde::Deserialize;

use crate::data::Dataset;

// ── Target schema ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct CalibrationTargetFile {
    pub targets: Vec<CalibrationTarget>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct CalibrationTarget {
    pub name: String,
    pub variable: String,
    pub entity: String,
    pub aggregation: String,
    #[serde(default)]
    pub filter: Option<TargetFilter>,
    pub value: f64,
    pub source: String,
    pub year: u32,
    #[serde(default)]
    pub holdout: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TargetFilter {
    pub variable: String,
    pub min: f64,
    pub max: f64,
}

// ── Load targets ───────────────────────────────────────────────────────────

pub fn load_targets(path: &Path) -> anyhow::Result<Vec<CalibrationTarget>> {
    let text = std::fs::read_to_string(path)?;
    let file: CalibrationTargetFile = serde_json::from_str(&text)?;
    Ok(file.targets)
}

// ── Variable resolution ────────────────────────────────────────────────────

/// Get a person-level variable value by name.
fn person_variable(p: &crate::engine::entities::Person, name: &str) -> f64 {
    match name {
        "age" => p.age,
        "employment_income" => p.employment_income,
        "self_employment_income" => p.self_employment_income,
        "pension_income" | "private_pension_income" => p.pension_income,
        "state_pension" => p.state_pension,
        "savings_interest_income" | "savings_interest" => p.savings_interest_income,
        "dividend_income" => p.dividend_income,
        "capital_gains" => p.capital_gains,
        "property_income" => p.property_income,
        "maintenance_income" => p.maintenance_income,
        "miscellaneous_income" => p.miscellaneous_income,
        "other_income" => p.other_income,
        "child_benefit" => p.child_benefit,
        "housing_benefit" => p.housing_benefit,
        "income_support" => p.income_support,
        "pension_credit" => p.pension_credit,
        "child_tax_credit" => p.child_tax_credit,
        "working_tax_credit" => p.working_tax_credit,
        "universal_credit" => p.universal_credit,
        "dla_care" => p.dla_care,
        "dla_mobility" => p.dla_mobility,
        "pip_daily_living" => p.pip_daily_living,
        "pip_mobility" => p.pip_mobility,
        "carers_allowance" => p.carers_allowance,
        "attendance_allowance" => p.attendance_allowance,
        "esa_income" => p.esa_income,
        "esa_contributory" => p.esa_contributory,
        "jsa_income" => p.jsa_income,
        "jsa_contributory" => p.jsa_contributory,
        "other_benefits" => p.other_benefits,
        "total_income" => p.total_income(),
        "hours_worked" => p.hours_worked,
        _ => 0.0,
    }
}

/// Get a household-level variable value by name.
fn household_variable(h: &crate::engine::entities::Household, name: &str) -> f64 {
    match name {
        "council_tax_annual" | "council_tax" => h.council_tax,
        "rent_annual" | "rent" => h.rent,
        "weight" => h.weight,
        "household_id" => 1.0, // For counting households
        "property_wealth" => h.property_wealth,
        "net_financial_wealth" => h.net_financial_wealth,
        "gross_financial_wealth" => h.gross_financial_wealth,
        "savings" => h.savings,
        _ => 0.0,
    }
}

// ── Matrix building ────────────────────────────────────────────────────────

/// Build the calibration matrix M[i][j] and target vector y[j].
///
/// M[i][j] = household i's contribution to target j (before weighting).
/// y[j] = the target value.
///
/// Returns (matrix, target_values, training_mask) where training_mask[j]
/// is true if target j should be included in the loss.
pub fn build_matrix(
    dataset: &Dataset,
    targets: &[CalibrationTarget],
) -> (Vec<Vec<f64>>, Vec<f64>, Vec<bool>) {
    let n_hh = dataset.households.len();
    let n_targets = targets.len();
    let mut matrix = vec![vec![0.0f64; n_targets]; n_hh];
    let mut target_values = vec![0.0f64; n_targets];
    let mut training_mask = vec![true; n_targets];

    for (j, target) in targets.iter().enumerate() {
        target_values[j] = target.value;
        // Will be refined after matrix is built (skip unfittable targets)
        training_mask[j] = !target.holdout;

        match target.entity.as_str() {
            "person" => {
                for (i, hh) in dataset.households.iter().enumerate() {
                    let mut contribution = 0.0f64;
                    for &pid in &hh.person_ids {
                        let person = &dataset.people[pid];

                        // Apply filter if present
                        if let Some(ref filter) = target.filter {
                            let filter_val = person_variable(person, &filter.variable);
                            if filter_val < filter.min || filter_val >= filter.max {
                                continue;
                            }
                        }

                        match target.aggregation.as_str() {
                            "sum" => {
                                contribution += person_variable(person, &target.variable);
                            }
                            "count_nonzero" => {
                                if person_variable(person, &target.variable) > 0.0 {
                                    contribution += 1.0;
                                }
                            }
                            "count" => {
                                contribution += 1.0;
                            }
                            _ => {}
                        }
                    }
                    matrix[i][j] = contribution;
                }
            }
            "household" => {
                for (i, hh) in dataset.households.iter().enumerate() {
                    match target.aggregation.as_str() {
                        "sum" => {
                            matrix[i][j] = household_variable(hh, &target.variable);
                        }
                        "count" | "count_nonzero" => {
                            let val = household_variable(hh, &target.variable);
                            matrix[i][j] = if val > 0.0 { 1.0 } else { 0.0 };
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // Skip targets where no household contributes (matrix column all zero).
    // These are unfittable (e.g. top income bands not represented in FRS).
    let mut n_skipped = 0;
    for j in 0..n_targets {
        let col_sum: f64 = (0..n_hh).map(|i| matrix[i][j].abs()).sum();
        if col_sum < 1e-10 {
            training_mask[j] = false;
            n_skipped += 1;
        }
    }
    if n_skipped > 0 {
        eprintln!("  Skipped {} targets with no survey representation", n_skipped);
    }

    (matrix, target_values, training_mask)
}

// ── Adam optimiser ─────────────────────────────────────────────────────────

/// Calibration configuration.
pub struct CalibrateConfig {
    pub epochs: usize,
    pub lr: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub eps: f64,
    pub dropout: f64,
    pub log_interval: usize,
}

impl Default for CalibrateConfig {
    fn default() -> Self {
        CalibrateConfig {
            epochs: 512,
            lr: 0.1,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            dropout: 0.05,
            log_interval: 50,
        }
    }
}

/// Result of calibration.
pub struct CalibrateResult {
    pub weights: Vec<f64>,
    pub final_training_loss: f64,
    pub final_holdout_loss: f64,
    pub per_target_error: Vec<(String, f64, f64, f64, bool)>, // (name, predicted, target, rel_error, holdout)
}

/// Run Adam optimisation to find weights minimising MSRE against targets.
///
/// Loss = mean_j((pred_j / target_j - 1)^2) for training targets.
/// Weights are parameterised as w_i = exp(u_i) for positivity.
pub fn calibrate(
    matrix: &[Vec<f64>],
    target_values: &[f64],
    training_mask: &[bool],
    initial_weights: &[f64],
    config: &CalibrateConfig,
) -> CalibrateResult {
    let n_hh = matrix.len();
    let n_targets = target_values.len();
    let n_training = training_mask.iter().filter(|&&m| m).count();

    if n_hh == 0 || n_targets == 0 || n_training == 0 {
        return CalibrateResult {
            weights: initial_weights.to_vec(),
            final_training_loss: 0.0,
            final_holdout_loss: 0.0,
            per_target_error: Vec::new(),
        };
    }

    // Initialise log-weights
    let mut u: Vec<f64> = initial_weights.iter()
        .map(|&w| if w > 0.0 { w.ln() } else { 0.0 })
        .collect();

    // Adam state
    let mut m = vec![0.0f64; n_hh];
    let mut v = vec![0.0f64; n_hh];

    let mut rng = rand::thread_rng();

    for epoch in 0..config.epochs {
        // Compute weights with optional dropout
        let weights: Vec<f64> = u.iter().enumerate().map(|(_i, &ui)| {
            let w = ui.exp();
            if config.dropout > 0.0 && rng.gen::<f64>() < config.dropout {
                0.0 // Drop this household
            } else {
                w / (1.0 - config.dropout) // Scale up to compensate
            }
        }).collect();

        // Forward pass: pred_j = sum_i w_i * M_ij
        let predictions: Vec<f64> = (0..n_targets).into_par_iter().map(|j| {
            let mut pred = 0.0f64;
            for i in 0..n_hh {
                pred += weights[i] * matrix[i][j];
            }
            pred
        }).collect();

        // Compute residuals: r_j = pred_j / target_j - 1
        let residuals: Vec<f64> = (0..n_targets).map(|j| {
            if target_values[j].abs() > 1.0 {
                predictions[j] / target_values[j] - 1.0
            } else {
                0.0
            }
        }).collect();

        // Training loss
        let training_loss: f64 = residuals.iter().enumerate()
            .filter(|(j, _)| training_mask[*j])
            .map(|(_, r)| r * r)
            .sum::<f64>() / n_training as f64;

        // Holdout loss
        let n_holdout = training_mask.iter().filter(|&&m| !m).count();
        let holdout_loss = if n_holdout > 0 {
            residuals.iter().enumerate()
                .filter(|(j, _)| !training_mask[*j])
                .map(|(_, r)| r * r)
                .sum::<f64>() / n_holdout as f64
        } else {
            0.0
        };

        if epoch % config.log_interval == 0 || epoch == config.epochs - 1 {
            let rmse_train = training_loss.sqrt() * 100.0;
            let rmse_holdout = holdout_loss.sqrt() * 100.0;
            eprintln!(
                "  Epoch {:>4}/{}: training RMSRE {:.2}%, holdout RMSRE {:.2}%",
                epoch, config.epochs, rmse_train, rmse_holdout
            );
        }

        if epoch == config.epochs - 1 {
            // Build final result with actual weights (no dropout)
            let final_weights: Vec<f64> = u.iter().map(|&ui| ui.exp()).collect();
            let final_preds: Vec<f64> = (0..n_targets).map(|j| {
                let mut pred = 0.0f64;
                for i in 0..n_hh {
                    pred += final_weights[i] * matrix[i][j];
                }
                pred
            }).collect();

            let per_target_error: Vec<(String, f64, f64, f64, bool)> = Vec::new();

            let final_training_loss: f64 = (0..n_targets)
                .filter(|&j| training_mask[j])
                .map(|j| {
                    let r = if target_values[j].abs() > 1.0 {
                        final_preds[j] / target_values[j] - 1.0
                    } else { 0.0 };
                    r * r
                }).sum::<f64>() / n_training as f64;

            let final_holdout_loss = if n_holdout > 0 {
                (0..n_targets)
                    .filter(|&j| !training_mask[j])
                    .map(|j| {
                        let r = if target_values[j].abs() > 1.0 {
                            final_preds[j] / target_values[j] - 1.0
                        } else { 0.0 };
                        r * r
                    }).sum::<f64>() / n_holdout as f64
            } else { 0.0 };

            // Compute per-target errors for reporting
            // (done outside the return to avoid borrow issues)
            let per_target: Vec<(String, f64, f64, f64, bool)> = (0..n_targets).map(|j| {
                let rel_err = if target_values[j].abs() > 1.0 {
                    final_preds[j] / target_values[j] - 1.0
                } else { 0.0 };
                (String::new(), final_preds[j], target_values[j], rel_err, !training_mask[j])
            }).collect();

            // We'll fill names outside this block
            let _ = per_target;
            let _ = per_target_error;

            return CalibrateResult {
                weights: final_weights,
                final_training_loss,
                final_holdout_loss,
                per_target_error: (0..n_targets).map(|j| {
                    let rel_err = if target_values[j].abs() > 1.0 {
                        final_preds[j] / target_values[j] - 1.0
                    } else { 0.0 };
                    (String::new(), final_preds[j], target_values[j], rel_err, !training_mask[j])
                }).collect(),
            };
        }

        // Backward pass: compute gradient dL/du_i
        // dL/du_i = (2/n_training) * sum_j [training_mask_j * r_j * M_ij * w_i / y_j]
        let grad: Vec<f64> = (0..n_hh).into_par_iter().map(|i| {
            let w_i = weights[i];
            let mut g = 0.0f64;
            for j in 0..n_targets {
                if training_mask[j] && target_values[j].abs() > 1.0 {
                    g += residuals[j] * matrix[i][j] * w_i / target_values[j];
                }
            }
            2.0 * g / n_training as f64
        }).collect();

        // Adam update
        let t = (epoch + 1) as f64;
        let bc1 = 1.0 - config.beta1.powf(t);
        let bc2 = 1.0 - config.beta2.powf(t);

        for i in 0..n_hh {
            m[i] = config.beta1 * m[i] + (1.0 - config.beta1) * grad[i];
            v[i] = config.beta2 * v[i] + (1.0 - config.beta2) * grad[i] * grad[i];
            let m_hat = m[i] / bc1;
            let v_hat = v[i] / bc2;
            u[i] -= config.lr * m_hat / (v_hat.sqrt() + config.eps);
        }
    }

    // Should not reach here, but just in case
    let final_weights: Vec<f64> = u.iter().map(|&ui| ui.exp()).collect();
    CalibrateResult {
        weights: final_weights,
        final_training_loss: 0.0,
        final_holdout_loss: 0.0,
        per_target_error: Vec::new(),
    }
}

// ── Reporting ──────────────────────────────────────────────────────────────

/// Print a summary table of calibration results.
pub fn print_report(
    targets: &[CalibrationTarget],
    result: &CalibrateResult,
    dataset: &Dataset,
) {
    let total_weight: f64 = result.weights.iter().sum();
    let original_weight: f64 = dataset.households.iter().map(|h| h.weight).sum();

    eprintln!("\n{}", "Calibration complete".bright_green().bold());
    eprintln!(
        "  Households: {}  Original weight sum: {:.0}  Calibrated weight sum: {:.0}",
        dataset.households.len(), original_weight, total_weight
    );
    eprintln!(
        "  Training RMSRE: {:.2}%  Holdout RMSRE: {:.2}%",
        result.final_training_loss.sqrt() * 100.0,
        result.final_holdout_loss.sqrt() * 100.0,
    );

    // Per-target table (show worst 20 + all holdout)
    let mut rows: Vec<(usize, &str, f64, f64, f64, bool)> = result.per_target_error.iter().enumerate()
        .map(|(j, (_, pred, target, rel_err, holdout))| {
            (j, targets[j].name.as_str(), *pred, *target, *rel_err, *holdout)
        })
        .collect();

    // Sort by absolute relative error, descending
    rows.sort_by(|a, b| b.4.abs().partial_cmp(&a.4.abs()).unwrap_or(std::cmp::Ordering::Equal));

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Target", "Predicted", "Actual", "Rel error", "Type"]);

    let show_count = 30;
    for (idx, (_, name, pred, target, rel_err, holdout)) in rows.iter().enumerate() {
        if idx >= show_count && !holdout {
            continue;
        }
        let err_pct = rel_err * 100.0;
        let err_color = if err_pct.abs() < 5.0 {
            Color::Green
        } else if err_pct.abs() < 15.0 {
            Color::Yellow
        } else {
            Color::Red
        };
        let type_str = if *holdout { "holdout" } else { "training" };

        table.add_row(vec![
            Cell::new(name),
            Cell::new(format_value(*pred)),
            Cell::new(format_value(*target)),
            Cell::new(format!("{:+.1}%", err_pct)).fg(err_color),
            Cell::new(type_str),
        ]);
    }

    eprintln!("\n{table}");
}

fn format_value(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1e9 {
        format!("£{:.1}bn", v / 1e9)
    } else if abs >= 1e6 {
        format!("£{:.1}m", v / 1e6)
    } else if abs >= 1e3 {
        format!("{:.0}k", v / 1e3)
    } else {
        format!("{:.0}", v)
    }
}

// ── Apply weights ──────────────────────────────────────────────────────────

/// Apply calibrated weights to the dataset.
pub fn apply_weights(dataset: &mut Dataset, weights: &[f64]) {
    for (i, hh) in dataset.households.iter_mut().enumerate() {
        if i < weights.len() {
            hh.weight = weights[i];
        }
    }
}
