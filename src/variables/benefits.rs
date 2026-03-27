use crate::engine::entities::*;
use crate::engine::simulation::*;
use crate::parameters::Parameters;

/// Calculate all benefit-unit-level benefits.
pub fn calculate_benunit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    params: &Parameters,
) -> BenUnitResult {
    let child_benefit = calculate_child_benefit(bu, people, person_results, params);
    let uc = calculate_universal_credit(bu, people, person_results, params);
    let state_pension = calculate_state_pension(bu, people);
    let pension_credit = calculate_pension_credit(bu, people, params);

    BenUnitResult {
        universal_credit: uc.0,
        child_benefit,
        state_pension,
        pension_credit,
        total_benefits: uc.0 + child_benefit + state_pension + pension_credit,
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
    if !bu.would_claim_child_benefit {
        return 0.0;
    }

    let num_children = bu.num_children(people);
    if num_children == 0 {
        return 0.0;
    }

    let weekly = params.child_benefit.eldest_weekly
        + params.child_benefit.additional_weekly * (num_children as f64 - 1.0).max(0.0);
    let annual = weekly * 52.0;

    // HICBC: clawed back between threshold and taper_end based on highest earner
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
///
/// MaxUC = standard_allowance + child_elements + housing + disability + LCWRA + carer
///
/// Earned income (after work allowance, tax, pension contribs) is tapered at 55%.
/// Unearned income (savings interest, maintenance, etc.) reduces UC pound-for-pound.
///
/// UC = max(0, MaxUC - earned_income_reduction - unearned_income)
///
/// All UC parameter amounts are monthly. We annualise by multiplying by 12.
///
/// Returns (uc_amount, max_amount, income_reduction) — all annual.
fn calculate_universal_credit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
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

    // ── Standard allowance (monthly → annual) ──
    let standard_allowance_monthly = if is_couple {
        if eldest_age >= 25.0 { uc.standard_allowance_couple_over25 }
        else { uc.standard_allowance_couple_under25 }
    } else if eldest_age >= 25.0 {
        uc.standard_allowance_single_over25
    } else {
        uc.standard_allowance_single_under25
    };

    // ── Child element (subject to 2-child limit) ──
    let capped_children = num_children.min(uc.child_limit);
    let child_element_monthly = if capped_children == 0 {
        0.0
    } else {
        uc.child_element_first + uc.child_element_subsequent * (capped_children as f64 - 1.0).max(0.0)
    };

    // ── Disabled child element ──
    let disabled_child_monthly: f64 = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_child())
        .map(|&pid| {
            let p = &people[pid];
            if p.is_severely_disabled || p.is_enhanced_disabled {
                uc.disabled_child_higher
            } else if p.is_disabled {
                uc.disabled_child_lower
            } else {
                0.0
            }
        })
        .sum();

    // ── LCWRA element (if any adult has limited capability for work) ──
    let has_lcwra = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_disabled);
    let lcwra_monthly = if has_lcwra { uc.lcwra_element } else { 0.0 };

    // ── Carer element ──
    let has_carer = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_carer);
    let carer_monthly = if has_carer { uc.carer_element } else { 0.0 };

    // ── Housing element (simplified: rent passed through, minus non-dep deduction) ──
    let housing_element_monthly = bu.rent_monthly;

    let max_amount_monthly = standard_allowance_monthly
        + child_element_monthly
        + disabled_child_monthly
        + lcwra_monthly
        + carer_monthly
        + housing_element_monthly;
    let max_amount_annual = max_amount_monthly * 12.0;

    // ── Work allowance ──
    let has_work_allowance = has_housing_costs || num_children > 0 || has_lcwra;
    let work_allowance_annual = if has_work_allowance {
        if has_housing_costs {
            uc.work_allowance_lower * 12.0
        } else {
            uc.work_allowance_higher * 12.0
        }
    } else {
        0.0
    };

    // ── Earned income (employment + self-employment) ──
    let gross_earned: f64 = bu.person_ids.iter()
        .map(|&pid| people[pid].employment_income + people[pid].self_employment_income)
        .sum();

    // Deductions from earned income: income tax + NI + pension contributions
    let tax_and_ni: f64 = bu.person_ids.iter()
        .map(|&pid| person_results[pid].income_tax + person_results[pid].national_insurance)
        .sum();
    let pension_contribs: f64 = bu.person_ids.iter()
        .map(|&pid| people[pid].employee_pension_contributions + people[pid].personal_pension_contributions)
        .sum();

    let net_earned = (gross_earned - tax_and_ni - pension_contribs).max(0.0);
    let earned_after_allowance = (net_earned - work_allowance_annual).max(0.0);
    let earned_income_reduction = earned_after_allowance * uc.taper_rate;

    // ── Unearned income (reduces UC pound-for-pound) ──
    let unearned_income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            // Key unearned income sources for UC means test
            p.savings_interest_income
                + p.pension_income
                + p.maintenance_income
                + p.property_income
                + p.other_income
        })
        .sum();

    let total_reduction = (earned_income_reduction + unearned_income).min(max_amount_annual);
    let uc_amount = (max_amount_annual - total_reduction).max(0.0);

    (uc_amount, max_amount_annual, total_reduction)
}

/// State Pension: passthrough from reported amounts.
/// Full calculation from NI records is not feasible in a microsim —
/// we use the FRS-reported amounts which are already annualised.
fn calculate_state_pension(bu: &BenUnit, people: &[Person]) -> f64 {
    bu.person_ids.iter()
        .map(|&pid| people[pid].state_pension_reported)
        .sum()
}

/// Pension Credit (Guarantee Credit only — simplified):
/// GC = max(0, minimum_guarantee - income) for SP-age benefit units.
///
/// Income includes: state pension + private pension + earned income + savings.
/// Savings credit is a small top-up we omit for simplicity.
fn calculate_pension_credit(bu: &BenUnit, people: &[Person], params: &Parameters) -> f64 {
    if !bu.would_claim_pc {
        return 0.0;
    }

    // Check if any adult is over state pension age
    let any_sp_age = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_sp_age());
    if !any_sp_age {
        return 0.0;
    }

    let is_couple = bu.is_couple(people);
    let pc = &params.pension_credit;

    // Minimum guarantee (weekly → annual)
    let min_guarantee_weekly = if is_couple {
        pc.standard_minimum_couple
    } else {
        pc.standard_minimum_single
    };
    let min_guarantee_annual = min_guarantee_weekly * 52.0;

    // Income for PC purposes (annual)
    let income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            p.state_pension_reported
                + p.pension_income
                + p.employment_income
                + p.self_employment_income
                + p.savings_interest_income
        })
        .sum();

    (min_guarantee_annual - income).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single_bu(employment_income: f64, num_children: usize) -> (Vec<Person>, BenUnit) {
        let mut people = vec![{
            let mut p = Person::default();
            p.age = 30.0;
            p.employment_income = employment_income;
            p.hours_worked = 37.5 * 52.0;
            p
        }];
        let mut ids = vec![0];
        for i in 0..num_children {
            let mut child = Person::default();
            child.id = i + 1;
            child.age = 5.0;
            people.push(child);
            ids.push(i + 1);
        }
        let bu = BenUnit {
            id: 0,
            household_id: 0,
            person_ids: ids,
            would_claim_uc: true,
            would_claim_child_benefit: true,
            would_claim_pc: true,
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

    #[test]
    fn test_uc_disabled_child_element() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu) = make_single_bu(10000.0, 1);
        people[1].is_disabled = true;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &params);
        // Should include disabled child lower element
        assert!(result.uc_max_amount > 0.0);
        // Max should be higher than without disability
        let (people2, bu2) = make_single_bu(10000.0, 1);
        let pr2: Vec<PersonResult> = people2.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result2 = calculate_benunit(&bu2, &people2, &pr2, &params);
        assert!(result.uc_max_amount > result2.uc_max_amount,
            "Disabled child should increase UC max amount");
    }

    #[test]
    fn test_uc_with_lcwra() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu) = make_single_bu(0.0, 0);
        people[0].is_disabled = true;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &params);
        // Should include LCWRA element + standard allowance + housing
        let expected_min = (params.universal_credit.standard_allowance_single_over25
            + params.universal_credit.lcwra_element
            + 800.0) * 12.0;
        assert!((result.uc_max_amount - expected_min).abs() < 1.0,
            "Expected max ~{}, got {}", expected_min, result.uc_max_amount);
    }

    #[test]
    fn test_uc_unearned_income_reduces() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu) = make_single_bu(0.0, 0);
        people[0].savings_interest_income = 5000.0;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &params);
        // Unearned income should reduce UC
        assert!(result.uc_income_reduction >= 5000.0,
            "£5000 unearned income should reduce UC by at least £5000, got {}", result.uc_income_reduction);
    }
}
