use std::path::Path;
use crate::engine::entities::*;
use crate::engine::simulation::{Simulation, SimulationResults};
use crate::data::Dataset;
use crate::parameters::Parameters;

/// Write a Dataset to clean CSVs with descriptive column names.
///
/// Produces three files in `output_dir`:
///   - persons.csv: one row per person, annual values
///   - benunits.csv: one row per benefit unit (includes ENR flags)
///   - households.csv: one row per household
///
/// `params` is used to compute baseline entitlements so that ENR flags can be
/// baked into benunits.csv at extract time, avoiding a baseline re-run on every
/// simulation.
pub fn write_clean_csvs(dataset: &mut Dataset, params: &Parameters, output_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_dir)?;

    compute_enr_flags(dataset, params);

    write_persons(dataset, output_dir)?;
    write_benunits(dataset, output_dir)?;
    write_households(dataset, output_dir)?;

    Ok(())
}

/// Run a baseline simulation pass and mark each benunit as an ENR for each
/// benefit where the model says they are entitled but they did not report receipt.
fn compute_enr_flags(dataset: &mut Dataset, params: &Parameters) {
    let sim = Simulation::new(
        dataset.people.clone(),
        dataset.benunits.clone(),
        dataset.households.clone(),
        params.clone(),
    );
    let results = sim.run();

    for bu in &mut dataset.benunits {
        let br = &results.benunit_results[bu.id];
        // ENR = entitled (model says > 0) AND not reporting receipt
        bu.is_enr_uc  = br.universal_credit > 0.0  && !bu.reported_uc;
        bu.is_enr_hb  = br.housing_benefit  > 0.0  && !bu.reported_hb;
        bu.is_enr_pc  = br.pension_credit   > 0.0  && !bu.reported_pc;
        bu.is_enr_cb  = br.child_benefit    > 0.0  && !bu.reported_cb;
        bu.is_enr_ctc = br.child_tax_credit > 0.0  && !bu.reported_ctc;
        bu.is_enr_wtc = br.working_tax_credit > 0.0 && !bu.reported_wtc;
    }
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
            p.is_disabled.to_string(),
            p.is_enhanced_disabled.to_string(),
            p.is_severely_disabled.to_string(),
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
        "take_up_seed", "on_uc", "on_legacy",
        "rent_monthly", "is_lone_parent",
        // Reported receipt flags
        "reported_cb", "reported_uc", "reported_hb",
        "reported_pc", "reported_ctc", "reported_wtc", "reported_is",
        // ENR flags
        "is_enr_uc", "is_enr_hb", "is_enr_pc",
        "is_enr_cb", "is_enr_ctc", "is_enr_wtc",
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
            bu.reported_cb.to_string(),
            bu.reported_uc.to_string(),
            bu.reported_hb.to_string(),
            bu.reported_pc.to_string(),
            bu.reported_ctc.to_string(),
            bu.reported_wtc.to_string(),
            bu.reported_is.to_string(),
            bu.is_enr_uc.to_string(),
            bu.is_enr_hb.to_string(),
            bu.is_enr_pc.to_string(),
            bu.is_enr_cb.to_string(),
            bu.is_enr_ctc.to_string(),
            bu.is_enr_wtc.to_string(),
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
        let state_pension = parse_f64(next());
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
            dla_care_low: false, dla_care_mid: false, dla_care_high: false,
            dla_mob_low: false, dla_mob_high: false,
            pip_dl_std: false, pip_dl_enh: false,
            pip_mob_std: false, pip_mob_enh: false,
            aa_low: false, aa_high: false,
            is_disabled, is_enhanced_disabled, is_severely_disabled, is_carer,
            limitill: false, esa_group: 0, emp_status: 0,
            looking_for_work: false, is_self_identified_carer: false,
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
        let reported_cb  = parse_bool(next());
        let reported_uc  = parse_bool(next());
        let reported_hb  = parse_bool(next());
        let reported_pc  = parse_bool(next());
        let reported_ctc = parse_bool(next());
        let reported_wtc = parse_bool(next());
        let reported_is  = parse_bool(next());
        let is_enr_uc  = parse_bool(next());
        let is_enr_hb  = parse_bool(next());
        let is_enr_pc  = parse_bool(next());
        let is_enr_cb  = parse_bool(next());
        let is_enr_ctc = parse_bool(next());
        let is_enr_wtc = parse_bool(next());

        benunits.push(BenUnit {
            id, household_id, person_ids,
            take_up_seed, on_uc, on_legacy, rent_monthly, is_lone_parent,
            reported_cb, reported_uc, reported_hb, reported_pc,
            reported_ctc, reported_wtc, reported_is,
            is_enr_uc, is_enr_hb, is_enr_pc, is_enr_cb, is_enr_ctc, is_enr_wtc,
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
