use crate::engine::entities::*;
use crate::engine::simulation::*;
use crate::parameters::Parameters;

/// Calculate all benefit-unit-level benefits: Universal Credit + Child Benefit.
pub fn calculate_benunit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    params: &Parameters,
) -> BenUnitResult {
    let child_benefit = calculate_child_benefit(bu, people, person_results, params);
    let uc = calculate_universal_credit(bu, people, person_results, params);

    BenUnitResult {
        universal_credit: uc.0,
        child_benefit,
        total_benefits: uc.0 + child_benefit,
        uc_max_amount: uc.1,
        uc_income_reduction: uc.2,
    }
}

/// Child Benefit: eldest child gets higher rate, others get additional rate.
/// Subject to High Income Child Benefit Charge (HICBC).
fn calculate_child_benefit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    params: &Parameters,
) -> f64 {
    let num_children = bu.num_children(people);
    if num_children == 0 {
        return 0.0;
    }

    let weekly = params.child_benefit.eldest_weekly
        + params.child_benefit.additional_weekly * (num_children as f64 - 1.0).max(0.0);
    let annual = weekly * 52.0;

    // HICBC: clawed back between threshold and taper_end
    let max_income: f64 = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .map(|&pid| person_results[pid].adjusted_net_income)
        .fold(0.0_f64, f64::max);

    if max_income <= params.child_benefit.hicbc_threshold {
        annual
    } else if max_income >= params.child_benefit.hicbc_taper_end {
        0.0
    } else {
        let fraction = (max_income - params.child_benefit.hicbc_threshold)
            / (params.child_benefit.hicbc_taper_end - params.child_benefit.hicbc_threshold);
        annual * (1.0 - fraction)
    }
}

/// Universal Credit calculation:
/// MaxUC = standard_allowance + child_elements + housing
/// UC = max(0, MaxUC - taper * max(0, earned_income - work_allowance))
///
/// All UC amounts in parameters are monthly (per assessment period).
/// We annualise by multiplying by 12 for comparison with annual incomes.
///
/// Returns (uc_amount, max_amount, income_reduction) — all annual.
fn calculate_universal_credit(
    bu: &BenUnit,
    people: &[Person],
    _person_results: &[PersonResult],
    params: &Parameters,
) -> (f64, f64, f64) {
    if !bu.would_claim_uc {
        return (0.0, 0.0, 0.0);
    }

    let uc = &params.universal_credit;
    let is_couple = bu.is_couple(people);
    let eldest_age = bu.eldest_adult_age(people);
    let num_children = bu.num_children(people);
    let has_housing_costs = bu.rent_monthly > 0.0;

    // Standard allowance (monthly → annual)
    let standard_allowance_monthly = if is_couple {
        if eldest_age >= 25.0 {
            uc.standard_allowance_couple_over25
        } else {
            uc.standard_allowance_couple_under25
        }
    } else if eldest_age >= 25.0 {
        uc.standard_allowance_single_over25
    } else {
        uc.standard_allowance_single_under25
    };

    // Child element: first child gets higher amount, subsequent get lower,
    // subject to 2-child limit
    let capped_children = num_children.min(uc.child_limit);
    let child_element_monthly = if capped_children == 0 {
        0.0
    } else {
        uc.child_element_first + uc.child_element_subsequent * (capped_children as f64 - 1.0).max(0.0)
    };

    // Housing element (simplified: rent passed through)
    let housing_element_monthly = bu.rent_monthly;

    let max_amount_monthly = standard_allowance_monthly + child_element_monthly + housing_element_monthly;
    let max_amount_annual = max_amount_monthly * 12.0;

    // Work allowance (monthly → annual)
    let work_allowance_annual = if has_housing_costs || num_children > 0 {
        if has_housing_costs {
            uc.work_allowance_lower * 12.0
        } else {
            uc.work_allowance_higher * 12.0
        }
    } else {
        0.0  // No work allowance if no children and no housing costs
    };

    // Earned income for UC means test
    let earned_income: f64 = bu.person_ids.iter()
        .map(|&pid| people[pid].employment_income + people[pid].self_employment_income)
        .sum();

    let income_after_allowance = (earned_income - work_allowance_annual).max(0.0);
    let income_reduction = income_after_allowance * uc.taper_rate;

    let uc_amount = (max_amount_annual - income_reduction).max(0.0);
    (uc_amount, max_amount_annual, income_reduction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single_bu(employment_income: f64, num_children: usize) -> (Vec<Person>, BenUnit) {
        let mut people = vec![Person {
            id: 0, benunit_id: 0, household_id: 0,
            age: 30.0,
            employment_income,
            self_employment_income: 0.0,
            pension_income: 0.0,
            savings_interest_income: 0.0,
            dividend_income: 0.0,
            property_income: 0.0,
            other_income: 0.0,
            is_in_scotland: false,
            hours_worked: 37.5,
            is_disabled: false,
            is_carer: false,
        }];
        let mut ids = vec![0];
        for i in 0..num_children {
            people.push(Person {
                id: i + 1, benunit_id: 0, household_id: 0,
                age: 5.0,
                employment_income: 0.0,
                self_employment_income: 0.0,
                pension_income: 0.0,
                savings_interest_income: 0.0,
                dividend_income: 0.0,
                property_income: 0.0,
                other_income: 0.0,
                is_in_scotland: false,
                hours_worked: 0.0,
                is_disabled: false,
                is_carer: false,
            });
            ids.push(i + 1);
        }
        let bu = BenUnit {
            id: 0,
            household_id: 0,
            person_ids: ids,
            would_claim_uc: true,
            rent_monthly: 800.0,
        };
        (people, bu)
    }

    #[test]
    fn test_child_benefit_two_children() {
        let params = Parameters::for_year(2025).unwrap();
        let (people, bu) = make_single_bu(25000.0, 2);
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &params);
        let expected_cb = params.child_benefit.eldest_weekly * 52.0
            + params.child_benefit.additional_weekly * 52.0;
        assert!((result.child_benefit - expected_cb).abs() < 1.0);
    }

    #[test]
    fn test_uc_low_earner() {
        let params = Parameters::for_year(2025).unwrap();
        let (people, bu) = make_single_bu(10000.0, 1);
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &params);
        assert!(result.universal_credit > 0.0, "Low earner should receive UC");
    }
}
