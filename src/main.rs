mod engine;
mod parameters;
mod variables;
mod data;
mod reforms;

use clap::Parser;
use colored::Colorize;
use comfy_table::{Table, ContentArrangement, presets};
use std::path::PathBuf;
use std::time::Instant;

use crate::engine::Simulation;
use crate::parameters::Parameters;
use crate::reforms::Reform;
use crate::data::synthetic::generate_synthetic_frs;
use crate::data::frs::load_frs;

#[derive(Parser)]
#[command(name = "policyengine-uk")]
#[command(about = "High-performance UK tax-benefit microsimulation engine")]
#[command(version)]
struct Cli {
    /// Reform file (YAML). If omitted, runs the default PA=£20k reform.
    #[arg(short, long)]
    reform: Option<PathBuf>,

    /// Path to FRS CSV data directory (e.g. data/UKDA-9367-csv/csv/).
    /// If omitted, uses synthetic data.
    #[arg(long)]
    frs: Option<PathBuf>,

    /// Fiscal year start (e.g. 2029 for FY 2029/30).
    /// Available: 2023-2029.
    #[arg(short, long, default_value = "2029")]
    year: u32,

    /// Number of synthetic households (more = slower but more precise)
    #[arg(short = 'n', long, default_value = "20000")]
    households: usize,

    /// Export baseline parameters to YAML (useful for writing reforms)
    #[arg(long)]
    export_baseline: bool,

    /// Show per-decile breakdown
    #[arg(long, default_value = "true")]
    deciles: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load baseline parameters for the chosen fiscal year
    let baseline_params = Parameters::for_year(cli.year)?;

    if cli.export_baseline {
        println!("{}", baseline_params.to_yaml());
        println!("\n{}", "# Copy this file and modify values to create a reform.".dimmed());
        println!("{}", "# Only include the sections/values you want to change.".dimmed());
        println!("{}", "# Run with: policyengine-uk --reform my_reform.yaml".dimmed());
        return Ok(());
    }

    // Header
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!("  {} {}", "PolicyEngine UK".bright_white().bold(), format!("v{}", env!("CARGO_PKG_VERSION")).dimmed());
    println!("  {}", "High-performance microsimulation engine in Rust".dimmed());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!();

    // Load dataset
    let t_data = Instant::now();
    let dataset = if let Some(frs_path) = &cli.frs {
        println!("  {} Loading FRS microdata from {}...", "▸".bright_cyan(), frs_path.display());
        load_frs(frs_path)?
    } else {
        println!("  {} Generating synthetic population...", "▸".bright_cyan());
        generate_synthetic_frs(cli.year)
    };
    let data_time = t_data.elapsed();
    println!("    {} {} households, {} people, representing {:.1}M households",
        "✓".bright_green(),
        format_num(dataset.households.len()),
        format_num(dataset.people.len()),
        dataset.weighted_population() / 1e6,
    );
    println!("    {} Dataset: {}", "◆".bright_cyan(), dataset.name.bright_white());
    println!("    {} Fiscal year: {}", "◆".bright_cyan(), baseline_params.fiscal_year.bright_white());
    println!("    {} in {:.0}ms", "⏱".dimmed(), data_time.as_millis());

    // Load reform
    let reform = if let Some(path) = &cli.reform {
        println!("\n  {} Loading reform from {}", "▸".bright_cyan(), path.display());
        Reform::from_file(path, &baseline_params)?
    } else {
        println!("\n  {} Using default reform: {}", "▸".bright_cyan(), "Personal Allowance → £20,000".bright_yellow());
        Reform::personal_allowance_20k(&baseline_params)
    };

    // Run baseline
    println!("\n  {} Running baseline simulation...", "▸".bright_cyan());
    let t_base = Instant::now();
    let baseline_sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        baseline_params.clone(),
    );
    let baseline = baseline_sim.run();
    let base_time = t_base.elapsed();
    println!("    {} Baseline complete in {:.0}ms", "✓".bright_green(), base_time.as_millis());

    // Run reform
    println!("  {} Running reform simulation...", "▸".bright_cyan());
    let t_reform = Instant::now();
    let reform_sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        reform.parameters.clone(),
    );
    let reformed = reform_sim.run();
    let reform_time = t_reform.elapsed();
    println!("    {} Reform complete in {:.0}ms", "✓".bright_green(), reform_time.as_millis());

    // ═══════════════════════════════════════════════════════════════
    // ANALYSIS
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════════".bright_yellow());
    println!("  {} {}", "Reform:".bright_white().bold(), reform.name.bright_yellow().bold());
    println!("  {} {} → {}", "Change:".bright_white(),
        format!("PA = £{}", format_num_f(baseline_params.income_tax.personal_allowance)).dimmed(),
        format!("PA = £{}", format_num_f(reform.parameters.income_tax.personal_allowance)).bright_green().bold());
    println!("  {} {}", "Year:".bright_white(), baseline_params.fiscal_year);
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════════".bright_yellow());

    // Aggregate impacts
    let households = &dataset.households;

    let baseline_revenue: f64 = households.iter()
        .map(|h| h.weight * baseline.household_results[h.id].total_tax)
        .sum();
    let reform_revenue: f64 = households.iter()
        .map(|h| h.weight * reformed.household_results[h.id].total_tax)
        .sum();
    let revenue_impact = reform_revenue - baseline_revenue;

    let baseline_benefits: f64 = households.iter()
        .map(|h| h.weight * baseline.household_results[h.id].total_benefits)
        .sum();
    let reform_benefits: f64 = households.iter()
        .map(|h| h.weight * reformed.household_results[h.id].total_benefits)
        .sum();
    let benefit_impact = reform_benefits - baseline_benefits;

    let net_cost = -revenue_impact + benefit_impact;

    println!("\n  {}", "FISCAL IMPACT".bright_white().bold().underline());
    println!();

    let mut fiscal_table = Table::new();
    fiscal_table.load_preset(presets::UTF8_FULL);
    fiscal_table.set_content_arrangement(ContentArrangement::Dynamic);
    fiscal_table.set_header(vec!["Metric", "Baseline", "Reform", "Change"]);
    fiscal_table.add_row(vec![
        "Tax Revenue".to_string(),
        format!("£{:.1}bn", baseline_revenue / 1e9),
        format!("£{:.1}bn", reform_revenue / 1e9),
        format_change_bn(revenue_impact),
    ]);
    fiscal_table.add_row(vec![
        "Benefit Spending".to_string(),
        format!("£{:.1}bn", baseline_benefits / 1e9),
        format!("£{:.1}bn", reform_benefits / 1e9),
        format_change_bn(benefit_impact),
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
    println!("    {} {:.1}% of households ({:.1}M) gain — avg £{:.0}/year",
        "▲".bright_green(),
        100.0 * winners / total_hh,
        winners / 1e6,
        if winners > 0.0 { total_gain / winners } else { 0.0 });
    println!("    {} {:.1}% of households ({:.1}M) lose — avg £{:.0}/year",
        "▼".bright_red(),
        100.0 * losers / total_hh,
        losers / 1e6,
        if losers > 0.0 { total_loss.abs() / losers } else { 0.0 });
    println!("    {} {:.1}% of households ({:.1}M) unchanged",
        "●".dimmed(),
        100.0 * unchanged / total_hh,
        unchanged / 1e6);

    // Decile analysis
    if cli.deciles {
        println!("\n  {}", "IMPACT BY INCOME DECILE".bright_white().bold().underline());
        println!();

        let mut hh_incomes: Vec<(usize, f64, f64)> = households.iter().map(|hh| {
            let base_inc = baseline.household_results[hh.id].net_income;
            let reform_inc = reformed.household_results[hh.id].net_income;
            (hh.id, base_inc, reform_inc)
        }).collect();
        hh_incomes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let decile_size = hh_incomes.len() / 10;
        let mut decile_table = Table::new();
        decile_table.load_preset(presets::UTF8_FULL);
        decile_table.set_header(vec!["Decile", "Avg Baseline", "Avg Reform", "Avg Change", "% Change", ""]);

        let max_bar_width = 30;
        let mut max_abs_change = 0.0f64;
        let mut decile_data = Vec::new();

        for d in 0..10 {
            let start = d * decile_size;
            let end = if d == 9 { hh_incomes.len() } else { (d + 1) * decile_size };
            let slice = &hh_incomes[start..end];
            let n = slice.len() as f64;

            let avg_base: f64 = slice.iter().map(|h| h.1).sum::<f64>() / n;
            let avg_reform: f64 = slice.iter().map(|h| h.2).sum::<f64>() / n;
            let avg_change = avg_reform - avg_base;
            let pct_change = if avg_base != 0.0 { 100.0 * avg_change / avg_base } else { 0.0 };

            if avg_change.abs() > max_abs_change {
                max_abs_change = avg_change.abs();
            }
            decile_data.push((d + 1, avg_base, avg_reform, avg_change, pct_change));
        }

        for (decile, avg_base, avg_reform, avg_change, pct_change) in &decile_data {
            let bar_len = if max_abs_change > 0.0 {
                (avg_change.abs() / max_abs_change * max_bar_width as f64) as usize
            } else { 0 };
            let bar = if *avg_change >= 0.0 {
                format!("{}", "█".repeat(bar_len).bright_green())
            } else {
                format!("{}", "█".repeat(bar_len).bright_red())
            };

            decile_table.add_row(vec![
                format!("{}", decile),
                format!("£{}", format_num_f(*avg_base)),
                format!("£{}", format_num_f(*avg_reform)),
                format_change(*avg_change),
                format!("{:+.1}%", pct_change),
                bar,
            ]);
        }
        println!("{decile_table}");
    }

    // Performance summary
    println!("\n  {}", "PERFORMANCE".bright_white().bold().underline());
    println!();
    let total_time = base_time + reform_time + data_time;
    let hh_per_sec = (2 * dataset.households.len()) as f64 / (base_time + reform_time).as_secs_f64();
    println!("    {} Data generation:  {:.0}ms", "⏱".dimmed(), data_time.as_millis());
    println!("    {} Baseline sim:     {:.0}ms", "⏱".dimmed(), base_time.as_millis());
    println!("    {} Reform sim:       {:.0}ms", "⏱".dimmed(), reform_time.as_millis());
    println!("    {} Total:            {:.0}ms", "⏱".dimmed(), total_time.as_millis());
    println!("    {} Throughput:       {} households/sec", "⚡".bright_yellow(), format_num(hh_per_sec as usize));
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
