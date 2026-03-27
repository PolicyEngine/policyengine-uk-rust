use std::collections::HashMap;
use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;

/// Parse real FRS microdata from UKDS CSV files.
///
/// Expected directory structure:
///   data_dir/adult.csv
///   data_dir/child.csv
///   data_dir/househol.csv
///   data_dir/benunit.csv
///
/// FRS income variables are WEEKLY — we annualise by multiplying by 52.
pub fn load_frs(data_dir: &Path) -> anyhow::Result<Dataset> {
    let adult_path = data_dir.join("adult.csv");
    let child_path = data_dir.join("child.csv");
    let hh_path = data_dir.join("househol.csv");
    let bu_path = data_dir.join("benunit.csv");

    // Phase 1: Load household data (weights, regions)
    let hh_data = load_household_data(&hh_path)?;

    // Phase 2: Load benefit unit data (UC claims, rent)
    let bu_data = load_benunit_data(&bu_path)?;

    // Phase 3: Load adults
    let adult_records = load_adult_data(&adult_path)?;

    // Phase 4: Load children
    let child_records = load_child_data(&child_path)?;

    // Phase 5: Assemble into entity hierarchy
    assemble_dataset(hh_data, bu_data, adult_records, child_records)
}

struct HouseholdRecord {
    sernum: i64,
    weight: f64,
    region: Region,
    rent_weekly: f64,
}

struct BenUnitRecord {
    sernum: i64,
    benunit: i64,
    claims_uc: bool,
    rent_weekly: f64,
}

#[allow(dead_code)]
struct PersonRecord {
    sernum: i64,
    benunit: i64,
    person: i64,
    age: f64,
    employment_income_weekly: f64,
    self_employment_income_weekly: f64,
    pension_income_weekly: f64,
    dividend_income_weekly: f64,
    is_child: bool,
}

fn parse_f64(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

fn parse_i64(s: &str) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

fn gss_to_region(code: &str) -> Region {
    match code.trim() {
        "112000001" => Region::NorthEast,
        "112000002" => Region::NorthWest,
        "112000003" => Region::Yorkshire,
        "112000004" => Region::EastMidlands,
        "112000005" => Region::WestMidlands,
        "112000006" => Region::EastOfEngland,
        "112000007" => Region::London,
        "112000008" => Region::SouthEast,
        "112000009" => Region::SouthWest,
        "299999999" => Region::Wales,
        "399999999" => Region::Scotland,
        "499999999" => Region::NorthernIreland,
        _ => Region::London,
    }
}

fn load_household_data(path: &Path) -> anyhow::Result<Vec<HouseholdRecord>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut records = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: HashMap<&str, &str> = headers.iter().zip(record.iter()).collect();

        records.push(HouseholdRecord {
            sernum: parse_i64(row.get("SERNUM").unwrap_or(&"0")),
            weight: parse_f64(row.get("gross4").unwrap_or(&"0")),
            region: gss_to_region(row.get("GVTREGN").unwrap_or(&"")),
            rent_weekly: parse_f64(row.get("hhrent").unwrap_or(&"0")),
        });
    }
    Ok(records)
}

fn load_benunit_data(path: &Path) -> anyhow::Result<Vec<BenUnitRecord>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut records = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: HashMap<&str, &str> = headers.iter().zip(record.iter()).collect();

        let buuc = parse_i64(row.get("BUUC").unwrap_or(&"0"));

        records.push(BenUnitRecord {
            sernum: parse_i64(row.get("SERNUM").unwrap_or(&"0")),
            benunit: parse_i64(row.get("BENUNIT").unwrap_or(&"0")),
            claims_uc: buuc == 1,
            rent_weekly: parse_f64(row.get("BURENT").unwrap_or(&"0")),
        });
    }
    Ok(records)
}

fn load_adult_data(path: &Path) -> anyhow::Result<Vec<PersonRecord>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut records = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: HashMap<&str, &str> = headers.iter().zip(record.iter()).collect();

        records.push(PersonRecord {
            sernum: parse_i64(row.get("SERNUM").unwrap_or(&"0")),
            benunit: parse_i64(row.get("BENUNIT").unwrap_or(&"0")),
            person: parse_i64(row.get("PERSON").unwrap_or(&"0")),
            age: parse_f64(row.get("age80").unwrap_or(&"30")),
            employment_income_weekly: parse_f64(row.get("inearns").unwrap_or(&"0")),
            self_employment_income_weekly: parse_f64(row.get("seincam2").unwrap_or(&"0")),
            pension_income_weekly: parse_f64(row.get("inpeninc").unwrap_or(&"0")),
            dividend_income_weekly: parse_f64(row.get("DIVIDGRO").unwrap_or(&"0")),
            is_child: false,
        });
    }
    Ok(records)
}

fn load_child_data(path: &Path) -> anyhow::Result<Vec<PersonRecord>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut records = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: HashMap<&str, &str> = headers.iter().zip(record.iter()).collect();

        records.push(PersonRecord {
            sernum: parse_i64(row.get("SERNUM").unwrap_or(&"0")),
            benunit: parse_i64(row.get("BENUNIT").unwrap_or(&"0")),
            person: parse_i64(row.get("PERSON").unwrap_or(&"0")),
            age: parse_f64(row.get("AGE").unwrap_or(&"8")),
            employment_income_weekly: 0.0,
            self_employment_income_weekly: 0.0,
            pension_income_weekly: 0.0,
            dividend_income_weekly: 0.0,
            is_child: true,
        });
    }
    Ok(records)
}

fn assemble_dataset(
    hh_data: Vec<HouseholdRecord>,
    bu_data: Vec<BenUnitRecord>,
    adult_records: Vec<PersonRecord>,
    child_records: Vec<PersonRecord>,
) -> anyhow::Result<Dataset> {
    // Build lookup: sernum -> household index
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
            rent: hh.rent_weekly * 52.0,
            council_tax: 1800.0, // Not in EUL FRS, use average
        });
    }

    // Build benefit units
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
                rent_monthly: bu.rent_weekly * 52.0 / 12.0,
            });
            households[hh_idx].benunit_ids.push(bu_idx);
        }
    }

    // Build people (adults + children)
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
                    employment_income: pr.employment_income_weekly * 52.0,
                    self_employment_income: (pr.self_employment_income_weekly * 52.0).max(0.0),
                    pension_income: pr.pension_income_weekly * 52.0,
                    savings_interest_income: 0.0, // TOTINT is often 'A' in EUL
                    dividend_income: pr.dividend_income_weekly * 52.0,
                    property_income: 0.0, // RENTPROF often 'A' in EUL
                    other_income: 0.0,
                    is_in_scotland: is_scotland,
                    hours_worked: 0.0,
                    is_disabled: false,
                    is_carer: false,
                });

                benunits[bu_idx].person_ids.push(pid);
                households[hh_idx].person_ids.push(pid);
            }
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
