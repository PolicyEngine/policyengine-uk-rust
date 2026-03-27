use std::collections::HashMap;
use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;

const WEEKS_IN_YEAR: f64 = 365.25 / 7.0;

/// Parse real FRS microdata from UKDS tab-delimited files.
///
/// Expected directory structure (tab-delimited, as distributed by UKDS):
///   data_dir/adult.tab
///   data_dir/child.tab
///   data_dir/househol.tab
///   data_dir/benunit.tab
///   data_dir/accounts.tab
///   data_dir/benefits.tab
///   data_dir/job.tab
///   data_dir/pension.tab
///   data_dir/penprov.tab
///
/// Also supports .csv extension as fallback.
///
/// FRS income variables are WEEKLY — we annualise by multiplying by WEEKS_IN_YEAR.
pub fn load_frs(data_dir: &Path) -> anyhow::Result<Dataset> {
    // Load all required tables
    let household_table = load_table(data_dir, "househol")?;
    let benunit_table = load_table(data_dir, "benunit")?;
    let adult_table = load_table(data_dir, "adult")?;
    let child_table = load_table(data_dir, "child")?;

    // Optional tables — gracefully handle missing
    let accounts_table = load_table(data_dir, "accounts").ok();
    let benefits_table = load_table(data_dir, "benefits").ok();
    let job_table = load_table(data_dir, "job").ok();
    let pension_table = load_table(data_dir, "pension").ok();
    let penprov_table = load_table(data_dir, "penprov").ok();

    // Phase 1: Build household records
    let hh_data = parse_households(&household_table);

    // Phase 2: Build benefit unit records
    let bu_data = parse_benunits(&benunit_table);

    // Phase 3: Build person-level aggregates from sub-tables
    let account_agg = accounts_table.as_ref().map(|t| aggregate_accounts(t));
    let benefit_agg = benefits_table.as_ref().map(|t| aggregate_benefits(t));
    let job_agg = job_table.as_ref().map(|t| aggregate_jobs(t));
    let pension_agg = pension_table.as_ref().map(|t| aggregate_pensions(t));
    let penprov_agg = penprov_table.as_ref().map(|t| aggregate_penprov(t));

    // Phase 4: Build adult records
    let adult_records = parse_adults(&adult_table, &account_agg, &benefit_agg, &job_agg, &pension_agg, &penprov_agg);

    // Phase 5: Build child records
    let child_records = parse_children(&child_table);

    // Phase 6: Assemble into entity hierarchy
    assemble_dataset(hh_data, bu_data, adult_records, child_records)
}

// ── Table loading ────────────────────────────────────────────────────────

type Table = Vec<HashMap<String, String>>;

fn load_table(data_dir: &Path, name: &str) -> anyhow::Result<Table> {
    // Try .tab first (UKDS format), then .csv
    let tab_path = data_dir.join(format!("{}.tab", name));
    let csv_path = data_dir.join(format!("{}.csv", name));

    let (path, delimiter) = if tab_path.exists() {
        (tab_path, b'\t')
    } else if csv_path.exists() {
        (csv_path, b',')
    } else {
        anyhow::bail!("Neither {}.tab nor {}.csv found in {:?}", name, name, data_dir);
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_path(&path)?;

    let headers: Vec<String> = rdr.headers()?.iter().map(|h| h.to_lowercase()).collect();
    let mut table = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: HashMap<String, String> = headers.iter()
            .zip(record.iter())
            .map(|(h, v)| (h.clone(), v.to_string()))
            .collect();
        table.push(row);
    }

    Ok(table)
}

fn get_f64(row: &HashMap<String, String>, key: &str) -> f64 {
    row.get(key)
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn get_i64(row: &HashMap<String, String>, key: &str) -> i64 {
    row.get(key)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

fn get_positive_f64(row: &HashMap<String, String>, key: &str) -> f64 {
    get_f64(row, key).max(0.0)
}

// ── Household parsing ────────────────────────────────────────────────────

struct HouseholdRecord {
    sernum: i64,
    weight: f64,
    region: Region,
    rent_weekly: f64,
    council_tax_annual: f64,
}

fn region_from_gvtregno(code: i64) -> Region {
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
        13 => Region::NorthernIreland,
        _ => Region::London,
    }
}

fn parse_households(table: &Table) -> Vec<HouseholdRecord> {
    table.iter().map(|row| {
        let ct = get_f64(row, "ctannual");
        HouseholdRecord {
            sernum: get_i64(row, "sernum"),
            weight: get_f64(row, "gross4"),
            region: region_from_gvtregno(get_i64(row, "gvtregno")),
            rent_weekly: get_positive_f64(row, "hhrent"),
            council_tax_annual: if ct > 0.0 { ct } else { 1800.0 },
        }
    }).collect()
}

// ── Benefit unit parsing ─────────────────────────────────────────────────

struct BenUnitRecord {
    sernum: i64,
    benunit: i64,
    claims_uc: bool,
    rent_weekly: f64,
}

fn parse_benunits(table: &Table) -> Vec<BenUnitRecord> {
    table.iter().map(|row| {
        BenUnitRecord {
            sernum: get_i64(row, "sernum"),
            benunit: get_i64(row, "benunit"),
            claims_uc: get_positive_f64(row, "buuc") > 0.0,
            rent_weekly: get_positive_f64(row, "burent"),
        }
    }).collect()
}

// ── Person-level sub-table aggregation ───────────────────────────────────

type PersonKey = (i64, i64); // (sernum * 1000 + person)

fn person_key(sernum: i64, person: i64) -> PersonKey {
    (sernum, person)
}

#[derive(Default)]
struct AccountAgg {
    savings_interest_weekly: f64,
    dividend_income_weekly: f64,
}

fn aggregate_accounts(table: &Table) -> HashMap<PersonKey, AccountAgg> {
    let mut map: HashMap<PersonKey, AccountAgg> = HashMap::new();
    for row in table {
        let sernum = get_i64(row, "sernum");
        let person = get_i64(row, "person");
        let accint = get_f64(row, "accint");
        let account_type = get_i64(row, "account");
        let acctax = get_i64(row, "acctax");
        let invtax = get_i64(row, "invtax");

        let entry = map.entry(person_key(sernum, person)).or_default();

        // Savings accounts: types 1, 3, 5, 27, 28
        if [1, 3, 5, 27, 28].contains(&account_type) {
            let gross = if acctax == 1 { accint * 1.25 } else { accint };
            entry.savings_interest_weekly += gross.max(0.0);
        }

        // Dividend-bearing: type 6 (GGES), 7, 8 (stocks/shares/UITs)
        if account_type == 6 || account_type == 7 || account_type == 8 {
            let gross = if invtax == 1 { accint * 1.25 } else { accint };
            entry.dividend_income_weekly += gross.max(0.0);
        }
    }
    map
}

/// Benefit codes from FRS benefits table
#[derive(Default)]
struct BenefitAgg {
    state_pension: f64,
    child_benefit: f64,
    income_support: f64,
    housing_benefit: f64,
    attendance_allowance: f64,
    dla_sc: f64,
    dla_m: f64,
    carers_allowance: f64,
    pension_credit: f64,
    child_tax_credit: f64,
    working_tax_credit: f64,
    universal_credit: f64,
    pip_m: f64,
    pip_dl: f64,
    esa_income: f64,
    esa_contrib: f64,
    jsa_income: f64,
    jsa_contrib: f64,
}

fn aggregate_benefits(table: &Table) -> HashMap<PersonKey, BenefitAgg> {
    let mut map: HashMap<PersonKey, BenefitAgg> = HashMap::new();
    for row in table {
        let sernum = get_i64(row, "sernum");
        let person = get_i64(row, "person");
        let benefit = get_i64(row, "benefit");
        let benamt = get_positive_f64(row, "benamt");
        let var2 = get_i64(row, "var2");

        let entry = map.entry(person_key(sernum, person)).or_default();
        match benefit {
            5 => entry.state_pension += benamt,
            3 => entry.child_benefit += benamt,
            19 => entry.income_support += benamt,
            94 => entry.housing_benefit += benamt,
            12 => entry.attendance_allowance += benamt,
            1 => entry.dla_sc += benamt,
            2 => entry.dla_m += benamt,
            13 => entry.carers_allowance += benamt,
            4 => entry.pension_credit += benamt,
            91 => entry.child_tax_credit += benamt,
            90 => entry.working_tax_credit += benamt,
            95 => entry.universal_credit += benamt,
            97 => entry.pip_m += benamt,
            96 => entry.pip_dl += benamt,
            14 => {
                // JSA: var2 1,3 = contrib; 2,4 = income-based
                if var2 == 1 || var2 == 3 { entry.jsa_contrib += benamt; }
                if var2 == 2 || var2 == 4 { entry.jsa_income += benamt; }
            }
            16 => {
                // ESA: var2 1,3 = contrib; 2,4 = income-related
                if var2 == 1 || var2 == 3 { entry.esa_contrib += benamt; }
                if var2 == 2 || var2 == 4 { entry.esa_income += benamt; }
            }
            _ => {}
        }
    }
    map
}

#[derive(Default)]
struct JobAgg {
    employee_pension_contributions_weekly: f64,
    #[allow(dead_code)]
    hours_worked_weekly: f64,
}

fn aggregate_jobs(table: &Table) -> HashMap<PersonKey, JobAgg> {
    let mut map: HashMap<PersonKey, JobAgg> = HashMap::new();
    for row in table {
        let sernum = get_i64(row, "sernum");
        let person = get_i64(row, "person");
        let deduc1 = get_positive_f64(row, "deduc1");

        let entry = map.entry(person_key(sernum, person)).or_default();
        entry.employee_pension_contributions_weekly += deduc1;
    }
    map
}

#[derive(Default)]
struct PensionAgg {
    private_pension_weekly: f64,
}

fn aggregate_pensions(table: &Table) -> HashMap<PersonKey, PensionAgg> {
    let mut map: HashMap<PersonKey, PensionAgg> = HashMap::new();
    for row in table {
        let sernum = get_i64(row, "sernum");
        let person = get_i64(row, "person");
        let penpay = get_positive_f64(row, "penpay");
        let ptamt = get_f64(row, "ptamt");
        let ptinc = get_i64(row, "ptinc");
        let poamt = get_f64(row, "poamt");
        let poinc = get_i64(row, "poinc");
        let penoth = get_i64(row, "penoth");

        let entry = map.entry(person_key(sernum, person)).or_default();
        entry.private_pension_weekly += penpay;
        if ptinc == 2 && ptamt > 0.0 { entry.private_pension_weekly += ptamt; }
        if (poinc == 2 || penoth == 1) && poamt > 0.0 { entry.private_pension_weekly += poamt; }
    }
    map
}

#[derive(Default)]
struct PenprovAgg {
    personal_pension_contributions_weekly: f64,
}

fn aggregate_penprov(table: &Table) -> HashMap<PersonKey, PenprovAgg> {
    let mut map: HashMap<PersonKey, PenprovAgg> = HashMap::new();
    for row in table {
        let sernum = get_i64(row, "sernum");
        let person = get_i64(row, "person");
        let stemppen = get_i64(row, "stemppen");
        let penamt = get_positive_f64(row, "penamt");

        // Personal pension: stemppen 5 or 6
        if stemppen == 5 || stemppen == 6 {
            let entry = map.entry(person_key(sernum, person)).or_default();
            entry.personal_pension_contributions_weekly += penamt;
        }
    }
    map
}

// ── Person record parsing ────────────────────────────────────────────────

struct PersonRecord {
    sernum: i64,
    benunit: i64,
    person: i64,
    age: f64,
    gender: Gender,
    is_benunit_head: bool,
    is_household_head: bool,
    employment_income_weekly: f64,
    self_employment_income_weekly: f64,
    private_pension_income_weekly: f64,
    state_pension_weekly: f64,
    savings_interest_weekly: f64,
    dividend_income_weekly: f64,
    property_income_weekly: f64,
    maintenance_income_weekly: f64,
    miscellaneous_income_weekly: f64,
    hours_worked_weekly: f64,
    is_disabled: bool,
    is_enhanced_disabled: bool,
    is_severely_disabled: bool,
    is_carer: bool,
    employee_pension_contributions_weekly: f64,
    personal_pension_contributions_weekly: f64,
    childcare_expenses_weekly: f64,
    // Reported benefits (weekly)
    child_benefit_reported_weekly: f64,
    housing_benefit_reported_weekly: f64,
    income_support_reported_weekly: f64,
    pension_credit_reported_weekly: f64,
    child_tax_credit_reported_weekly: f64,
    working_tax_credit_reported_weekly: f64,
    universal_credit_reported_weekly: f64,
    dla_sc_reported_weekly: f64,
    dla_m_reported_weekly: f64,
    pip_dl_reported_weekly: f64,
    pip_m_reported_weekly: f64,
    carers_allowance_reported_weekly: f64,
    attendance_allowance_reported_weekly: f64,
    esa_income_reported_weekly: f64,
    esa_contrib_reported_weekly: f64,
    jsa_income_reported_weekly: f64,
    jsa_contrib_reported_weekly: f64,
    is_child: bool,
}

fn parse_adults(
    table: &Table,
    account_agg: &Option<HashMap<PersonKey, AccountAgg>>,
    benefit_agg: &Option<HashMap<PersonKey, BenefitAgg>>,
    job_agg: &Option<HashMap<PersonKey, JobAgg>>,
    pension_agg: &Option<HashMap<PersonKey, PensionAgg>>,
    penprov_agg: &Option<HashMap<PersonKey, PenprovAgg>>,
) -> Vec<PersonRecord> {
    table.iter().map(|row| {
        let sernum = get_i64(row, "sernum");
        let person_id = get_i64(row, "person");
        let key = person_key(sernum, person_id);

        let acct = account_agg.as_ref().and_then(|m| m.get(&key));
        let bens = benefit_agg.as_ref().and_then(|m| m.get(&key));
        let jobs = job_agg.as_ref().and_then(|m| m.get(&key));
        let pens = pension_agg.as_ref().and_then(|m| m.get(&key));
        let pp = penprov_agg.as_ref().and_then(|m| m.get(&key));

        let sex = get_i64(row, "sex");
        let hours = get_f64(row, "tothours").max(0.0);

        // Disability: DLA/PIP receipt indicates disability
        let dla_sc = bens.map_or(0.0, |b| b.dla_sc);
        let dla_m = bens.map_or(0.0, |b| b.dla_m);
        let pip_dl = bens.map_or(0.0, |b| b.pip_dl);
        let pip_m = bens.map_or(0.0, |b| b.pip_m);
        let is_disabled = (dla_sc + dla_m + pip_m + pip_dl) > 0.0;

        // Property income: cvpay + royyr1
        let property = get_positive_f64(row, "cvpay") + get_positive_f64(row, "royyr1");

        // Maintenance income
        let mntus1 = get_i64(row, "mntus1");
        let maint = if mntus1 == 2 {
            get_positive_f64(row, "mntusam1")
        } else {
            get_positive_f64(row, "mntamt1")
        };
        let maint2 = get_positive_f64(row, "mntamt2");

        PersonRecord {
            sernum,
            benunit: get_i64(row, "benunit"),
            person: person_id,
            age: get_f64(row, "age80"),
            gender: if sex == 1 { Gender::Male } else { Gender::Female },
            is_benunit_head: get_i64(row, "uperson") == 1,
            is_household_head: get_i64(row, "hrpid") == 1,
            employment_income_weekly: get_positive_f64(row, "inearns"),
            self_employment_income_weekly: get_positive_f64(row, "seincam2"),
            private_pension_income_weekly: pens.map_or(
                get_positive_f64(row, "inpeninc"),
                |p| p.private_pension_weekly,
            ),
            state_pension_weekly: bens.map_or(0.0, |b| b.state_pension),
            savings_interest_weekly: acct.map_or(0.0, |a| a.savings_interest_weekly),
            dividend_income_weekly: acct.map_or(
                get_positive_f64(row, "dividgro"),
                |a| a.dividend_income_weekly,
            ),
            property_income_weekly: property,
            maintenance_income_weekly: maint + maint2,
            miscellaneous_income_weekly: 0.0,
            hours_worked_weekly: hours,
            is_disabled,
            is_enhanced_disabled: dla_sc > 100.0, // rough proxy for higher rate
            is_severely_disabled: pip_dl > 100.0,  // rough proxy for enhanced rate
            is_carer: bens.map_or(false, |b| b.carers_allowance > 0.0),
            employee_pension_contributions_weekly: jobs.map_or(0.0, |j| j.employee_pension_contributions_weekly),
            personal_pension_contributions_weekly: pp.map_or(0.0, |p| p.personal_pension_contributions_weekly),
            childcare_expenses_weekly: 0.0, // Would need chldcare table
            child_benefit_reported_weekly: bens.map_or(0.0, |b| b.child_benefit),
            housing_benefit_reported_weekly: bens.map_or(0.0, |b| b.housing_benefit),
            income_support_reported_weekly: bens.map_or(0.0, |b| b.income_support),
            pension_credit_reported_weekly: bens.map_or(0.0, |b| b.pension_credit),
            child_tax_credit_reported_weekly: bens.map_or(0.0, |b| b.child_tax_credit),
            working_tax_credit_reported_weekly: bens.map_or(0.0, |b| b.working_tax_credit),
            universal_credit_reported_weekly: bens.map_or(0.0, |b| b.universal_credit),
            dla_sc_reported_weekly: dla_sc,
            dla_m_reported_weekly: dla_m,
            pip_dl_reported_weekly: pip_dl,
            pip_m_reported_weekly: pip_m,
            carers_allowance_reported_weekly: bens.map_or(0.0, |b| b.carers_allowance),
            attendance_allowance_reported_weekly: bens.map_or(0.0, |b| b.attendance_allowance),
            esa_income_reported_weekly: bens.map_or(0.0, |b| b.esa_income),
            esa_contrib_reported_weekly: bens.map_or(0.0, |b| b.esa_contrib),
            jsa_income_reported_weekly: bens.map_or(0.0, |b| b.jsa_income),
            jsa_contrib_reported_weekly: bens.map_or(0.0, |b| b.jsa_contrib),
            is_child: false,
        }
    }).collect()
}

fn parse_children(table: &Table) -> Vec<PersonRecord> {
    table.iter().map(|row| {
        let sernum = get_i64(row, "sernum");
        let person_id = get_i64(row, "person");
        let sex = get_i64(row, "sex");
        PersonRecord {
            sernum,
            benunit: get_i64(row, "benunit"),
            person: person_id,
            age: get_f64(row, "age"),
            gender: if sex == 1 { Gender::Male } else { Gender::Female },
            is_benunit_head: false,
            is_household_head: false,
            employment_income_weekly: 0.0,
            self_employment_income_weekly: 0.0,
            private_pension_income_weekly: 0.0,
            state_pension_weekly: 0.0,
            savings_interest_weekly: 0.0,
            dividend_income_weekly: 0.0,
            property_income_weekly: 0.0,
            maintenance_income_weekly: 0.0,
            miscellaneous_income_weekly: 0.0,
            hours_worked_weekly: 0.0,
            is_disabled: false,
            is_enhanced_disabled: false,
            is_severely_disabled: false,
            is_carer: false,
            employee_pension_contributions_weekly: 0.0,
            personal_pension_contributions_weekly: 0.0,
            childcare_expenses_weekly: 0.0,
            child_benefit_reported_weekly: 0.0,
            housing_benefit_reported_weekly: 0.0,
            income_support_reported_weekly: 0.0,
            pension_credit_reported_weekly: 0.0,
            child_tax_credit_reported_weekly: 0.0,
            working_tax_credit_reported_weekly: 0.0,
            universal_credit_reported_weekly: 0.0,
            dla_sc_reported_weekly: 0.0,
            dla_m_reported_weekly: 0.0,
            pip_dl_reported_weekly: 0.0,
            pip_m_reported_weekly: 0.0,
            carers_allowance_reported_weekly: 0.0,
            attendance_allowance_reported_weekly: 0.0,
            esa_income_reported_weekly: 0.0,
            esa_contrib_reported_weekly: 0.0,
            jsa_income_reported_weekly: 0.0,
            jsa_contrib_reported_weekly: 0.0,
            is_child: true,
        }
    }).collect()
}

// ── Dataset assembly ─────────────────────────────────────────────────────

fn assemble_dataset(
    hh_data: Vec<HouseholdRecord>,
    bu_data: Vec<BenUnitRecord>,
    adult_records: Vec<PersonRecord>,
    child_records: Vec<PersonRecord>,
) -> anyhow::Result<Dataset> {
    let mut hh_map: HashMap<i64, usize> = HashMap::new();
    let mut households: Vec<Household> = Vec::new();

    for (idx, hh) in hh_data.iter().enumerate() {
        hh_map.insert(hh.sernum, idx);
        households.push(Household {
            id: idx,
            benunit_ids: Vec::new(),
            person_ids: Vec::new(),
            weight: hh.weight,
            region: hh.region,
            rent: hh.rent_weekly * WEEKS_IN_YEAR,
            council_tax: hh.council_tax_annual,
        });
    }

    let mut bu_map: HashMap<(i64, i64), usize> = HashMap::new();
    let mut benunits: Vec<BenUnit> = Vec::new();

    for bu in &bu_data {
        if let Some(&hh_idx) = hh_map.get(&bu.sernum) {
            let bu_idx = benunits.len();
            bu_map.insert((bu.sernum, bu.benunit), bu_idx);
            benunits.push(BenUnit {
                id: bu_idx,
                household_id: hh_idx,
                person_ids: Vec::new(),
                would_claim_uc: bu.claims_uc,
                would_claim_child_benefit: true,  // Derived below from person data
                would_claim_pc: false,             // Derived below from person data
                would_claim_hb: false,             // Derived below from person data
                would_claim_ctc: false,            // Derived below from person data
                would_claim_wtc: false,            // Derived below from person data
                would_claim_is: false,             // Derived below from person data
                rent_monthly: bu.rent_weekly * WEEKS_IN_YEAR / 12.0,
                is_lone_parent: false,             // Set after people are assigned
            });
            households[hh_idx].benunit_ids.push(bu_idx);
        }
    }

    let mut people: Vec<Person> = Vec::new();

    let all_persons: Vec<&PersonRecord> = adult_records.iter()
        .chain(child_records.iter())
        .collect();

    for pr in all_persons {
        if let Some(&hh_idx) = hh_map.get(&pr.sernum) {
            let bu_key = (pr.sernum, pr.benunit);
            if let Some(&bu_idx) = bu_map.get(&bu_key) {
                let pid = people.len();
                let is_scotland = households[hh_idx].region.is_scotland();

                people.push(Person {
                    id: pid,
                    benunit_id: bu_idx,
                    household_id: hh_idx,
                    age: pr.age,
                    gender: pr.gender,
                    is_benunit_head: pr.is_benunit_head,
                    is_household_head: pr.is_household_head,
                    employment_income: pr.employment_income_weekly * WEEKS_IN_YEAR,
                    self_employment_income: (pr.self_employment_income_weekly * WEEKS_IN_YEAR).max(0.0),
                    pension_income: pr.private_pension_income_weekly * WEEKS_IN_YEAR,
                    state_pension_reported: pr.state_pension_weekly * WEEKS_IN_YEAR,
                    savings_interest_income: pr.savings_interest_weekly * WEEKS_IN_YEAR,
                    dividend_income: pr.dividend_income_weekly * WEEKS_IN_YEAR,
                    property_income: pr.property_income_weekly * WEEKS_IN_YEAR,
                    maintenance_income: pr.maintenance_income_weekly * WEEKS_IN_YEAR,
                    miscellaneous_income: pr.miscellaneous_income_weekly * WEEKS_IN_YEAR,
                    other_income: 0.0,
                    is_in_scotland: is_scotland,
                    hours_worked: pr.hours_worked_weekly * 52.0,
                    is_disabled: pr.is_disabled,
                    is_enhanced_disabled: pr.is_enhanced_disabled,
                    is_severely_disabled: pr.is_severely_disabled,
                    is_carer: pr.is_carer,
                    employee_pension_contributions: pr.employee_pension_contributions_weekly * WEEKS_IN_YEAR,
                    personal_pension_contributions: pr.personal_pension_contributions_weekly * WEEKS_IN_YEAR,
                    childcare_expenses: pr.childcare_expenses_weekly * WEEKS_IN_YEAR,
                    child_benefit_reported: pr.child_benefit_reported_weekly * WEEKS_IN_YEAR,
                    housing_benefit_reported: pr.housing_benefit_reported_weekly * WEEKS_IN_YEAR,
                    income_support_reported: pr.income_support_reported_weekly * WEEKS_IN_YEAR,
                    pension_credit_reported: pr.pension_credit_reported_weekly * WEEKS_IN_YEAR,
                    child_tax_credit_reported: pr.child_tax_credit_reported_weekly * WEEKS_IN_YEAR,
                    working_tax_credit_reported: pr.working_tax_credit_reported_weekly * WEEKS_IN_YEAR,
                    universal_credit_reported: pr.universal_credit_reported_weekly * WEEKS_IN_YEAR,
                    dla_sc_reported: pr.dla_sc_reported_weekly * WEEKS_IN_YEAR,
                    dla_m_reported: pr.dla_m_reported_weekly * WEEKS_IN_YEAR,
                    pip_dl_reported: pr.pip_dl_reported_weekly * WEEKS_IN_YEAR,
                    pip_m_reported: pr.pip_m_reported_weekly * WEEKS_IN_YEAR,
                    carers_allowance_reported: pr.carers_allowance_reported_weekly * WEEKS_IN_YEAR,
                    attendance_allowance_reported: pr.attendance_allowance_reported_weekly * WEEKS_IN_YEAR,
                    esa_income_reported: pr.esa_income_reported_weekly * WEEKS_IN_YEAR,
                    esa_contrib_reported: pr.esa_contrib_reported_weekly * WEEKS_IN_YEAR,
                    jsa_income_reported: pr.jsa_income_reported_weekly * WEEKS_IN_YEAR,
                    jsa_contrib_reported: pr.jsa_contrib_reported_weekly * WEEKS_IN_YEAR,
                    would_claim_marriage_allowance: false,
                });

                benunits[bu_idx].person_ids.push(pid);
                households[hh_idx].person_ids.push(pid);
            }
        }
    }

    // Derive take-up flags and lone parent status from person-level reported benefits
    for bu in &mut benunits {
        let num_adults = bu.person_ids.iter().filter(|&&pid| people[pid].is_adult()).count();
        let num_children = bu.person_ids.iter().filter(|&&pid| people[pid].is_child()).count();
        bu.is_lone_parent = num_adults == 1 && num_children > 0;

        // Set take-up flags based on whether any person in the benunit reports receiving the benefit
        for &pid in &bu.person_ids {
            let p = &people[pid];
            if p.housing_benefit_reported > 0.0 { bu.would_claim_hb = true; }
            if p.child_tax_credit_reported > 0.0 { bu.would_claim_ctc = true; }
            if p.working_tax_credit_reported > 0.0 { bu.would_claim_wtc = true; }
            if p.income_support_reported > 0.0 { bu.would_claim_is = true; }
            if p.pension_credit_reported > 0.0 { bu.would_claim_pc = true; }
            if p.child_benefit_reported > 0.0 { bu.would_claim_child_benefit = true; }
            if p.universal_credit_reported > 0.0 { bu.would_claim_uc = true; }
        }
    }

    Ok(Dataset {
        people,
        benunits,
        households,
        name: "Family Resources Survey 2023-24".to_string(),
        year: 2023,
    })
}
