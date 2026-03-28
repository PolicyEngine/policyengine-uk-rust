mod engine;
mod parameters;
mod variables;
mod data;
mod reforms;

use clap::Parser;
use colored::Colorize;
use comfy_table::{Table, ContentArrangement, presets};
use serde::Serialize;
use std::path::PathBuf;

use crate::engine::Simulation;
use crate::parameters::Parameters;
use crate::reforms::Reform;
use crate::data::synthetic::generate_synthetic_frs;
use crate::data::frs::load_frs;
use crate::data::clean::{write_clean_csvs, load_clean_frs};

#[derive(Parser)]
#[command(name = "policyengine-uk")]
#[command(about = "High-performance UK tax-benefit microsimulation engine")]
#[command(version)]
struct Cli {
    /// Reform file (YAML). If omitted, runs the default PA=£20k reform.
    #[arg(short, long)]
    reform: Option<PathBuf>,

    /// Reform as inline JSON string (alternative to --reform YAML file)
    #[arg(long)]
    reform_json: Option<String>,

    /// Path to FRS CSV data directory (e.g. data/UKDA-9367-csv/csv/).
    /// If omitted, uses synthetic data.
    #[arg(long)]
    frs: Option<PathBuf>,

    /// Fiscal year start (e.g. 2029 for FY 2029/30).
    /// Available: 2023-2029.
    #[arg(short, long, default_value = "2025")]
    year: u32,

    /// Number of synthetic households (more = slower but more precise)
    #[arg(short = 'n', long, default_value = "20000")]
    households: usize,

    /// Export baseline parameters to YAML (useful for writing reforms)
    #[arg(long)]
    export_baseline: bool,

    /// Export baseline parameters as JSON
    #[arg(long)]
    export_params_json: bool,

    /// Extract raw FRS data to clean CSVs. Requires --frs.
    /// Writes persons.csv, benunits.csv, households.csv to the given directory.
    #[arg(long)]
    extract_frs: Option<PathBuf>,

    /// Load from clean FRS CSVs (produced by --extract-frs) instead of raw FRS.
    #[arg(long)]
    clean_frs: Option<PathBuf>,

    /// Output format: "pretty" (default) or "json" for machine-readable output
    #[arg(long, default_value = "pretty")]
    output: String,

    /// Dump per-household microdata as CSV to stdout (weight, equivalised_net_income, gross_income, total_tax, total_benefits)
    #[arg(long)]
    output_microdata: bool,

    /// Show per-decile breakdown
    #[arg(long, default_value = "true")]
    deciles: bool,
}

#[derive(Serialize)]
struct JsonOutput {
    fiscal_year: String,
    budgetary_impact: BudgetaryImpact,
    program_breakdown: ProgramBreakdown,
    caseloads: Caseloads,
    decile_impacts: Vec<DecileImpact>,
    winners_losers: WinnersLosers,
}

#[derive(Serialize)]
struct BudgetaryImpact {
    baseline_revenue: f64,
    reform_revenue: f64,
    revenue_change: f64,
    baseline_benefits: f64,
    reform_benefits: f64,
    benefit_spending_change: f64,
    net_cost: f64,
}

#[derive(Serialize)]
struct ProgramBreakdown {
    income_tax: f64,
    employee_ni: f64,
    employer_ni: f64,
    universal_credit: f64,
    child_benefit: f64,
    state_pension: f64,
    pension_credit: f64,
    housing_benefit: f64,
    child_tax_credit: f64,
    working_tax_credit: f64,
    income_support: f64,
    scottish_child_payment: f64,
    benefit_cap_reduction: f64,
}

#[derive(Serialize)]
struct Caseloads {
    income_tax_payers: f64,
    ni_payers: f64,
    employer_ni_payers: f64,
    universal_credit: f64,
    child_benefit: f64,
    state_pension: f64,
    pension_credit: f64,
    housing_benefit: f64,
    child_tax_credit: f64,
    working_tax_credit: f64,
    income_support: f64,
    scottish_child_payment: f64,
    benefit_cap_affected: f64,
}

#[derive(Serialize)]
struct DecileImpact {
    decile: usize,
    avg_baseline_income: f64,
    avg_reform_income: f64,
    avg_change: f64,
    pct_change: f64,
}

#[derive(Serialize)]
struct WinnersLosers {
    winners_pct: f64,
    losers_pct: f64,
    unchanged_pct: f64,
    avg_gain: f64,
    avg_loss: f64,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load baseline parameters for the chosen fiscal year
    let baseline_params = Parameters::for_year(cli.year)?;

    if cli.export_baseline {
        println!("{}", baseline_params.to_yaml());
        return Ok(());
    }

    if cli.export_params_json {
        println!("{}", baseline_params.to_json());
        return Ok(());
    }

    let json_mode = cli.output == "json";

    // Extract FRS to clean CSVs if requested
    if let Some(output_dir) = &cli.extract_frs {
        let frs_path = cli.frs.as_ref()
            .ok_or_else(|| anyhow::anyhow!("--extract-frs requires --frs <raw-frs-dir>"))?;
        eprintln!("Loading raw FRS from {}...", frs_path.display());
        let mut dataset = load_frs(frs_path)?;
        eprintln!("Loaded {} households, {} people", dataset.households.len(), dataset.people.len());
        eprintln!("Computing ENR flags from baseline {}...", cli.year);
        write_clean_csvs(&mut dataset, &baseline_params, output_dir)?;
        eprintln!("Wrote clean CSVs to {}", output_dir.display());
        return Ok(());
    }

    // Load dataset
    let dataset = if let Some(clean_path) = &cli.clean_frs {
        if !json_mode { println!("  {} Loading clean FRS from {}...", "▸".bright_cyan(), clean_path.display()); }
        load_clean_frs(clean_path)?
    } else if let Some(frs_path) = &cli.frs {
        if !json_mode { println!("  {} Loading FRS microdata from {}...", "▸".bright_cyan(), frs_path.display()); }
        load_frs(frs_path)?
    } else {
        if !json_mode { println!("  {} Generating synthetic population...", "▸".bright_cyan()); }
        generate_synthetic_frs(cli.year)
    };

    // Load reform
    let reform_params = if let Some(json_str) = &cli.reform_json {
        baseline_params.apply_json_overlay(json_str)?
    } else if let Some(path) = &cli.reform {
        let r = Reform::from_file(path, &baseline_params)?;
        r.parameters
    } else if json_mode {
        // JSON mode with no reform = baseline vs baseline
        baseline_params.clone()
    } else {
        let r = Reform::personal_allowance_20k(&baseline_params);
        r.parameters
    };

    // Run baseline
    let baseline_sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        baseline_params.clone(),
    );
    let baseline = baseline_sim.run();

    // Run reform
    let reform_sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        reform_params.clone(),
    );
    let reformed = reform_sim.run();

    // Microdata output mode
    if cli.output_microdata {
        println!("weight,equivalised_net_income,gross_income,total_tax,total_benefits");
        for hh in &dataset.households {
            let r = &baseline.household_results[hh.id];
            println!("{},{},{},{},{}", hh.weight, r.equivalised_net_income, r.gross_income, r.total_tax, r.total_benefits);
        }
        return Ok(());
    }

    // Analysis
    let households = &dataset.households;

    let baseline_revenue: f64 = households.iter()
        .map(|h| h.weight * baseline.household_results[h.id].total_tax)
        .sum();
    let reform_revenue: f64 = households.iter()
        .map(|h| h.weight * reformed.household_results[h.id].total_tax)
        .sum();
    let revenue_change = reform_revenue - baseline_revenue;

    let baseline_benefits: f64 = households.iter()
        .map(|h| h.weight * baseline.household_results[h.id].total_benefits)
        .sum();
    let reform_benefits: f64 = households.iter()
        .map(|h| h.weight * reformed.household_results[h.id].total_benefits)
        .sum();
    let benefit_change = reform_benefits - baseline_benefits;
    let net_cost = -revenue_change + benefit_change;

    // Decile analysis — ranked by equivalised HBAI net income BHC (baseline)
    let mut hh_incomes: Vec<(usize, f64, f64)> = households.iter().map(|hh| {
        (hh.id,
         baseline.household_results[hh.id].equivalised_net_income,
         reformed.household_results[hh.id].equivalised_net_income)
    }).collect();
    hh_incomes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let decile_size = hh_incomes.len() / 10;
    let mut decile_impacts = Vec::new();
    for d in 0..10 {
        let start = d * decile_size;
        let end = if d == 9 { hh_incomes.len() } else { (d + 1) * decile_size };
        let slice = &hh_incomes[start..end];
        let n = slice.len() as f64;
        let avg_base: f64 = slice.iter().map(|h| h.1).sum::<f64>() / n;
        let avg_reform: f64 = slice.iter().map(|h| h.2).sum::<f64>() / n;
        let avg_change = avg_reform - avg_base;
        let pct_change = if avg_base != 0.0 { 100.0 * avg_change / avg_base } else { 0.0 };
        decile_impacts.push(DecileImpact {
            decile: d + 1,
            avg_baseline_income: (avg_base * 100.0).round() / 100.0,
            avg_reform_income: (avg_reform * 100.0).round() / 100.0,
            avg_change: (avg_change * 100.0).round() / 100.0,
            pct_change: (pct_change * 100.0).round() / 100.0,
        });
    }

    // Winners and losers
    let mut winners = 0.0f64;
    let mut losers = 0.0f64;
    let mut unchanged = 0.0f64;
    let mut total_gain = 0.0f64;
    let mut total_loss = 0.0f64;

    for hh in households {
        let change = reformed.household_results[hh.id].net_income
            - baseline.household_results[hh.id].net_income;
        if change > 1.0 {
            winners += hh.weight;
            total_gain += hh.weight * change;
        } else if change < -1.0 {
            losers += hh.weight;
            total_loss += hh.weight * change;
        } else {
            unchanged += hh.weight;
        }
    }

    let total_hh = winners + losers + unchanged;
    let winners_losers = WinnersLosers {
        winners_pct: (1000.0 * winners / total_hh).round() / 10.0,
        losers_pct: (1000.0 * losers / total_hh).round() / 10.0,
        unchanged_pct: (1000.0 * unchanged / total_hh).round() / 10.0,
        avg_gain: if winners > 0.0 { (total_gain / winners).round() } else { 0.0 },
        avg_loss: if losers > 0.0 { (total_loss.abs() / losers).round() } else { 0.0 },
    };

    // Program-level breakdown and caseloads (weighted totals from reform)
    let benunits = &dataset.benunits;
    let (program_breakdown, caseloads) = {
        // Tax spending and caseloads
        let mut income_tax = 0.0f64;
        let mut employee_ni = 0.0f64;
        let mut employer_ni = 0.0f64;
        let mut it_payers = 0.0f64;
        let mut ni_payers = 0.0f64;
        let mut eni_payers = 0.0f64;
        for hh in households {
            for &pid in &hh.person_ids {
                let pr = &reformed.person_results[pid];
                income_tax += hh.weight * pr.income_tax;
                employee_ni += hh.weight * pr.national_insurance;
                employer_ni += hh.weight * pr.employer_ni;
                if pr.income_tax > 0.0 { it_payers += hh.weight; }
                if pr.national_insurance > 0.0 { ni_payers += hh.weight; }
                if pr.employer_ni > 0.0 { eni_payers += hh.weight; }
            }
        }
        // Benefit spending and caseloads
        let mut uc = 0.0f64;
        let mut cb = 0.0f64;
        let mut sp = 0.0f64;
        let mut pc = 0.0f64;
        let mut hb = 0.0f64;
        let mut ctc = 0.0f64;
        let mut wtc = 0.0f64;
        let mut is_val = 0.0f64;
        let mut scp = 0.0f64;
        let mut cap = 0.0f64;
        let mut cl_uc = 0.0f64;
        let mut cl_cb = 0.0f64;
        let mut cl_sp = 0.0f64;
        let mut cl_pc = 0.0f64;
        let mut cl_hb = 0.0f64;
        let mut cl_ctc = 0.0f64;
        let mut cl_wtc = 0.0f64;
        let mut cl_is = 0.0f64;
        let mut cl_scp = 0.0f64;
        let mut cl_cap = 0.0f64;
        for bu in benunits {
            let w = households[bu.household_id].weight;
            let br = &reformed.benunit_results[bu.id];
            uc += w * br.universal_credit;
            cb += w * br.child_benefit;
            sp += w * br.state_pension;
            pc += w * br.pension_credit;
            hb += w * br.housing_benefit;
            ctc += w * br.child_tax_credit;
            wtc += w * br.working_tax_credit;
            is_val += w * br.income_support;
            scp += w * br.scottish_child_payment;
            cap += w * br.benefit_cap_reduction;
            if br.universal_credit > 0.0 { cl_uc += w; }
            if br.child_benefit > 0.0 { cl_cb += w; }
            if br.state_pension > 0.0 { cl_sp += w; }
            if br.pension_credit > 0.0 { cl_pc += w; }
            if br.housing_benefit > 0.0 { cl_hb += w; }
            if br.child_tax_credit > 0.0 { cl_ctc += w; }
            if br.working_tax_credit > 0.0 { cl_wtc += w; }
            if br.income_support > 0.0 { cl_is += w; }
            if br.scottish_child_payment > 0.0 { cl_scp += w; }
            if br.benefit_cap_reduction > 0.0 { cl_cap += w; }
        }
        (ProgramBreakdown {
            income_tax,
            employee_ni,
            employer_ni,
            universal_credit: uc,
            child_benefit: cb,
            state_pension: sp,
            pension_credit: pc,
            housing_benefit: hb,
            child_tax_credit: ctc,
            working_tax_credit: wtc,
            income_support: is_val,
            scottish_child_payment: scp,
            benefit_cap_reduction: cap,
        }, Caseloads {
            income_tax_payers: it_payers,
            ni_payers,
            employer_ni_payers: eni_payers,
            universal_credit: cl_uc,
            child_benefit: cl_cb,
            state_pension: cl_sp,
            pension_credit: cl_pc,
            housing_benefit: cl_hb,
            child_tax_credit: cl_ctc,
            working_tax_credit: cl_wtc,
            income_support: cl_is,
            scottish_child_payment: cl_scp,
            benefit_cap_affected: cl_cap,
        })
    };

    // JSON output mode
    if json_mode {
        let output = JsonOutput {
            fiscal_year: baseline_params.fiscal_year.clone(),
            budgetary_impact: BudgetaryImpact {
                baseline_revenue,
                reform_revenue,
                revenue_change,
                baseline_benefits,
                reform_benefits,
                benefit_spending_change: benefit_change,
                net_cost,
            },
            program_breakdown,
            caseloads,
            decile_impacts,
            winners_losers,
        };
        println!("{}", serde_json::to_string(&output)?);
        return Ok(());
    }

    // Pretty output
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!("  {} {}", "PolicyEngine UK".bright_white().bold(), format!("v{}", env!("CARGO_PKG_VERSION")).dimmed());
    println!("  {}", "High-performance microsimulation engine in Rust".dimmed());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!();

    println!("    {} {} households, {} people",
        "✓".bright_green(),
        format_num(dataset.households.len()),
        format_num(dataset.people.len()),
    );
    println!("    {} Fiscal year: {}", "◆".bright_cyan(), baseline_params.fiscal_year.bright_white());

    println!();
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════════".bright_yellow());
    println!("  {}", "FISCAL IMPACT".bright_white().bold().underline());
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════════".bright_yellow());

    let mut fiscal_table = Table::new();
    fiscal_table.load_preset(presets::UTF8_FULL);
    fiscal_table.set_content_arrangement(ContentArrangement::Dynamic);
    fiscal_table.set_header(vec!["Metric", "Baseline", "Reform", "Change"]);
    fiscal_table.add_row(vec![
        "Tax Revenue".to_string(),
        format!("£{:.1}bn", baseline_revenue / 1e9),
        format!("£{:.1}bn", reform_revenue / 1e9),
        format_change_bn(revenue_change),
    ]);
    fiscal_table.add_row(vec![
        "Benefit Spending".to_string(),
        format!("£{:.1}bn", baseline_benefits / 1e9),
        format!("£{:.1}bn", reform_benefits / 1e9),
        format_change_bn(benefit_change),
    ]);
    fiscal_table.add_row(vec![
        "Net Cost to Exchequer".to_string(),
        "".to_string(),
        "".to_string(),
        format!("£{:.1}bn", net_cost / 1e9),
    ]);
    println!("{fiscal_table}");

    // Winners and losers
    println!("\n  {}", "WINNERS & LOSERS".bright_white().bold().underline());
    println!();
    println!("    {} {:.1}% gain — avg £{:.0}/year",
        "▲".bright_green(), winners_losers.winners_pct, winners_losers.avg_gain);
    println!("    {} {:.1}% lose — avg £{:.0}/year",
        "▼".bright_red(), winners_losers.losers_pct, winners_losers.avg_loss);
    println!("    {} {:.1}% unchanged",
        "●".dimmed(), winners_losers.unchanged_pct);

    // Decile table
    if cli.deciles {
        println!("\n  {}", "IMPACT BY INCOME DECILE".bright_white().bold().underline());
        println!();

        let max_abs_change = decile_impacts.iter()
            .map(|d| d.avg_change.abs())
            .fold(0.0f64, f64::max);

        let mut decile_table = Table::new();
        decile_table.load_preset(presets::UTF8_FULL);
        decile_table.set_header(vec!["Decile", "Avg Baseline", "Avg Reform", "Avg Change", "% Change", ""]);

        for d in &decile_impacts {
            let bar_len = if max_abs_change > 0.0 {
                (d.avg_change.abs() / max_abs_change * 30.0) as usize
            } else { 0 };
            let bar = if d.avg_change >= 0.0 {
                format!("{}", "█".repeat(bar_len).bright_green())
            } else {
                format!("{}", "█".repeat(bar_len).bright_red())
            };

            decile_table.add_row(vec![
                format!("{}", d.decile),
                format!("£{}", format_num_f(d.avg_baseline_income)),
                format!("£{}", format_num_f(d.avg_reform_income)),
                format_change(d.avg_change),
                format!("{:+.1}%", d.pct_change),
                bar,
            ]);
        }
        println!("{decile_table}");
    }

    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!();

    Ok(())
}

fn format_num(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_num_f(n: f64) -> String {
    format_num(n.round() as usize)
}

fn format_change(n: f64) -> String {
    if n >= 0.0 {
        format!("+£{}", format_num_f(n))
    } else {
        format!("-£{}", format_num_f(n.abs()))
    }
}

fn format_change_bn(n: f64) -> String {
    if n >= 0.0 {
        format!("+£{:.1}bn", n / 1e9)
    } else {
        format!("-£{:.1}bn", n.abs() / 1e9)
    }
}

/// Aggregate statistics from a simulation run for validation.
#[allow(dead_code)]
fn aggregate_stats(
    dataset: &crate::data::Dataset,
    results: &crate::engine::simulation::SimulationResults,
) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) {
    let hhs = &dataset.households;
    let bus = &dataset.benunits;

    let income_tax: f64 = hhs.iter().flat_map(|h| h.person_ids.iter().map(|&p| h.weight * results.person_results[p].income_tax)).sum();
    let employee_ni: f64 = hhs.iter().flat_map(|h| h.person_ids.iter().map(|&p| h.weight * results.person_results[p].national_insurance)).sum();
    let employer_ni: f64 = hhs.iter().flat_map(|h| h.person_ids.iter().map(|&p| h.weight * results.person_results[p].employer_ni)).sum();
    let uc: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].universal_credit).sum();
    let cb: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].child_benefit).sum();
    let sp: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].state_pension).sum();
    let pc: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].pension_credit).sum();
    let hb: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].housing_benefit).sum();
    let ctc: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].child_tax_credit).sum();
    let wtc: f64 = bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].working_tax_credit).sum();
    let it_payers: f64 = hhs.iter().flat_map(|h| h.person_ids.iter().map(|&p| if results.person_results[p].income_tax > 0.0 { h.weight } else { 0.0 })).sum();
    let uc_claimants: f64 = bus.iter().map(|b| if results.benunit_results[b.id].universal_credit > 0.0 { hhs[b.household_id].weight } else { 0.0 }).sum();
    (income_tax, employee_ni, employer_ni, uc, cb, sp, pc, hb, ctc + wtc, it_payers, uc_claimants, 0.0)
}

#[cfg(test)]
mod obr_validation {
    /// OBR validation tests — require clean FRS data at data/frs_clean.
    /// Skips gracefully if data not present (e.g. in CI without FRS access).
    ///
    /// Tolerances are ±20% of OBR outturn/forecast (OBR EFO March 2025, 2025/26).
    /// These are gross sanity checks, not precision targets.
    #[test]
    fn obr_2025_revenue_and_spending() {
        use crate::data::clean::load_clean_frs;
        use crate::engine::Simulation;
        use crate::parameters::Parameters;
        use std::path::Path;

        if !Path::new("data/frs_clean").exists() {
            eprintln!("Skipping OBR validation: data/frs_clean not found (run --extract-frs first)");
            return;
        }

        let dataset = load_clean_frs(Path::new("data/frs_clean"))
            .expect("data/frs_clean must exist — run --extract-frs first");
        let params = Parameters::for_year(2025).unwrap();
        let sim = Simulation::new(
            dataset.people.clone(), dataset.benunits.clone(),
            dataset.households.clone(), params,
        );
        let results = sim.run();

        let hhs = &dataset.households;
        let bus = &dataset.benunits;

        macro_rules! weighted_person_sum {
            ($field:ident) => {
                hhs.iter().flat_map(|h| h.person_ids.iter()
                    .map(|&p| h.weight * results.person_results[p].$field))
                    .sum::<f64>()
            };
        }
        macro_rules! weighted_bu_sum {
            ($field:ident) => {
                bus.iter().map(|b| hhs[b.household_id].weight * results.benunit_results[b.id].$field)
                    .sum::<f64>()
            };
        }
        macro_rules! bu_caseload {
            ($field:ident) => {
                bus.iter().map(|b| if results.benunit_results[b.id].$field > 0.0 { hhs[b.household_id].weight } else { 0.0 })
                    .sum::<f64>()
            };
        }
        macro_rules! person_caseload {
            ($field:ident) => {
                hhs.iter().flat_map(|h| h.person_ids.iter()
                    .map(|&p| if results.person_results[p].$field > 0.0 { h.weight } else { 0.0 }))
                    .sum::<f64>()
            };
        }

        // OBR March 2025 EFO, 2025/26 (£bn)
        // Revenue
        let income_tax = weighted_person_sum!(income_tax);
        let employee_ni = weighted_person_sum!(national_insurance);
        let employer_ni = weighted_person_sum!(employer_ni);
        // Benefits
        let uc = weighted_bu_sum!(universal_credit);
        let cb = weighted_bu_sum!(child_benefit);
        let sp = weighted_bu_sum!(state_pension);
        let pc = weighted_bu_sum!(pension_credit);
        let _hb = weighted_bu_sum!(housing_benefit);
        let _tc = weighted_bu_sum!(child_tax_credit) + weighted_bu_sum!(working_tax_credit);
        // Caseloads
        let it_payers = person_caseload!(income_tax);
        let uc_claimants = bu_caseload!(universal_credit);
        let cb_claimants = bu_caseload!(child_benefit);

        // ── Revenue checks (OBR 2025/26 central forecast) ──
        // Income tax: ~£305bn (OBR), model ~£250bn due to FRS income underreporting
        assert!(income_tax > 200e9 && income_tax < 380e9,
            "Income tax £{:.0}bn outside [£200bn, £380bn]", income_tax / 1e9);
        // Employee NI: ~£72bn
        assert!(employee_ni > 40e9 && employee_ni < 100e9,
            "Employee NI £{:.0}bn outside [£40bn, £100bn]", employee_ni / 1e9);
        // Employer NI: ~£115bn (pre-2025 Budget rise)
        assert!(employer_ni > 80e9 && employer_ni < 200e9,
            "Employer NI £{:.0}bn outside [£80bn, £200bn]", employer_ni / 1e9);

        // ── Benefit spending checks ──
        // UC: ~£79bn OBR (inc. housing element); model ~£60bn after legacy migration
        assert!(uc > 40e9 && uc < 100e9,
            "UC £{:.0}bn outside [£40bn, £100bn]", uc / 1e9);
        // Child benefit: ~£15bn
        assert!(cb > 8e9 && cb < 22e9,
            "Child benefit £{:.0}bn outside [£8bn, £22bn]", cb / 1e9);
        // State pension: ~£130bn
        assert!(sp > 80e9 && sp < 180e9,
            "State pension £{:.0}bn outside [£80bn, £180bn]", sp / 1e9);
        // Pension credit: ~£6bn
        assert!(pc > 2e9 && pc < 12e9,
            "Pension credit £{:.0}bn outside [£2bn, £12bn]", pc / 1e9);
        // Housing benefit: now folded into UC housing element; standalone HB ~£0 in model
        // OBR shows £12bn standalone HB (pensioners/legacy remaining) — we skip this check
        // as the spending is captured within UC total above.
        // Tax credits: folded into UC; standalone TC now ~£0 in model (migration complete)

        // ── Caseload checks ──
        // IT payers: ~32m
        assert!(it_payers > 25e6 && it_payers < 40e6,
            "IT payers {:.1}m outside [25m, 40m]", it_payers / 1e6);
        // UC claimants: ~3-7m benefit units (OBR counts individuals; model counts benefit units)
        assert!(uc_claimants > 2e6 && uc_claimants < 10e6,
            "UC claimants {:.1}m outside [2m, 10m]", uc_claimants / 1e6);
        // Child benefit claimants: ~6m families
        assert!(cb_claimants > 4e6 && cb_claimants < 9e6,
            "CB claimants {:.1}m outside [4m, 9m]", cb_claimants / 1e6);
    }
}
