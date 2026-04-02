use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;
use crate::data::frs::{load_table_cols, get_f64, get_i64, region_from_gvtregno};

/// Parse Survey of Personal Incomes (SPI) microdata from UKDS tab-delimited files.
///
/// The SPI is an HMRC administrative dataset of individual tax records. Each row is one
/// taxpayer. There is no household or benefit unit structure — we create synthetic
/// 1-person households.
///
/// SPI values are ANNUAL (unlike FRS which is weekly), so no annualisation is needed.
///
/// Expected file: `put{YYYY}uk.tab` where YYYY = fiscal_year + 1 (e.g. put2023uk.tab for 2022/23).
/// Also tries `put{YYYY}uk.csv` as fallback.
pub fn load_spi(data_dir: &Path, fiscal_year: u32) -> anyhow::Result<Dataset> {
    // SPI files use two naming conventions:
    //   put{end_year}uk  (e.g. put2023uk for 2022/23) — older files
    //   put{yy}{yy+1}uk  (e.g. put2223uk for 2022/23) — newer files
    let end_year = fiscal_year + 1;
    let file_name = find_spi_file(data_dir, fiscal_year)
        .unwrap_or_else(|| format!("put{}uk", end_year));

    let table = load_table_cols(data_dir, &file_name, Some(&[
        "fact", "pay", "epb", "profits", "pension", "srp",
        "incbbs", "dividends", "incprop", "gorcode", "agerange", "sex",
        "mothinc", "incpben", "ossben", "taxterm", "ubisja", "otherinc",
    ]))?;

    let n = table.len();
    let mut people = Vec::with_capacity(n);
    let mut benunits = Vec::with_capacity(n);
    let mut households = Vec::with_capacity(n);

    for (idx, row) in table.iter().enumerate() {
        let weight = get_f64(row, "fact");
        if weight <= 0.0 { continue; }

        let region = region_from_gvtregno(get_i64(row, "gorcode"));

        let person = Person {
            id: idx,
            benunit_id: idx,
            household_id: idx,
            age: age_from_agerange(get_i64(row, "agerange")),
            gender: if get_i64(row, "sex") == 1 { Gender::Male } else { Gender::Female },
            is_benunit_head: true,
            is_household_head: true,
            is_in_scotland: region.is_scotland(),
            // Income (all annual in SPI)
            employment_income: get_f64(row, "pay") + get_f64(row, "epb"),
            self_employment_income: get_f64(row, "profits"),
            pension_income: get_f64(row, "pension"),
            state_pension: get_f64(row, "srp"),
            savings_interest_income: get_f64(row, "incbbs"),
            dividend_income: get_f64(row, "dividends"),
            property_income: get_f64(row, "incprop"),
            miscellaneous_income: get_f64(row, "mothinc")
                + get_f64(row, "incpben")
                + get_f64(row, "ossben")
                + get_f64(row, "taxterm")
                + get_f64(row, "ubisja")
                + get_f64(row, "otherinc"),
            ..Person::default()
        };

        let benunit = BenUnit {
            id: idx,
            household_id: idx,
            person_ids: vec![idx],
            ..BenUnit::default()
        };

        let household = Household {
            id: idx,
            benunit_ids: vec![idx],
            person_ids: vec![idx],
            weight,
            region,
            ..Household::default()
        };

        people.push(person);
        benunits.push(benunit);
        households.push(household);
    }

    // Reindex after skipping zero-weight rows
    for (i, p) in people.iter_mut().enumerate() {
        p.id = i;
        p.benunit_id = i;
        p.household_id = i;
    }
    for (i, bu) in benunits.iter_mut().enumerate() {
        bu.id = i;
        bu.household_id = i;
        bu.person_ids = vec![i];
    }
    for (i, hh) in households.iter_mut().enumerate() {
        hh.id = i;
        hh.benunit_ids = vec![i];
        hh.person_ids = vec![i];
    }

    Ok(Dataset {
        people,
        benunits,
        households,
        name: format!("Survey of Personal Incomes {}/{:02}", fiscal_year, (fiscal_year + 1) % 100),
        year: fiscal_year,
    })
}

/// Find the SPI tab file in the directory, trying both naming conventions.
fn find_spi_file(data_dir: &Path, fiscal_year: u32) -> Option<String> {
    let end_year = fiscal_year + 1;
    // Try two-digit-year range format first: put{yy}{yy+1}uk (e.g. put2223uk)
    let short_start = fiscal_year % 100;
    let short_end = end_year % 100;
    let two_digit = format!("put{:02}{:02}uk", short_start, short_end);
    // Full end-year format: put{YYYY}uk (e.g. put2023uk)
    let full_year = format!("put{}uk", end_year);

    let entries = std::fs::read_dir(data_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&name);
        if stem == two_digit || stem == full_year {
            return Some(stem.to_string());
        }
    }
    None
}

/// Map SPI AGERANGE code to age midpoint.
fn age_from_agerange(code: i64) -> f64 {
    match code {
        1 => 20.0,  // under 25
        2 => 30.0,  // 25-34
        3 => 40.0,  // 35-44
        4 => 50.0,  // 45-54
        5 => 60.0,  // 55-64
        6 => 70.0,  // 65-74
        7 => 82.0,  // 75+
        _ => 43.0,  // unknown / all ages → UK median
    }
}
