use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;

/// Write a Dataset to clean CSVs with descriptive column names.
///
/// Produces three files in `output_dir`:
///   - persons.csv: one row per person, annual values
///   - benunits.csv: one row per benefit unit
///   - households.csv: one row per household
///
/// All monetary values are ANNUAL (already converted from FRS weekly).
pub fn write_clean_csvs(dataset: &Dataset, output_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_dir)?;

    write_persons(dataset, output_dir)?;
    write_benunits(dataset, output_dir)?;
    write_households(dataset, output_dir)?;

    Ok(())
}

fn write_persons(dataset: &Dataset, output_dir: &Path) -> anyhow::Result<()> {
    let path = output_dir.join("persons.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        "person_id", "benunit_id", "household_id",
        "age", "gender", "is_benunit_head", "is_household_head",
        // Income (annual)
        "employment_income", "self_employment_income",
        "private_pension_income", "state_pension",
        "savings_interest", "dividend_income",
        "property_income", "maintenance_income",
        "miscellaneous_income", "other_income",
        // Employment
        "is_in_scotland", "hours_worked_annual",
        // Status
        "is_disabled", "is_enhanced_disabled", "is_severely_disabled", "is_carer",
        // Contributions (annual)
        "employee_pension_contributions", "personal_pension_contributions",
        "childcare_expenses",
        // Reported benefits (annual)
        "child_benefit_reported", "housing_benefit_reported",
        "income_support_reported", "pension_credit_reported",
        "child_tax_credit_reported", "working_tax_credit_reported",
        "universal_credit_reported",
        "dla_self_care_reported", "dla_mobility_reported",
        "pip_daily_living_reported", "pip_mobility_reported",
        "carers_allowance_reported", "attendance_allowance_reported",
        "esa_income_reported", "esa_contributory_reported",
        "jsa_income_reported", "jsa_contributory_reported",
        // Flags
        "would_claim_marriage_allowance",
    ])?;

    for p in &dataset.people {
        wtr.write_record(&[
            p.id.to_string(),
            p.benunit_id.to_string(),
            p.household_id.to_string(),
            format!("{:.0}", p.age),
            if p.gender == Gender::Male { "male".to_string() } else { "female".to_string() },
            p.is_benunit_head.to_string(),
            p.is_household_head.to_string(),
            format!("{:.2}", p.employment_income),
            format!("{:.2}", p.self_employment_income),
            format!("{:.2}", p.pension_income),
            format!("{:.2}", p.state_pension_reported),
            format!("{:.2}", p.savings_interest_income),
            format!("{:.2}", p.dividend_income),
            format!("{:.2}", p.property_income),
            format!("{:.2}", p.maintenance_income),
            format!("{:.2}", p.miscellaneous_income),
            format!("{:.2}", p.other_income),
            p.is_in_scotland.to_string(),
            format!("{:.1}", p.hours_worked),
            p.is_disabled.to_string(),
            p.is_enhanced_disabled.to_string(),
            p.is_severely_disabled.to_string(),
            p.is_carer.to_string(),
            format!("{:.2}", p.employee_pension_contributions),
            format!("{:.2}", p.personal_pension_contributions),
            format!("{:.2}", p.childcare_expenses),
            format!("{:.2}", p.child_benefit_reported),
            format!("{:.2}", p.housing_benefit_reported),
            format!("{:.2}", p.income_support_reported),
            format!("{:.2}", p.pension_credit_reported),
            format!("{:.2}", p.child_tax_credit_reported),
            format!("{:.2}", p.working_tax_credit_reported),
            format!("{:.2}", p.universal_credit_reported),
            format!("{:.2}", p.dla_sc_reported),
            format!("{:.2}", p.dla_m_reported),
            format!("{:.2}", p.pip_dl_reported),
            format!("{:.2}", p.pip_m_reported),
            format!("{:.2}", p.carers_allowance_reported),
            format!("{:.2}", p.attendance_allowance_reported),
            format!("{:.2}", p.esa_income_reported),
            format!("{:.2}", p.esa_contrib_reported),
            format!("{:.2}", p.jsa_income_reported),
            format!("{:.2}", p.jsa_contrib_reported),
            p.would_claim_marriage_allowance.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn write_benunits(dataset: &Dataset, output_dir: &Path) -> anyhow::Result<()> {
    let path = output_dir.join("benunits.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        "benunit_id", "household_id",
        "person_ids",
        "take_up_seed", "on_uc", "on_legacy",
        "rent_monthly", "is_lone_parent",
    ])?;

    for bu in &dataset.benunits {
        let ids: String = bu.person_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(";");

        wtr.write_record(&[
            bu.id.to_string(),
            bu.household_id.to_string(),
            ids,
            format!("{:.6}", bu.take_up_seed),
            bu.on_uc.to_string(),
            bu.on_legacy.to_string(),
            format!("{:.2}", bu.rent_monthly),
            bu.is_lone_parent.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn write_households(dataset: &Dataset, output_dir: &Path) -> anyhow::Result<()> {
    let path = output_dir.join("households.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        "household_id",
        "benunit_ids", "person_ids",
        "weight", "region",
        "rent_annual", "council_tax_annual",
    ])?;

    for hh in &dataset.households {
        let bu_ids: String = hh.benunit_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(";");
        let p_ids: String = hh.person_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(";");

        wtr.write_record(&[
            hh.id.to_string(),
            bu_ids,
            p_ids,
            format!("{:.4}", hh.weight),
            hh.region.name().to_string(),
            format!("{:.2}", hh.rent),
            format!("{:.2}", hh.council_tax),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Load a Dataset from clean CSVs (produced by write_clean_csvs).
pub fn load_clean_frs(data_dir: &Path) -> anyhow::Result<Dataset> {
    let households = load_households_csv(data_dir)?;
    let benunits = load_benunits_csv(data_dir)?;
    let people = load_persons_csv(data_dir)?;

    Ok(Dataset {
        people,
        benunits,
        households,
        name: "FRS (cleaned)".to_string(),
        year: 2023,
    })
}

fn parse_bool(s: &str) -> bool {
    s == "true" || s == "1"
}

fn parse_f64(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn parse_usize(s: &str) -> usize {
    s.parse::<usize>().unwrap_or(0)
}

fn parse_id_list(s: &str) -> Vec<usize> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(';').filter_map(|x| x.parse::<usize>().ok()).collect()
}

fn parse_region(s: &str) -> Region {
    match s {
        "North East" => Region::NorthEast,
        "North West" => Region::NorthWest,
        "Yorkshire" => Region::Yorkshire,
        "East Midlands" => Region::EastMidlands,
        "West Midlands" => Region::WestMidlands,
        "East of England" => Region::EastOfEngland,
        "London" => Region::London,
        "South East" => Region::SouthEast,
        "South West" => Region::SouthWest,
        "Wales" => Region::Wales,
        "Scotland" => Region::Scotland,
        "Northern Ireland" => Region::NorthernIreland,
        _ => Region::London,
    }
}

fn load_persons_csv(data_dir: &Path) -> anyhow::Result<Vec<Person>> {
    let path = data_dir.join("persons.csv");
    let mut rdr = csv::Reader::from_path(&path)?;
    let mut people = Vec::new();

    for result in rdr.records() {
        let r = result?;
        let mut i = 0;
        let mut next = || -> &str { let v = r.get(i).unwrap_or(""); i += 1; v };

        let person_id = parse_usize(next());
        let benunit_id = parse_usize(next());
        let household_id = parse_usize(next());
        let age = parse_f64(next());
        let gender = if next() == "male" { Gender::Male } else { Gender::Female };
        let is_benunit_head = parse_bool(next());
        let is_household_head = parse_bool(next());
        let employment_income = parse_f64(next());
        let self_employment_income = parse_f64(next());
        let pension_income = parse_f64(next());
        let state_pension_reported = parse_f64(next());
        let savings_interest_income = parse_f64(next());
        let dividend_income = parse_f64(next());
        let property_income = parse_f64(next());
        let maintenance_income = parse_f64(next());
        let miscellaneous_income = parse_f64(next());
        let other_income = parse_f64(next());
        let is_in_scotland = parse_bool(next());
        let hours_worked = parse_f64(next());
        let is_disabled = parse_bool(next());
        let is_enhanced_disabled = parse_bool(next());
        let is_severely_disabled = parse_bool(next());
        let is_carer = parse_bool(next());
        let employee_pension_contributions = parse_f64(next());
        let personal_pension_contributions = parse_f64(next());
        let childcare_expenses = parse_f64(next());
        let child_benefit_reported = parse_f64(next());
        let housing_benefit_reported = parse_f64(next());
        let income_support_reported = parse_f64(next());
        let pension_credit_reported = parse_f64(next());
        let child_tax_credit_reported = parse_f64(next());
        let working_tax_credit_reported = parse_f64(next());
        let universal_credit_reported = parse_f64(next());
        let dla_sc_reported = parse_f64(next());
        let dla_m_reported = parse_f64(next());
        let pip_dl_reported = parse_f64(next());
        let pip_m_reported = parse_f64(next());
        let carers_allowance_reported = parse_f64(next());
        let attendance_allowance_reported = parse_f64(next());
        let esa_income_reported = parse_f64(next());
        let esa_contrib_reported = parse_f64(next());
        let jsa_income_reported = parse_f64(next());
        let jsa_contrib_reported = parse_f64(next());
        let would_claim_marriage_allowance = parse_bool(next());

        people.push(Person {
            id: person_id, benunit_id, household_id,
            age, gender, is_benunit_head, is_household_head,
            employment_income, self_employment_income,
            pension_income, state_pension_reported,
            savings_interest_income, dividend_income,
            property_income, maintenance_income,
            miscellaneous_income, other_income,
            is_in_scotland, hours_worked,
            is_disabled, is_enhanced_disabled, is_severely_disabled, is_carer,
            employee_pension_contributions, personal_pension_contributions,
            childcare_expenses,
            child_benefit_reported, housing_benefit_reported,
            income_support_reported, pension_credit_reported,
            child_tax_credit_reported, working_tax_credit_reported,
            universal_credit_reported,
            dla_sc_reported, dla_m_reported,
            pip_dl_reported, pip_m_reported,
            carers_allowance_reported, attendance_allowance_reported,
            esa_income_reported, esa_contrib_reported,
            jsa_income_reported, jsa_contrib_reported,
            would_claim_marriage_allowance,
        });
    }

    Ok(people)
}

fn load_benunits_csv(data_dir: &Path) -> anyhow::Result<Vec<BenUnit>> {
    let path = data_dir.join("benunits.csv");
    let mut rdr = csv::Reader::from_path(&path)?;
    let mut benunits = Vec::new();

    for result in rdr.records() {
        let r = result?;
        let mut i = 0;
        let mut next = || -> &str { let v = r.get(i).unwrap_or(""); i += 1; v };

        let id = parse_usize(next());
        let household_id = parse_usize(next());
        let person_ids = parse_id_list(next());
        let take_up_seed = parse_f64(next());
        let on_uc = parse_bool(next());
        let on_legacy = parse_bool(next());
        let rent_monthly = parse_f64(next());
        let is_lone_parent = parse_bool(next());

        benunits.push(BenUnit {
            id, household_id, person_ids,
            take_up_seed, on_uc, on_legacy, rent_monthly, is_lone_parent,
        });
    }

    Ok(benunits)
}

fn load_households_csv(data_dir: &Path) -> anyhow::Result<Vec<Household>> {
    let path = data_dir.join("households.csv");
    let mut rdr = csv::Reader::from_path(&path)?;
    let mut households = Vec::new();

    for result in rdr.records() {
        let r = result?;
        let mut i = 0;
        let mut next = || -> &str { let v = r.get(i).unwrap_or(""); i += 1; v };

        let id = parse_usize(next());
        let benunit_ids = parse_id_list(next());
        let person_ids = parse_id_list(next());
        let weight = parse_f64(next());
        let region = parse_region(next());
        let rent = parse_f64(next());
        let council_tax = parse_f64(next());

        households.push(Household {
            id, benunit_ids, person_ids,
            weight, region, rent, council_tax,
        });
    }

    Ok(households)
}
