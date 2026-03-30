use std::path::Path;
use crate::engine::entities::*;
use crate::engine::simulation::SimulationResults;
use crate::data::Dataset;

/// Write a Dataset to clean CSVs with descriptive column names.
///
/// Produces three files in `output_dir`:
///   - persons.csv: one row per person, annual values
///   - benunits.csv: one row per benefit unit (includes would_claim flags)
///   - households.csv: one row per household
pub fn write_clean_csvs(dataset: &mut Dataset, output_dir: &Path) -> anyhow::Result<()> {
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
        // Disability rate-band flags
        "dla_care_low", "dla_care_mid", "dla_care_high",
        "dla_mob_low", "dla_mob_high",
        "pip_dl_std", "pip_dl_enh",
        "pip_mob_std", "pip_mob_enh",
        "aa_low", "aa_high",
        // Status
        "is_disabled", "is_enhanced_disabled", "is_severely_disabled", "is_carer",
        "limitill", "esa_group", "emp_status", "looking_for_work",
        "is_self_identified_carer",
        // Contributions (annual)
        "employee_pension_contributions", "personal_pension_contributions",
        "childcare_expenses",
        // Benefits (annual)
        "child_benefit", "housing_benefit",
        "income_support", "pension_credit",
        "child_tax_credit", "working_tax_credit",
        "universal_credit",
        "dla_care", "dla_mobility",
        "pip_daily_living", "pip_mobility",
        "carers_allowance", "attendance_allowance",
        "esa_income", "esa_contributory",
        "jsa_income", "jsa_contributory",
        "other_benefits",
        "adp_daily_living", "adp_mobility",
        "cdp_care", "cdp_mobility",
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
            format!("{:.2}", p.state_pension),
            format!("{:.2}", p.savings_interest_income),
            format!("{:.2}", p.dividend_income),
            format!("{:.2}", p.property_income),
            format!("{:.2}", p.maintenance_income),
            format!("{:.2}", p.miscellaneous_income),
            format!("{:.2}", p.other_income),
            p.is_in_scotland.to_string(),
            format!("{:.1}", p.hours_worked),
            p.dla_care_low.to_string(),
            p.dla_care_mid.to_string(),
            p.dla_care_high.to_string(),
            p.dla_mob_low.to_string(),
            p.dla_mob_high.to_string(),
            p.pip_dl_std.to_string(),
            p.pip_dl_enh.to_string(),
            p.pip_mob_std.to_string(),
            p.pip_mob_enh.to_string(),
            p.aa_low.to_string(),
            p.aa_high.to_string(),
            p.is_disabled.to_string(),
            p.is_enhanced_disabled.to_string(),
            p.is_severely_disabled.to_string(),
            p.is_carer.to_string(),
            p.limitill.to_string(),
            p.esa_group.to_string(),
            p.emp_status.to_string(),
            p.looking_for_work.to_string(),
            p.is_self_identified_carer.to_string(),
            format!("{:.2}", p.employee_pension_contributions),
            format!("{:.2}", p.personal_pension_contributions),
            format!("{:.2}", p.childcare_expenses),
            format!("{:.2}", p.child_benefit),
            format!("{:.2}", p.housing_benefit),
            format!("{:.2}", p.income_support),
            format!("{:.2}", p.pension_credit),
            format!("{:.2}", p.child_tax_credit),
            format!("{:.2}", p.working_tax_credit),
            format!("{:.2}", p.universal_credit),
            format!("{:.2}", p.dla_care),
            format!("{:.2}", p.dla_mobility),
            format!("{:.2}", p.pip_daily_living),
            format!("{:.2}", p.pip_mobility),
            format!("{:.2}", p.carers_allowance),
            format!("{:.2}", p.attendance_allowance),
            format!("{:.2}", p.esa_income),
            format!("{:.2}", p.esa_contributory),
            format!("{:.2}", p.jsa_income),
            format!("{:.2}", p.jsa_contributory),
            format!("{:.2}", p.other_benefits),
            format!("{:.2}", p.adp_daily_living),
            format!("{:.2}", p.adp_mobility),
            format!("{:.2}", p.cdp_care),
            format!("{:.2}", p.cdp_mobility),
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
        "migration_seed", "on_uc", "on_legacy",
        "rent_monthly", "is_lone_parent",
        // Would-claim flags (set from reported receipt in FRS)
        "would_claim_uc", "would_claim_cb", "would_claim_hb",
        "would_claim_pc", "would_claim_ctc", "would_claim_wtc",
        "would_claim_is", "would_claim_esa", "would_claim_jsa",
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
            format!("{:.6}", bu.migration_seed),
            bu.on_uc.to_string(),
            bu.on_legacy.to_string(),
            format!("{:.2}", bu.rent_monthly),
            bu.is_lone_parent.to_string(),
            bu.would_claim_uc.to_string(),
            bu.would_claim_cb.to_string(),
            bu.would_claim_hb.to_string(),
            bu.would_claim_pc.to_string(),
            bu.would_claim_ctc.to_string(),
            bu.would_claim_wtc.to_string(),
            bu.would_claim_is.to_string(),
            bu.would_claim_esa.to_string(),
            bu.would_claim_jsa.to_string(),
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

/// Write enhanced microdata: input data + simulation outputs in one CSV per entity.
pub fn write_microdata(
    dataset: &Dataset,
    baseline: &SimulationResults,
    reformed: &SimulationResults,
    output_dir: &Path,
) -> anyhow::Result<()> {
    write_microdata_persons(dataset, baseline, reformed, output_dir)?;
    write_microdata_benunits(dataset, baseline, reformed, output_dir)?;
    write_microdata_households(dataset, baseline, reformed, output_dir)?;
    Ok(())
}

fn write_microdata_persons(
    dataset: &Dataset,
    baseline: &SimulationResults,
    reformed: &SimulationResults,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let path = output_dir.join("persons.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        // IDs
        "person_id", "benunit_id", "household_id",
        // Demographics
        "age", "gender", "is_benunit_head", "is_household_head",
        // Input incomes
        "employment_income", "self_employment_income",
        "private_pension_income", "state_pension",
        "savings_interest", "dividend_income",
        "property_income", "maintenance_income",
        "miscellaneous_income", "other_income",
        // Employment
        "is_in_scotland", "hours_worked_annual",
        // Status
        "is_disabled", "is_carer",
        // Contributions
        "employee_pension_contributions", "personal_pension_contributions",
        "childcare_expenses",
        // Reported benefits
        "child_benefit", "housing_benefit",
        "income_support", "pension_credit",
        "child_tax_credit", "working_tax_credit",
        "universal_credit",
        // ── Baseline outputs ──
        "baseline_income_tax", "baseline_employee_ni", "baseline_employer_ni",
        "baseline_total_income", "baseline_taxable_income",
        "baseline_personal_allowance",
        // ── Reform outputs ──
        "reform_income_tax", "reform_employee_ni", "reform_employer_ni",
        "reform_total_income", "reform_taxable_income",
        "reform_personal_allowance",
    ])?;

    for p in &dataset.people {
        let bl = &baseline.person_results[p.id];
        let rf = &reformed.person_results[p.id];
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
            format!("{:.2}", p.state_pension),
            format!("{:.2}", p.savings_interest_income),
            format!("{:.2}", p.dividend_income),
            format!("{:.2}", p.property_income),
            format!("{:.2}", p.maintenance_income),
            format!("{:.2}", p.miscellaneous_income),
            format!("{:.2}", p.other_income),
            p.is_in_scotland.to_string(),
            format!("{:.1}", p.hours_worked),
            p.is_disabled.to_string(),
            p.is_carer.to_string(),
            format!("{:.2}", p.employee_pension_contributions),
            format!("{:.2}", p.personal_pension_contributions),
            format!("{:.2}", p.childcare_expenses),
            format!("{:.2}", p.child_benefit),
            format!("{:.2}", p.housing_benefit),
            format!("{:.2}", p.income_support),
            format!("{:.2}", p.pension_credit),
            format!("{:.2}", p.child_tax_credit),
            format!("{:.2}", p.working_tax_credit),
            format!("{:.2}", p.universal_credit),
            // Baseline
            format!("{:.2}", bl.income_tax),
            format!("{:.2}", bl.national_insurance),
            format!("{:.2}", bl.employer_ni),
            format!("{:.2}", bl.total_income),
            format!("{:.2}", bl.taxable_income),
            format!("{:.2}", bl.personal_allowance),
            // Reform
            format!("{:.2}", rf.income_tax),
            format!("{:.2}", rf.national_insurance),
            format!("{:.2}", rf.employer_ni),
            format!("{:.2}", rf.total_income),
            format!("{:.2}", rf.taxable_income),
            format!("{:.2}", rf.personal_allowance),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn write_microdata_benunits(
    dataset: &Dataset,
    baseline: &SimulationResults,
    reformed: &SimulationResults,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let path = output_dir.join("benunits.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        // IDs
        "benunit_id", "household_id", "person_ids",
        // Inputs
        "on_uc", "on_legacy", "rent_monthly", "is_lone_parent",
        // ── Baseline outputs ──
        "baseline_universal_credit", "baseline_child_benefit",
        "baseline_state_pension", "baseline_pension_credit",
        "baseline_housing_benefit",
        "baseline_child_tax_credit", "baseline_working_tax_credit",
        "baseline_income_support",
        "baseline_esa_income_related", "baseline_jsa_income_based",
        "baseline_carers_allowance", "baseline_scottish_child_payment",
        "baseline_benefit_cap_reduction", "baseline_passthrough_benefits",
        "baseline_total_benefits",
        // ── Reform outputs ──
        "reform_universal_credit", "reform_child_benefit",
        "reform_state_pension", "reform_pension_credit",
        "reform_housing_benefit",
        "reform_child_tax_credit", "reform_working_tax_credit",
        "reform_income_support",
        "reform_esa_income_related", "reform_jsa_income_based",
        "reform_carers_allowance", "reform_scottish_child_payment",
        "reform_benefit_cap_reduction", "reform_passthrough_benefits",
        "reform_total_benefits",
    ])?;

    for bu in &dataset.benunits {
        let bl = &baseline.benunit_results[bu.id];
        let rf = &reformed.benunit_results[bu.id];
        let ids: String = bu.person_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(";");

        wtr.write_record(&[
            bu.id.to_string(),
            bu.household_id.to_string(),
            ids,
            bu.on_uc.to_string(),
            bu.on_legacy.to_string(),
            format!("{:.2}", bu.rent_monthly),
            bu.is_lone_parent.to_string(),
            // Baseline
            format!("{:.2}", bl.universal_credit),
            format!("{:.2}", bl.child_benefit),
            format!("{:.2}", bl.state_pension),
            format!("{:.2}", bl.pension_credit),
            format!("{:.2}", bl.housing_benefit),
            format!("{:.2}", bl.child_tax_credit),
            format!("{:.2}", bl.working_tax_credit),
            format!("{:.2}", bl.income_support),
            format!("{:.2}", bl.esa_income_related),
            format!("{:.2}", bl.jsa_income_based),
            format!("{:.2}", bl.carers_allowance),
            format!("{:.2}", bl.scottish_child_payment),
            format!("{:.2}", bl.benefit_cap_reduction),
            format!("{:.2}", bl.passthrough_benefits),
            format!("{:.2}", bl.total_benefits),
            // Reform
            format!("{:.2}", rf.universal_credit),
            format!("{:.2}", rf.child_benefit),
            format!("{:.2}", rf.state_pension),
            format!("{:.2}", rf.pension_credit),
            format!("{:.2}", rf.housing_benefit),
            format!("{:.2}", rf.child_tax_credit),
            format!("{:.2}", rf.working_tax_credit),
            format!("{:.2}", rf.income_support),
            format!("{:.2}", rf.esa_income_related),
            format!("{:.2}", rf.jsa_income_based),
            format!("{:.2}", rf.carers_allowance),
            format!("{:.2}", rf.scottish_child_payment),
            format!("{:.2}", rf.benefit_cap_reduction),
            format!("{:.2}", rf.passthrough_benefits),
            format!("{:.2}", rf.total_benefits),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn write_microdata_households(
    dataset: &Dataset,
    baseline: &SimulationResults,
    reformed: &SimulationResults,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let path = output_dir.join("households.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record(&[
        "household_id", "weight", "region",
        "rent_annual", "council_tax_annual",
        // ── Baseline outputs ──
        "baseline_net_income", "baseline_gross_income",
        "baseline_total_tax", "baseline_total_benefits",
        "baseline_equivalisation_factor", "baseline_equivalised_net_income",
        // ── Reform outputs ──
        "reform_net_income", "reform_gross_income",
        "reform_total_tax", "reform_total_benefits",
        "reform_equivalisation_factor", "reform_equivalised_net_income",
    ])?;

    for hh in &dataset.households {
        let bl = &baseline.household_results[hh.id];
        let rf = &reformed.household_results[hh.id];

        wtr.write_record(&[
            hh.id.to_string(),
            format!("{:.4}", hh.weight),
            hh.region.name().to_string(),
            format!("{:.2}", hh.rent),
            format!("{:.2}", hh.council_tax),
            // Baseline
            format!("{:.2}", bl.net_income),
            format!("{:.2}", bl.gross_income),
            format!("{:.2}", bl.total_tax),
            format!("{:.2}", bl.total_benefits),
            format!("{:.4}", bl.equivalisation_factor),
            format!("{:.2}", bl.equivalised_net_income),
            // Reform
            format!("{:.2}", rf.net_income),
            format!("{:.2}", rf.gross_income),
            format!("{:.2}", rf.total_tax),
            format!("{:.2}", rf.total_benefits),
            format!("{:.4}", rf.equivalisation_factor),
            format!("{:.2}", rf.equivalised_net_income),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Load a Dataset from clean CSVs (produced by write_clean_csvs).
pub fn load_clean_frs(data_dir: &Path) -> anyhow::Result<Dataset> {
    let households = load_households_csv(data_dir)?;
    let mut benunits = load_benunits_csv(data_dir)?;
    let people = load_persons_csv(data_dir)?;

    // Derive would_claim_esa/jsa from person data if not set (old CSV format)
    for bu in &mut benunits {
        if !bu.would_claim_esa {
            bu.would_claim_esa = bu.person_ids.iter().any(|&pid| people[pid].esa_income > 0.0);
        }
        if !bu.would_claim_jsa {
            bu.would_claim_jsa = bu.person_ids.iter().any(|&pid| people[pid].jsa_income > 0.0);
        }
    }

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
        let state_pension = parse_f64(next());
        let savings_interest_income = parse_f64(next());
        let dividend_income = parse_f64(next());
        let property_income = parse_f64(next());
        let maintenance_income = parse_f64(next());
        let miscellaneous_income = parse_f64(next());
        let other_income = parse_f64(next());
        let is_in_scotland = parse_bool(next());
        let hours_worked = parse_f64(next());
        // Disability rate-band flags — default false for old CSVs
        let dla_care_low = parse_bool(next());
        let dla_care_mid = parse_bool(next());
        let dla_care_high = parse_bool(next());
        let dla_mob_low = parse_bool(next());
        let dla_mob_high = parse_bool(next());
        let pip_dl_std = parse_bool(next());
        let pip_dl_enh = parse_bool(next());
        let pip_mob_std = parse_bool(next());
        let pip_mob_enh = parse_bool(next());
        let aa_low = parse_bool(next());
        let aa_high = parse_bool(next());
        let is_disabled = parse_bool(next());
        let is_enhanced_disabled = parse_bool(next());
        let is_severely_disabled = parse_bool(next());
        let is_carer = parse_bool(next());
        let limitill = parse_bool(next());
        let esa_group = next().parse::<i64>().unwrap_or(0);
        let emp_status = next().parse::<i64>().unwrap_or(0);
        let looking_for_work = parse_bool(next());
        let is_self_identified_carer = parse_bool(next());
        let employee_pension_contributions = parse_f64(next());
        let personal_pension_contributions = parse_f64(next());
        let childcare_expenses = parse_f64(next());
        let child_benefit = parse_f64(next());
        let housing_benefit = parse_f64(next());
        let income_support = parse_f64(next());
        let pension_credit = parse_f64(next());
        let child_tax_credit = parse_f64(next());
        let working_tax_credit = parse_f64(next());
        let universal_credit = parse_f64(next());
        let dla_care = parse_f64(next());
        let dla_mobility = parse_f64(next());
        let pip_daily_living = parse_f64(next());
        let pip_mobility = parse_f64(next());
        let carers_allowance = parse_f64(next());
        let attendance_allowance = parse_f64(next());
        let esa_income = parse_f64(next());
        let esa_contributory = parse_f64(next());
        let jsa_income = parse_f64(next());
        let jsa_contributory = parse_f64(next());
        let other_benefits = parse_f64(next());
        let adp_daily_living = parse_f64(next());
        let adp_mobility = parse_f64(next());
        let cdp_care = parse_f64(next());
        let cdp_mobility = parse_f64(next());
        let would_claim_marriage_allowance = parse_bool(next());

        people.push(Person {
            id: person_id, benunit_id, household_id,
            age, gender, is_benunit_head, is_household_head,
            employment_income, self_employment_income,
            pension_income, state_pension,
            savings_interest_income, dividend_income,
            property_income, maintenance_income,
            miscellaneous_income, other_income,
            is_in_scotland, hours_worked,
            dla_care_low, dla_care_mid, dla_care_high,
            dla_mob_low, dla_mob_high,
            pip_dl_std, pip_dl_enh,
            pip_mob_std, pip_mob_enh,
            aa_low, aa_high,
            is_disabled, is_enhanced_disabled, is_severely_disabled, is_carer,
            limitill, esa_group, emp_status,
            looking_for_work, is_self_identified_carer,
            employee_pension_contributions, personal_pension_contributions,
            childcare_expenses,
            child_benefit, housing_benefit,
            income_support, pension_credit,
            child_tax_credit, working_tax_credit,
            universal_credit,
            dla_care, dla_mobility,
            pip_daily_living, pip_mobility,
            carers_allowance, attendance_allowance,
            esa_income, esa_contributory,
            jsa_income, jsa_contributory,
            other_benefits,
            adp_daily_living, adp_mobility,
            cdp_care, cdp_mobility,
            would_claim_marriage_allowance,
        });
    }

    Ok(people)
}

fn load_benunits_csv(data_dir: &Path) -> anyhow::Result<Vec<BenUnit>> {
    let path = data_dir.join("benunits.csv");
    let mut rdr = csv::Reader::from_path(&path)?;

    // Build header index for forward-compatible reading
    let headers = rdr.headers()?.clone();
    let idx = |name: &str| -> Option<usize> {
        headers.iter().position(|h| h == name)
    };
    let get_str = |r: &csv::StringRecord, name: &str| -> String {
        idx(name).and_then(|i| r.get(i)).unwrap_or("").to_string()
    };
    let get_bool = |r: &csv::StringRecord, name: &str| -> bool {
        idx(name).map(|i| parse_bool(r.get(i).unwrap_or(""))).unwrap_or(false)
    };
    let get_f64 = |r: &csv::StringRecord, name: &str| -> f64 {
        idx(name).map(|i| parse_f64(r.get(i).unwrap_or(""))).unwrap_or(0.0)
    };

    // Detect old format (reported_cb) vs new format (would_claim_cb)
    let old_format = idx("reported_cb").is_some();

    let mut benunits = Vec::new();
    for result in rdr.records() {
        let r = result?;

        let seed = if old_format { get_f64(&r, "take_up_seed") } else { get_f64(&r, "migration_seed") };

        let (wc_uc, wc_cb, wc_hb, wc_pc, wc_ctc, wc_wtc, wc_is, wc_esa, wc_jsa);
        if old_format {
            // Old format: reported_X flags → would_claim_X
            wc_uc  = get_bool(&r, "reported_uc");
            wc_cb  = get_bool(&r, "reported_cb");
            wc_hb  = get_bool(&r, "reported_hb");
            wc_pc  = get_bool(&r, "reported_pc");
            wc_ctc = get_bool(&r, "reported_ctc");
            wc_wtc = get_bool(&r, "reported_wtc");
            wc_is  = get_bool(&r, "reported_is");
            // Old format didn't have explicit ESA/JSA reported flags on benunit;
            // these will be derived from person data after loading.
            wc_esa = false;
            wc_jsa = false;
        } else {
            wc_uc  = get_bool(&r, "would_claim_uc");
            wc_cb  = get_bool(&r, "would_claim_cb");
            wc_hb  = get_bool(&r, "would_claim_hb");
            wc_pc  = get_bool(&r, "would_claim_pc");
            wc_ctc = get_bool(&r, "would_claim_ctc");
            wc_wtc = get_bool(&r, "would_claim_wtc");
            wc_is  = get_bool(&r, "would_claim_is");
            wc_esa = get_bool(&r, "would_claim_esa");
            wc_jsa = get_bool(&r, "would_claim_jsa");
        }

        benunits.push(BenUnit {
            id: get_str(&r, "benunit_id").parse().unwrap_or(0),
            household_id: get_str(&r, "household_id").parse().unwrap_or(0),
            person_ids: parse_id_list(&get_str(&r, "person_ids")),
            migration_seed: seed,
            on_uc: get_bool(&r, "on_uc"),
            on_legacy: get_bool(&r, "on_legacy"),
            rent_monthly: get_f64(&r, "rent_monthly"),
            is_lone_parent: get_bool(&r, "is_lone_parent"),
            would_claim_uc: wc_uc, would_claim_cb: wc_cb,
            would_claim_hb: wc_hb, would_claim_pc: wc_pc,
            would_claim_ctc: wc_ctc, would_claim_wtc: wc_wtc,
            would_claim_is: wc_is, would_claim_esa: wc_esa,
            would_claim_jsa: wc_jsa,
            ..BenUnit::default()
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
