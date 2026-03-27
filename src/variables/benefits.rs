use crate::engine::entities::*;
use crate::engine::simulation::*;
use crate::parameters::Parameters;

/// Calculate all benefit-unit-level benefits.
pub fn calculate_benunit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    household: &Household,
    params: &Parameters,
) -> BenUnitResult {
    let child_benefit = calculate_child_benefit(bu, people, person_results, params);
    let uc = calculate_universal_credit(bu, people, person_results, params);
    let state_pension = calculate_state_pension(bu, people);
    let pension_credit = calculate_pension_credit(bu, people, params);
    let housing_benefit = calculate_housing_benefit(bu, people, person_results, params);
    let (ctc, wtc) = calculate_tax_credits(bu, people, person_results, params);
    let income_support = calculate_income_support(bu, people, person_results, params);
    let ctr = calculate_council_tax_reduction(bu, people, person_results, household, params);
    let scp = calculate_scottish_child_payment(bu, people, household, params);

    // Sum pre-cap benefits
    let pre_cap_benefits = uc.0 + child_benefit + state_pension + pension_credit
        + housing_benefit + ctc + wtc + income_support + ctr + scp;

    // Apply benefit cap
    let benefit_cap_reduction = calculate_benefit_cap(
        bu, people, person_results, household, params,
        pre_cap_benefits, child_benefit, state_pension,
    );

    let total_benefits = (pre_cap_benefits - benefit_cap_reduction).max(0.0);

    BenUnitResult {
        universal_credit: uc.0,
        child_benefit,
        state_pension,
        pension_credit,
        housing_benefit,
        child_tax_credit: ctc,
        working_tax_credit: wtc,
        income_support,
        council_tax_reduction: ctr,
        scottish_child_payment: scp,
        benefit_cap_reduction,
        total_benefits,
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

/// Universal Credit calculation.
///
/// MaxUC = standard_allowance + child_elements + housing + disability + LCWRA + carer
/// Earned income (after work allowance, tax, pension contribs) tapered at 55%.
/// Unearned income reduces UC pound-for-pound.
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

    // Standard allowance (monthly → annual)
    let standard_allowance_monthly = if is_couple {
        if eldest_age >= 25.0 { uc.standard_allowance_couple_over25 }
        else { uc.standard_allowance_couple_under25 }
    } else if eldest_age >= 25.0 {
        uc.standard_allowance_single_over25
    } else {
        uc.standard_allowance_single_under25
    };

    // Child element (subject to 2-child limit)
    let capped_children = num_children.min(uc.child_limit);
    let child_element_monthly = if capped_children == 0 {
        0.0
    } else {
        uc.child_element_first + uc.child_element_subsequent * (capped_children as f64 - 1.0).max(0.0)
    };

    // Disabled child element
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

    // LCWRA element
    let has_lcwra = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_disabled);
    let lcwra_monthly = if has_lcwra { uc.lcwra_element } else { 0.0 };

    // Carer element
    let has_carer = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_carer);
    let carer_monthly = if has_carer { uc.carer_element } else { 0.0 };

    // Housing element
    let housing_element_monthly = bu.rent_monthly;

    let max_amount_monthly = standard_allowance_monthly
        + child_element_monthly
        + disabled_child_monthly
        + lcwra_monthly
        + carer_monthly
        + housing_element_monthly;
    let max_amount_annual = max_amount_monthly * 12.0;

    // Work allowance
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

    // Earned income
    let gross_earned: f64 = bu.person_ids.iter()
        .map(|&pid| people[pid].employment_income + people[pid].self_employment_income)
        .sum();

    let tax_and_ni: f64 = bu.person_ids.iter()
        .map(|&pid| person_results[pid].income_tax + person_results[pid].national_insurance)
        .sum();
    let pension_contribs: f64 = bu.person_ids.iter()
        .map(|&pid| people[pid].employee_pension_contributions + people[pid].personal_pension_contributions)
        .sum();

    let net_earned = (gross_earned - tax_and_ni - pension_contribs).max(0.0);
    let earned_after_allowance = (net_earned - work_allowance_annual).max(0.0);
    let earned_income_reduction = earned_after_allowance * uc.taper_rate;

    // Unearned income (reduces UC pound-for-pound)
    let unearned_income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
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
fn calculate_state_pension(bu: &BenUnit, people: &[Person]) -> f64 {
    bu.person_ids.iter()
        .map(|&pid| people[pid].state_pension_reported)
        .sum()
}

/// Pension Credit: Guarantee Credit + Savings Credit.
///
/// Guarantee Credit: max(0, minimum_guarantee - income).
/// Savings Credit: max(0, min(income - threshold, max_savings_credit) - max(0, income - minimum_guarantee) * 0.40).
/// But savings credit only applies to those reaching SP age before 6 April 2016 — we include it
/// but the data should flag eligibility. Here we calculate it for all SP-age claimants.
fn calculate_pension_credit(bu: &BenUnit, people: &[Person], params: &Parameters) -> f64 {
    if !bu.would_claim_pc {
        return 0.0;
    }

    let any_sp_age = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_sp_age());
    if !any_sp_age {
        return 0.0;
    }

    let is_couple = bu.is_couple(people);
    let pc = &params.pension_credit;

    let min_guarantee_weekly = if is_couple {
        pc.standard_minimum_couple
    } else {
        pc.standard_minimum_single
    };
    let min_guarantee_annual = min_guarantee_weekly * 52.0;

    // Income for PC purposes
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

    // Guarantee Credit
    let gc = (min_guarantee_annual - income).max(0.0);

    // Savings Credit (for those who reached SP age before 6 Apr 2016)
    let sc_threshold = if is_couple {
        pc.savings_credit_threshold_couple
    } else {
        pc.savings_credit_threshold_single
    };
    let sc_threshold_annual = sc_threshold * 52.0;

    // Maximum savings credit = 60% of (minimum guarantee - savings credit threshold)
    let max_sc_weekly = (min_guarantee_weekly - sc_threshold) * 0.60;
    let qualifying_income_weekly = income / 52.0;

    let sc = if qualifying_income_weekly > sc_threshold && max_sc_weekly > 0.0 {
        let credit = (qualifying_income_weekly - sc_threshold).min(max_sc_weekly);
        let excess_over_mg = (qualifying_income_weekly - min_guarantee_weekly).max(0.0);
        let sc_weekly = (credit - excess_over_mg * 0.40).max(0.0);
        sc_weekly * 52.0
    } else {
        0.0
    };

    gc + sc
}

/// Housing Benefit (legacy system).
///
/// HB = max(0, eligible_rent - max(0, (income - applicable_amount) * 65%))
///
/// Applicable amount = personal allowance + family premium + child allowances.
fn calculate_housing_benefit(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    params: &Parameters,
) -> f64 {
    if !bu.would_claim_hb {
        return 0.0;
    }

    let hb_params = match &params.housing_benefit {
        Some(hb) => hb,
        None => return 0.0,
    };

    let eligible_rent = bu.rent_monthly * 12.0;
    if eligible_rent <= 0.0 {
        return 0.0;
    }

    // Applicable amount (weekly → annual)
    let is_couple = bu.is_couple(people);
    let eldest_age = bu.eldest_adult_age(people);
    let num_children = bu.num_children(people);

    let personal_allowance_weekly = if is_couple {
        hb_params.personal_allowance_couple
    } else if eldest_age >= 25.0 {
        hb_params.personal_allowance_single_25_plus
    } else {
        hb_params.personal_allowance_single_under25
    };

    let family_premium_weekly = if num_children > 0 { hb_params.family_premium } else { 0.0 };
    let child_allowance_weekly = hb_params.child_allowance * num_children as f64;

    let applicable_amount = (personal_allowance_weekly + family_premium_weekly + child_allowance_weekly) * 52.0;

    // Income for HB purposes
    let income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            p.employment_income + p.self_employment_income
                + p.pension_income + p.state_pension_reported
                + p.savings_interest_income + p.other_income
        })
        .sum();

    let excess_income = (income - applicable_amount).max(0.0);
    let reduction = excess_income * hb_params.withdrawal_rate;

    (eligible_rent - reduction).max(0.0)
}

/// Tax Credits: Working Tax Credit (WTC) and Child Tax Credit (CTC).
///
/// Maximum = WTC elements + CTC elements.
/// Income reduction = max(0, (income - threshold) * 41%).
/// CTC reduced first, then WTC.
///
/// Returns (ctc, wtc).
fn calculate_tax_credits(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    params: &Parameters,
) -> (f64, f64) {
    let tc = match &params.tax_credits {
        Some(tc) => tc,
        None => return (0.0, 0.0),
    };

    let num_children = bu.num_children(people);
    let is_couple = bu.is_couple(people);

    // CTC: available if there are children
    let max_ctc = if bu.would_claim_ctc && num_children > 0 {
        tc.ctc_family_element + tc.ctc_child_element * num_children as f64
            + bu.person_ids.iter()
                .filter(|&&pid| people[pid].is_child())
                .map(|&pid| {
                    let p = &people[pid];
                    if p.is_severely_disabled || p.is_enhanced_disabled {
                        tc.ctc_severely_disabled_child_element + tc.ctc_disabled_child_element
                    } else if p.is_disabled {
                        tc.ctc_disabled_child_element
                    } else {
                        0.0
                    }
                })
                .sum::<f64>()
    } else {
        0.0
    };

    // WTC: available if working sufficient hours
    let total_hours_weekly: f64 = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .map(|&pid| people[pid].hours_worked / 52.0)
        .sum();

    let min_hours = if is_couple {
        tc.wtc_min_hours_couple
    } else {
        tc.wtc_min_hours_single
    };

    let max_wtc = if bu.would_claim_wtc && total_hours_weekly >= min_hours {
        let mut wtc = tc.wtc_basic_element;
        if is_couple {
            wtc += tc.wtc_couple_element;
        } else if bu.is_lone_parent {
            wtc += tc.wtc_lone_parent_element;
        }
        if total_hours_weekly >= 30.0 {
            wtc += tc.wtc_30_hour_element;
        }
        wtc
    } else {
        0.0
    };

    if max_ctc + max_wtc <= 0.0 {
        return (0.0, 0.0);
    }

    // Income for tax credits
    let income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            p.employment_income + p.self_employment_income
                + p.pension_income + p.state_pension_reported
                + p.savings_interest_income + p.dividend_income
                + p.property_income + p.other_income
        })
        .sum();

    let excess = (income - tc.income_threshold).max(0.0);
    let reduction = excess * tc.taper_rate;

    // CTC reduced first, then WTC
    let ctc = (max_ctc - reduction).max(0.0);
    let remaining_reduction = (reduction - max_ctc).max(0.0);
    let wtc = (max_wtc - remaining_reduction).max(0.0);

    (ctc, wtc)
}

/// Income Support: legacy means-tested benefit for specific groups
/// (lone parents with young children, carers, disabled).
///
/// IS = max(0, applicable_amount - income).
/// Very few new claimants due to UC migration, but still in the system.
fn calculate_income_support(
    bu: &BenUnit,
    people: &[Person],
    _person_results: &[PersonResult],
    params: &Parameters,
) -> f64 {
    if !bu.would_claim_is {
        return 0.0;
    }

    // Use HB applicable amount params (they share the same personal allowance structure)
    let hb_params = match &params.housing_benefit {
        Some(hb) => hb,
        None => return 0.0,
    };

    let is_couple = bu.is_couple(people);
    let eldest_age = bu.eldest_adult_age(people);
    let num_children = bu.num_children(people);

    let personal_allowance_weekly = if is_couple {
        hb_params.personal_allowance_couple
    } else if eldest_age >= 25.0 {
        hb_params.personal_allowance_single_25_plus
    } else {
        hb_params.personal_allowance_single_under25
    };

    let family_premium_weekly = if num_children > 0 { hb_params.family_premium } else { 0.0 };
    let child_allowance_weekly = hb_params.child_allowance * num_children as f64;

    let applicable_amount = (personal_allowance_weekly + family_premium_weekly + child_allowance_weekly) * 52.0;

    let income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            p.employment_income + p.self_employment_income
                + p.pension_income + p.state_pension_reported
                + p.savings_interest_income + p.other_income
        })
        .sum();

    (applicable_amount - income).max(0.0)
}

/// Council Tax Reduction (Council Tax Support).
///
/// CTR = max(0, council_tax - max(0, (income - applicable_amount) * 20%))
fn calculate_council_tax_reduction(
    bu: &BenUnit,
    people: &[Person],
    _person_results: &[PersonResult],
    household: &Household,
    params: &Parameters,
) -> f64 {
    let ctr = match &params.council_tax_reduction {
        Some(ctr) => ctr,
        None => return 0.0,
    };

    let council_tax = household.council_tax;
    if council_tax <= 0.0 {
        return 0.0;
    }

    let is_couple = bu.is_couple(people);
    let eldest_age = bu.eldest_adult_age(people);
    let num_children = bu.num_children(people);

    let personal_allowance_weekly = if is_couple {
        ctr.personal_allowance_couple
    } else if eldest_age >= 25.0 {
        ctr.personal_allowance_single_25_plus
    } else {
        ctr.personal_allowance_single_under25
    };

    let family_premium_weekly = if num_children > 0 { ctr.family_premium } else { 0.0 };
    let child_allowance_weekly = ctr.child_allowance * num_children as f64;

    let applicable_amount = (personal_allowance_weekly + family_premium_weekly + child_allowance_weekly) * 52.0;

    let income: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            p.employment_income + p.self_employment_income
                + p.pension_income + p.state_pension_reported
                + p.savings_interest_income + p.other_income
        })
        .sum();

    let excess_income = (income - applicable_amount).max(0.0);
    let reduction = excess_income * ctr.taper_rate;

    (council_tax - reduction).max(0.0)
}

/// Scottish Child Payment: £26.70/week per eligible child under 16.
/// Only available in Scotland to UC/legacy benefit claimants.
fn calculate_scottish_child_payment(
    bu: &BenUnit,
    people: &[Person],
    household: &Household,
    params: &Parameters,
) -> f64 {
    let scp = match &params.scottish_child_payment {
        Some(scp) => scp,
        None => return 0.0,
    };

    if !household.region.is_scotland() {
        return 0.0;
    }

    // Must be on a qualifying benefit (UC or legacy)
    if !bu.would_claim_uc {
        return 0.0;
    }

    let eligible_children = bu.person_ids.iter()
        .filter(|&&pid| {
            let p = &people[pid];
            p.is_child() && p.age < scp.max_age
        })
        .count();

    scp.weekly_amount * 52.0 * eligible_children as f64
}

/// Benefit Cap: limits total benefits to a maximum level.
///
/// Welfare Reform Act 2012 s.96. Different caps for London/outside London,
/// single/non-single. Exempt if earning above threshold.
///
/// Returns the reduction amount (to be subtracted from total benefits).
fn calculate_benefit_cap(
    bu: &BenUnit,
    people: &[Person],
    person_results: &[PersonResult],
    household: &Household,
    params: &Parameters,
    total_benefits: f64,
    _child_benefit: f64,
    state_pension: f64,
) -> f64 {
    let cap_params = match &params.benefit_cap {
        Some(bc) => bc,
        None => return 0.0,
    };

    // Exempt if earnings above threshold
    let net_earnings: f64 = bu.person_ids.iter()
        .map(|&pid| {
            let p = &people[pid];
            let gross = p.employment_income + p.self_employment_income;
            let deductions = person_results[pid].income_tax + person_results[pid].national_insurance;
            (gross - deductions).max(0.0)
        })
        .sum();

    if net_earnings >= cap_params.earnings_exemption_threshold {
        return 0.0;
    }

    // SP-age exempt
    let any_sp_age = bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_adult())
        .any(|&pid| people[pid].is_sp_age());
    if any_sp_age {
        return 0.0;
    }

    let is_single_no_children = !bu.is_couple(people) && bu.num_children(people) == 0;
    let is_london = household.region == Region::London;

    let annual_cap = if is_single_no_children {
        if is_london { cap_params.single_london } else { cap_params.single_outside_london }
    } else {
        if is_london { cap_params.non_single_london } else { cap_params.non_single_outside_london }
    };

    // Benefits subject to cap (exclude state pension and some disability benefits)
    let capped_benefits = total_benefits - state_pension;

    (capped_benefits - annual_cap).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single_bu(employment_income: f64, num_children: usize) -> (Vec<Person>, BenUnit, Household) {
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
            would_claim_hb: false,
            would_claim_ctc: false,
            would_claim_wtc: false,
            would_claim_is: false,
            rent_monthly: 800.0,
            is_lone_parent: num_children > 0,
        };
        let hh = Household {
            id: 0,
            benunit_ids: vec![0],
            person_ids: (0..=num_children).collect(),
            weight: 1.0,
            region: Region::London,
            rent: 800.0 * 12.0,
            council_tax: 1500.0,
        };
        (people, bu, hh)
    }

    #[test]
    fn test_child_benefit_two_children() {
        let params = Parameters::for_year(2025).unwrap();
        let (people, bu, hh) = make_single_bu(25000.0, 2);
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &hh, &params);
        let expected_cb = params.child_benefit.eldest_weekly * 52.0
            + params.child_benefit.additional_weekly * 52.0;
        assert!((result.child_benefit - expected_cb).abs() < 1.0);
    }

    #[test]
    fn test_uc_low_earner() {
        let params = Parameters::for_year(2025).unwrap();
        let (people, bu, hh) = make_single_bu(10000.0, 1);
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &hh, &params);
        assert!(result.universal_credit > 0.0, "Low earner should receive UC");
    }

    #[test]
    fn test_uc_disabled_child_element() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu, hh) = make_single_bu(10000.0, 1);
        people[1].is_disabled = true;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &hh, &params);
        assert!(result.uc_max_amount > 0.0);

        let (people2, bu2, hh2) = make_single_bu(10000.0, 1);
        let pr2: Vec<PersonResult> = people2.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result2 = calculate_benunit(&bu2, &people2, &pr2, &hh2, &params);
        assert!(result.uc_max_amount > result2.uc_max_amount,
            "Disabled child should increase UC max amount");
    }

    #[test]
    fn test_uc_with_lcwra() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu, hh) = make_single_bu(0.0, 0);
        people[0].is_disabled = true;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &hh, &params);
        let expected_min = (params.universal_credit.standard_allowance_single_over25
            + params.universal_credit.lcwra_element
            + 800.0) * 12.0;
        assert!((result.uc_max_amount - expected_min).abs() < 1.0,
            "Expected max ~{}, got {}", expected_min, result.uc_max_amount);
    }

    #[test]
    fn test_uc_unearned_income_reduces() {
        let params = Parameters::for_year(2025).unwrap();
        let (mut people, bu, hh) = make_single_bu(0.0, 0);
        people[0].savings_interest_income = 5000.0;
        let person_results: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &person_results, &hh, &params);
        assert!(result.uc_income_reduction >= 5000.0,
            "£5000 unearned income should reduce UC by at least £5000, got {}", result.uc_income_reduction);
    }

    #[test]
    fn test_pension_credit_guarantee() {
        let params = Parameters::for_year(2025).unwrap();
        let mut p = Person::default();
        p.age = 70.0;
        p.state_pension_reported = 9000.0; // Below minimum guarantee
        let people = vec![p];
        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0],
            would_claim_uc: false, would_claim_child_benefit: false,
            would_claim_pc: true, would_claim_hb: false,
            would_claim_ctc: false, would_claim_wtc: false,
            would_claim_is: false, rent_monthly: 0.0,
            is_lone_parent: false,
        };
        let hh = Household {
            id: 0, benunit_ids: vec![0], person_ids: vec![0],
            weight: 1.0, region: Region::London, rent: 0.0, council_tax: 0.0,
        };
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        let mg_annual = params.pension_credit.standard_minimum_single * 52.0;
        // GC = mg - income
        assert!(result.pension_credit > 0.0, "Should receive pension credit");
        assert!((result.pension_credit - (mg_annual - 9000.0)).abs() < 200.0,
            "Expected ~{}, got {}", mg_annual - 9000.0, result.pension_credit);
    }

    #[test]
    fn test_housing_benefit() {
        let params = Parameters::for_year(2025).unwrap();
        let mut p = Person::default();
        p.age = 30.0;
        p.employment_income = 10000.0;
        let people = vec![p];
        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0],
            would_claim_uc: false, would_claim_child_benefit: false,
            would_claim_pc: false, would_claim_hb: true,
            would_claim_ctc: false, would_claim_wtc: false,
            would_claim_is: false, rent_monthly: 600.0,
            is_lone_parent: false,
        };
        let hh = Household {
            id: 0, benunit_ids: vec![0], person_ids: vec![0],
            weight: 1.0, region: Region::London, rent: 7200.0, council_tax: 0.0,
        };
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        assert!(result.housing_benefit > 0.0, "Low earner should get HB");
        assert!(result.housing_benefit <= 7200.0, "HB should not exceed rent");
    }

    #[test]
    fn test_tax_credits() {
        let params = Parameters::for_year(2025).unwrap();
        let mut p = Person::default();
        p.age = 30.0;
        p.employment_income = 15000.0;
        p.hours_worked = 35.0 * 52.0;
        let mut child = Person::default();
        child.id = 1;
        child.age = 5.0;
        let people = vec![p, child];
        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0, 1],
            would_claim_uc: false, would_claim_child_benefit: true,
            would_claim_pc: false, would_claim_hb: false,
            would_claim_ctc: true, would_claim_wtc: true,
            would_claim_is: false, rent_monthly: 0.0,
            is_lone_parent: true,
        };
        let hh = Household {
            id: 0, benunit_ids: vec![0], person_ids: vec![0, 1],
            weight: 1.0, region: Region::London, rent: 0.0, council_tax: 0.0,
        };
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        // CTC + WTC should be positive for low-income lone parent
        assert!(result.child_tax_credit > 0.0 || result.working_tax_credit > 0.0,
            "Low-income lone parent should receive tax credits. CTC={}, WTC={}",
            result.child_tax_credit, result.working_tax_credit);
    }

    #[test]
    fn test_benefit_cap() {
        let params = Parameters::for_year(2025).unwrap();
        // Non-working single person in London with massive UC entitlement
        let (mut people, mut bu, hh) = make_single_bu(0.0, 4);
        bu.rent_monthly = 3000.0; // Very high rent to push above cap
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        // With 4 children and £3000/month rent, total benefits should hit cap
        if let Some(bc) = &params.benefit_cap {
            let cap = bc.non_single_london;
            // Total benefits after cap should not exceed cap + state pension (which is exempt)
            assert!(result.total_benefits <= cap + result.state_pension + 1.0,
                "Benefits after cap should be <= £{}, got £{}", cap, result.total_benefits);
        }
    }

    #[test]
    fn test_scottish_child_payment() {
        let params = Parameters::for_year(2025).unwrap();
        let mut p = Person::default();
        p.age = 30.0;
        let mut child = Person::default();
        child.id = 1;
        child.age = 5.0;
        let people = vec![p, child];
        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0, 1],
            would_claim_uc: true, would_claim_child_benefit: true,
            would_claim_pc: false, would_claim_hb: false,
            would_claim_ctc: false, would_claim_wtc: false,
            would_claim_is: false, rent_monthly: 0.0,
            is_lone_parent: true,
        };
        let hh = Household {
            id: 0, benunit_ids: vec![0], person_ids: vec![0, 1],
            weight: 1.0, region: Region::Scotland, rent: 0.0, council_tax: 0.0,
        };
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        if let Some(scp) = &params.scottish_child_payment {
            let expected = scp.weekly_amount * 52.0;
            assert!((result.scottish_child_payment - expected).abs() < 1.0,
                "Expected SCP ~£{}, got £{}", expected, result.scottish_child_payment);
        }
    }

    #[test]
    fn test_council_tax_reduction() {
        let params = Parameters::for_year(2025).unwrap();
        let mut p = Person::default();
        p.age = 30.0;
        p.employment_income = 8000.0;
        let people = vec![p];
        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0],
            would_claim_uc: false, would_claim_child_benefit: false,
            would_claim_pc: false, would_claim_hb: false,
            would_claim_ctc: false, would_claim_wtc: false,
            would_claim_is: false, rent_monthly: 0.0,
            is_lone_parent: false,
        };
        let hh = Household {
            id: 0, benunit_ids: vec![0], person_ids: vec![0],
            weight: 1.0, region: Region::London, rent: 0.0, council_tax: 1800.0,
        };
        let pr: Vec<PersonResult> = people.iter()
            .map(|p| crate::variables::income_tax::calculate(p, &params))
            .collect();
        let result = calculate_benunit(&bu, &people, &pr, &hh, &params);
        // Low earner below applicable amount should get full CTR
        assert!(result.council_tax_reduction > 0.0,
            "Low earner should get council tax reduction");
        assert!(result.council_tax_reduction <= 1800.0,
            "CTR should not exceed council tax");
    }
}
