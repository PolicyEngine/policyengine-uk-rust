use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;
use crate::data::frs::{load_table_cols, get_f64, get_i64};

/// Parse Wealth and Assets Survey (WAS) microdata from UKDS tab-delimited files.
///
/// WAS is a household-level survey focused on wealth, savings, and assets.
/// Income data is available only as household aggregates (allocated to the household head).
/// Individual-level ages are not available in the EUL version — we use synthetic ages.
///
/// WAS does not sample Northern Ireland; NI region codes are mapped to Wales.
///
/// Expected directory: contains a file matching `was_round_*_hhold_*.tab`.
/// Also loads directly if the file is named in a standard WAS pattern.
pub fn load_was(data_dir: &Path, fiscal_year: u32) -> anyhow::Result<Dataset> {
    let file_name = find_was_file(data_dir)?;
    let round = detect_round(&file_name);

    // Build column names with round suffix
    let weight_col = format!("r{}xshhwgt", round);
    let region_col = format!("gorr{}", round);
    let adults_col = format!("numadultw{}", round);
    let children_col = format!("numch18w{}", round);
    let emp_income_col = format!("dvgiempr{}_aggr", round);
    let se_income_col = format!("dvgiser{}_aggr", round);
    let pension_income_col = format!("dvgippenr{}_aggr", round);
    let invest_income_col = format!("dvgiinvr{}_aggr", round);
    let council_tax_col = format!("ctamtw{}", round);

    let needed: Vec<&str> = vec![
        &weight_col, &region_col, &adults_col, &children_col,
        &emp_income_col, &se_income_col, &pension_income_col, &invest_income_col,
        &council_tax_col,
    ];

    let table = load_table_cols(data_dir, &file_name, Some(&needed))?;

    let mut people = Vec::new();
    let mut benunits = Vec::new();
    let mut households = Vec::new();

    for row in &table {
        let weight = get_f64(row, &weight_col);
        if weight <= 0.0 { continue; }

        let region_code = get_i64(row, &region_col);
        // WAS doesn't sample NI — map NI (13) to Wales (11)
        let region = was_region(region_code);

        let num_adults = get_i64(row, &adults_col).max(1) as usize;
        let num_children = get_i64(row, &children_col).max(0) as usize;

        // Household-level income (annual in WAS)
        let employment_income = get_f64(row, &emp_income_col).max(0.0);
        let self_employment_income = get_f64(row, &se_income_col).max(0.0);
        let pension_income = get_f64(row, &pension_income_col).max(0.0);
        let investment_income = get_f64(row, &invest_income_col).max(0.0);
        let council_tax = get_f64(row, &council_tax_col).max(0.0);

        let hh_id = households.len();
        let bu_id = benunits.len();
        let mut hh_person_ids = Vec::new();

        // Create adult persons — allocate all income to the household head
        for i in 0..num_adults {
            let pid = people.len();
            hh_person_ids.push(pid);
            let is_head = i == 0;

            let person = Person {
                id: pid,
                benunit_id: bu_id,
                household_id: hh_id,
                age: 40.0,  // WAS EUL has no individual ages
                gender: if i % 2 == 0 { Gender::Male } else { Gender::Female },
                is_benunit_head: is_head,
                is_household_head: is_head,
                is_in_scotland: region.is_scotland(),
                // All income allocated to household head
                employment_income: if is_head { employment_income } else { 0.0 },
                self_employment_income: if is_head { self_employment_income } else { 0.0 },
                pension_income: if is_head { pension_income } else { 0.0 },
                savings_interest_income: if is_head { investment_income } else { 0.0 },
                ..Person::default()
            };
            people.push(person);
        }

        // Create child persons
        for _ in 0..num_children {
            let pid = people.len();
            hh_person_ids.push(pid);

            let person = Person {
                id: pid,
                benunit_id: bu_id,
                household_id: hh_id,
                age: 8.0,
                gender: Gender::Male,
                is_in_scotland: region.is_scotland(),
                ..Person::default()
            };
            people.push(person);
        }

        let benunit = BenUnit {
            id: bu_id,
            household_id: hh_id,
            person_ids: hh_person_ids.clone(),
            ..BenUnit::default()
        };
        benunits.push(benunit);

        let household = Household {
            id: hh_id,
            benunit_ids: vec![bu_id],
            person_ids: hh_person_ids,
            weight,
            region,
            council_tax,
            ..Household::default()
        };
        households.push(household);
    }

    Ok(Dataset {
        people,
        benunits,
        households,
        name: format!("Wealth and Assets Survey Round {} ({}/{})", round, fiscal_year, (fiscal_year + 1) % 100),
        year: fiscal_year,
    })
}

/// Find the WAS household tab file in the directory.
/// Searches for files matching common WAS naming patterns.
fn find_was_file(data_dir: &Path) -> anyhow::Result<String> {
    let entries = std::fs::read_dir(data_dir)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("hhold") && (name.ends_with(".tab") || name.ends_with(".csv")) {
            let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(name);
            return Ok(stem);
        }
    }
    anyhow::bail!("No WAS household file (*hhold*.tab) found in {:?}", data_dir)
}

/// Detect which WAS round from the filename (e.g. "round_7" → 7, "round_8" → 8).
/// Defaults to 7 if not detectable.
fn detect_round(file_name: &str) -> u32 {
    // Try to find "round_N" or "r{N}" pattern
    let lower = file_name.to_lowercase();
    if let Some(pos) = lower.find("round_") {
        let after = &lower[pos + 6..];
        if let Some(digit) = after.chars().next().and_then(|c| c.to_digit(10)) {
            return digit;
        }
    }
    // Try "_rN_" pattern
    for r in (5..=9).rev() {
        if lower.contains(&format!("r{}", r)) {
            return r;
        }
    }
    7 // default to Round 7
}

/// Map WAS region code to Region. WAS does not sample Northern Ireland.
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
        13 => Region::Wales,  // NI → Wales (WAS doesn't sample NI)
        _ => Region::London,
    }
}
