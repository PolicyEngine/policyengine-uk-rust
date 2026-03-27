use crate::engine::entities::*;
use crate::data::Dataset;

/// Helper to create a Person with common defaults for synthetic data.
fn make_person(
    id: usize,
    benunit_id: usize,
    household_id: usize,
    age: f64,
    employment_income: f64,
    self_employment_income: f64,
    pension_income: f64,
    savings_interest_income: f64,
    dividend_income: f64,
    property_income: f64,
    is_scotland: bool,
    hours_worked: f64,
    is_disabled: bool,
    is_carer: bool,
) -> Person {
    Person {
        id,
        benunit_id,
        household_id,
        age,
        gender: if age < 18.0 { Gender::Male } else { Gender::Female }, // arbitrary for synthetic
        is_benunit_head: false, // set after creation
        is_household_head: false,
        employment_income,
        self_employment_income,
        pension_income,
        state_pension_reported: if age >= 66.0 && pension_income > 0.0 { 10600.0 } else { 0.0 },
        savings_interest_income,
        dividend_income,
        property_income,
        maintenance_income: 0.0,
        miscellaneous_income: 0.0,
        other_income: 0.0,
        is_in_scotland: is_scotland,
        hours_worked,
        is_disabled,
        is_enhanced_disabled: false,
        is_severely_disabled: false,
        is_carer,
        employee_pension_contributions: 0.0,
        personal_pension_contributions: 0.0,
        childcare_expenses: 0.0,
        child_benefit_reported: 0.0,
        housing_benefit_reported: 0.0,
        income_support_reported: 0.0,
        pension_credit_reported: 0.0,
        child_tax_credit_reported: 0.0,
        working_tax_credit_reported: 0.0,
        universal_credit_reported: 0.0,
        dla_sc_reported: 0.0,
        dla_m_reported: 0.0,
        pip_dl_reported: 0.0,
        pip_m_reported: 0.0,
        carers_allowance_reported: 0.0,
        attendance_allowance_reported: 0.0,
        esa_income_reported: 0.0,
        esa_contrib_reported: 0.0,
        jsa_income_reported: 0.0,
        jsa_contrib_reported: 0.0,
        would_claim_marriage_allowance: false,
    }
}

/// Generate a synthetic FRS-like dataset for microsimulation.
/// This creates a representative sample of ~20,000 households with realistic
/// income distributions based on published ONS/DWP statistics.
///
/// In production, this would be replaced by actual FRS microdata from UKDS.
pub fn generate_synthetic_frs(year: u32) -> Dataset {
    let mut people = Vec::new();
    let mut benunits = Vec::new();
    let mut households = Vec::new();

    let mut person_id = 0usize;
    let mut bu_id = 0usize;

    let income_profiles: Vec<IncomeProfile> = generate_income_distribution();

    let regions = [
        (Region::NorthEast, 0.041),
        (Region::NorthWest, 0.110),
        (Region::Yorkshire, 0.083),
        (Region::EastMidlands, 0.073),
        (Region::WestMidlands, 0.089),
        (Region::EastOfEngland, 0.094),
        (Region::London, 0.131),
        (Region::SouthEast, 0.138),
        (Region::SouthWest, 0.086),
        (Region::Wales, 0.048),
        (Region::Scotland, 0.083),
        (Region::NorthernIreland, 0.028),
    ];

    let total_households = 20_000usize;
    let weight_per_hh = 28_200_000.0 / total_households as f64;

    for hh_idx in 0..total_households {
        let profile = &income_profiles[hh_idx % income_profiles.len()];
        let region = assign_region(hh_idx, total_households, &regions);

        let mut hh_person_ids = Vec::new();
        let mut hh_bu_ids = Vec::new();
        let mut bu_person_ids = Vec::new();

        // Adult 1
        let mut p1 = make_person(
            person_id, bu_id, hh_idx,
            profile.adult1_age,
            profile.adult1_employment,
            profile.adult1_self_employment,
            profile.adult1_pension,
            profile.savings_income,
            profile.dividend_income,
            profile.property_income,
            region.is_scotland(),
            if profile.adult1_employment > 0.0 { 37.5 * 52.0 } else { 0.0 },
            hh_idx % 20 == 0,
            hh_idx % 30 == 0,
        );
        p1.is_benunit_head = true;
        p1.is_household_head = true;
        p1.gender = Gender::Male;
        bu_person_ids.push(person_id);
        hh_person_ids.push(person_id);
        people.push(p1);
        person_id += 1;

        // Adult 2 (if couple)
        if profile.is_couple {
            let mut p2 = make_person(
                person_id, bu_id, hh_idx,
                profile.adult2_age,
                profile.adult2_employment,
                0.0, 0.0, 0.0, 0.0, 0.0,
                region.is_scotland(),
                if profile.adult2_employment > 0.0 { 25.0 * 52.0 } else { 0.0 },
                false, false,
            );
            p2.gender = Gender::Female;
            bu_person_ids.push(person_id);
            hh_person_ids.push(person_id);
            people.push(p2);
            person_id += 1;
        }

        // Children
        for c in 0..profile.num_children {
            let child_age = match c { 0 => 8.0, 1 => 5.0, 2 => 2.0, _ => 1.0 };
            let child = make_person(
                person_id, bu_id, hh_idx,
                child_age,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                region.is_scotland(),
                0.0, false, false,
            );
            bu_person_ids.push(person_id);
            hh_person_ids.push(person_id);
            people.push(child);
            person_id += 1;
        }

        let rent = if profile.is_renter {
            match region {
                Region::London => 1500.0,
                Region::SouthEast => 1100.0,
                Region::EastOfEngland => 950.0,
                _ => 700.0,
            }
        } else {
            0.0
        };

        // Deterministic take-up seed from benunit index
        let seed = (((bu_id as u64).wrapping_mul(2654435761)) & 0xFFFF) as f64 / 65536.0;
        benunits.push(BenUnit {
            id: bu_id,
            household_id: hh_idx,
            person_ids: bu_person_ids.clone(),
            take_up_seed: seed,
            on_uc: seed < 0.50,     // ~50% on UC in synthetic data
            on_legacy: seed >= 0.50 && seed < 0.65,  // ~15% on legacy
            rent_monthly: rent,
            is_lone_parent: false,
        });
        hh_bu_ids.push(bu_id);
        bu_id += 1;

        households.push(Household {
            id: hh_idx,
            benunit_ids: hh_bu_ids,
            person_ids: hh_person_ids,
            weight: weight_per_hh,
            region,
            rent: rent * 12.0,
            council_tax: 1800.0,
        });
    }

    Dataset {
        people,
        benunits,
        households,
        name: format!("Synthetic FRS {}", year),
        year,
    }
}

#[allow(dead_code)]
struct IncomeProfile {
    adult1_age: f64,
    adult1_employment: f64,
    adult1_self_employment: f64,
    adult1_pension: f64,
    adult2_age: f64,
    adult2_employment: f64,
    is_couple: bool,
    num_children: usize,
    savings_income: f64,
    dividend_income: f64,
    property_income: f64,
    is_renter: bool,
    claims_uc: bool,
}

/// Generate income profiles matching UK income distribution.
fn generate_income_distribution() -> Vec<IncomeProfile> {
    let mut profiles = Vec::new();

    // Decile 1: Workless/very low income (10%)
    for i in 0..200 {
        profiles.push(IncomeProfile {
            adult1_age: 25.0 + (i % 40) as f64,
            adult1_employment: (i as f64 * 30.0).min(5000.0),
            adult1_self_employment: 0.0,
            adult1_pension: if i % 3 == 0 { 9000.0 } else { 0.0 },
            adult2_age: 0.0,
            adult2_employment: 0.0,
            is_couple: false,
            num_children: if i % 3 == 0 { 1 } else { 0 },
            savings_income: 0.0,
            dividend_income: 0.0,
            property_income: 0.0,
            is_renter: true,
            claims_uc: true,
        });
    }

    // Decile 2-3: Low earners (20%)
    for i in 0..400 {
        let emp = 8000.0 + (i as f64 / 400.0) * 10000.0;
        profiles.push(IncomeProfile {
            adult1_age: 22.0 + (i % 45) as f64,
            adult1_employment: emp,
            adult1_self_employment: 0.0,
            adult1_pension: 0.0,
            adult2_age: if i % 3 == 0 { 25.0 + (i % 30) as f64 } else { 0.0 },
            adult2_employment: if i % 3 == 0 { emp * 0.5 } else { 0.0 },
            is_couple: i % 3 == 0,
            num_children: if i % 4 == 0 { 2 } else if i % 3 == 0 { 1 } else { 0 },
            savings_income: 50.0,
            dividend_income: 0.0,
            property_income: 0.0,
            is_renter: i % 2 == 0,
            claims_uc: emp < 15000.0,
        });
    }

    // Decile 4-5: Below median (20%)
    for i in 0..400 {
        let emp = 18000.0 + (i as f64 / 400.0) * 12000.0;
        profiles.push(IncomeProfile {
            adult1_age: 28.0 + (i % 35) as f64,
            adult1_employment: emp,
            adult1_self_employment: if i % 10 == 0 { 5000.0 } else { 0.0 },
            adult1_pension: 0.0,
            adult2_age: if i % 2 == 0 { 27.0 + (i % 30) as f64 } else { 0.0 },
            adult2_employment: if i % 2 == 0 { emp * 0.6 } else { 0.0 },
            is_couple: i % 2 == 0,
            num_children: if i % 3 == 0 { 1 } else { 0 },
            savings_income: 200.0,
            dividend_income: 0.0,
            property_income: 0.0,
            is_renter: i % 3 == 0,
            claims_uc: false,
        });
    }

    // Decile 6-7: Median earners (20%)
    for i in 0..400 {
        let emp = 30000.0 + (i as f64 / 400.0) * 15000.0;
        profiles.push(IncomeProfile {
            adult1_age: 30.0 + (i % 30) as f64,
            adult1_employment: emp,
            adult1_self_employment: 0.0,
            adult1_pension: 0.0,
            adult2_age: if i % 2 == 0 { 29.0 + (i % 25) as f64 } else { 0.0 },
            adult2_employment: if i % 2 == 0 { emp * 0.7 } else { 0.0 },
            is_couple: i % 2 == 0,
            num_children: if i % 4 == 0 { 2 } else if i % 3 == 0 { 1 } else { 0 },
            savings_income: 500.0,
            dividend_income: 200.0,
            property_income: 0.0,
            is_renter: i % 4 == 0,
            claims_uc: false,
        });
    }

    // Decile 8-9: Higher earners (20%)
    for i in 0..400 {
        let emp = 45000.0 + (i as f64 / 400.0) * 40000.0;
        profiles.push(IncomeProfile {
            adult1_age: 35.0 + (i % 25) as f64,
            adult1_employment: emp,
            adult1_self_employment: if i % 5 == 0 { 10000.0 } else { 0.0 },
            adult1_pension: 0.0,
            adult2_age: if i % 2 == 0 { 33.0 + (i % 20) as f64 } else { 0.0 },
            adult2_employment: if i % 2 == 0 { 30000.0 } else { 0.0 },
            is_couple: i % 2 == 0,
            num_children: if i % 5 == 0 { 2 } else if i % 3 == 0 { 1 } else { 0 },
            savings_income: 1500.0,
            dividend_income: 2000.0,
            property_income: if i % 8 == 0 { 8000.0 } else { 0.0 },
            is_renter: i % 5 == 0,
            claims_uc: false,
        });
    }

    // Decile 10: Top earners (10%)
    for i in 0..200 {
        let emp = 85000.0 + (i as f64 / 200.0) * 200000.0;
        profiles.push(IncomeProfile {
            adult1_age: 40.0 + (i % 25) as f64,
            adult1_employment: emp,
            adult1_self_employment: if i % 3 == 0 { 30000.0 } else { 0.0 },
            adult1_pension: 0.0,
            adult2_age: if i % 2 == 0 { 38.0 + (i % 20) as f64 } else { 0.0 },
            adult2_employment: if i % 2 == 0 { 45000.0 } else { 0.0 },
            is_couple: i % 2 == 0,
            num_children: if i % 4 == 0 { 2 } else if i % 5 == 0 { 1 } else { 0 },
            savings_income: 5000.0 + (i as f64 * 100.0),
            dividend_income: 10000.0 + (i as f64 * 200.0),
            property_income: if i % 3 == 0 { 20000.0 } else { 0.0 },
            is_renter: false,
            claims_uc: false,
        });
    }

    profiles
}

fn assign_region(hh_idx: usize, total: usize, regions: &[(Region, f64)]) -> Region {
    let fraction = hh_idx as f64 / total as f64;
    let mut cumulative = 0.0;
    for (region, share) in regions {
        cumulative += share;
        if fraction < cumulative {
            return *region;
        }
    }
    Region::London
}
