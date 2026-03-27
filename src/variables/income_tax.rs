use crate::engine::entities::Person;
use crate::engine::simulation::PersonResult;
use crate::parameters::{Parameters, TaxBracket};

/// Calculate all person-level tax results: income tax + NI (Class 1 and Class 4).
pub fn calculate(person: &Person, params: &Parameters) -> PersonResult {
    let total_income = person.total_income();

    // Step 1: Adjusted net income (for PA taper)
    let adjusted_net_income = total_income;

    // Step 2: Personal allowance (tapered for high earners)
    let personal_allowance = calculate_personal_allowance(adjusted_net_income, params);

    // Step 3: Taxable income
    let taxable_income = (total_income - personal_allowance).max(0.0);

    // Step 4: Income tax
    let brackets = if person.is_in_scotland {
        &params.income_tax.scottish_brackets
    } else {
        &params.income_tax.uk_brackets
    };
    let income_tax = apply_brackets(taxable_income, brackets);

    // Step 5: National Insurance
    // Class 1 (employee) on employment income
    let ni_class1 = calculate_ni_class1(person, params);
    // Class 4 (self-employed) on self-employment profits
    let ni_class4 = calculate_ni_class4(person, params);
    let national_insurance = ni_class1 + ni_class4;

    PersonResult {
        income_tax,
        national_insurance,
        total_income,
        taxable_income,
        personal_allowance,
        adjusted_net_income,
    }
}

/// Personal allowance with taper: reduced by £1 for every £2 above £100k
fn calculate_personal_allowance(adjusted_net_income: f64, params: &Parameters) -> f64 {
    let pa = params.income_tax.personal_allowance;
    let excess = (adjusted_net_income - params.income_tax.pa_taper_threshold).max(0.0);
    let reduction = excess * params.income_tax.pa_taper_rate;
    (pa - reduction).max(0.0)
}

/// Apply graduated tax brackets to taxable income
fn apply_brackets(taxable_income: f64, brackets: &[TaxBracket]) -> f64 {
    let mut tax = 0.0;
    for i in 0..brackets.len() {
        let lower = brackets[i].threshold;
        let upper = if i + 1 < brackets.len() {
            brackets[i + 1].threshold
        } else {
            f64::INFINITY
        };
        let band_income = (taxable_income - lower).min(upper - lower).max(0.0);
        tax += band_income * brackets[i].rate;
    }
    tax
}

/// National Insurance: Class 1 employee contributions (on employment income)
fn calculate_ni_class1(person: &Person, params: &Parameters) -> f64 {
    let earnings = person.employment_income;
    let ni = &params.national_insurance;

    let main_band = (earnings.min(ni.upper_earnings_limit_annual) - ni.primary_threshold_annual).max(0.0);
    let additional_band = (earnings - ni.upper_earnings_limit_annual).max(0.0);

    main_band * ni.main_rate + additional_band * ni.additional_rate
}

/// National Insurance: Class 4 contributions (on self-employment profits)
fn calculate_ni_class4(person: &Person, params: &Parameters) -> f64 {
    let profits = person.self_employment_income;
    if profits <= 0.0 {
        return 0.0;
    }
    let ni = &params.national_insurance;

    let main_band = (profits.min(ni.class4_upper_profits_limit) - ni.class4_lower_profits_limit).max(0.0);
    let additional_band = (profits - ni.class4_upper_profits_limit).max(0.0);

    main_band * ni.class4_main_rate + additional_band * ni.class4_additional_rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::entities::Person;
    use crate::parameters::Parameters;

    fn test_person(employment_income: f64) -> Person {
        Person {
            id: 0, benunit_id: 0, household_id: 0,
            age: 35.0,
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
        }
    }

    fn test_person_se(self_employment_income: f64) -> Person {
        Person {
            id: 0, benunit_id: 0, household_id: 0,
            age: 35.0,
            employment_income: 0.0,
            self_employment_income,
            pension_income: 0.0,
            savings_interest_income: 0.0,
            dividend_income: 0.0,
            property_income: 0.0,
            other_income: 0.0,
            is_in_scotland: false,
            hours_worked: 37.5,
            is_disabled: false,
            is_carer: false,
        }
    }

    #[test]
    fn test_basic_rate_taxpayer() {
        let params = Parameters::for_year(2025).unwrap();
        let result = calculate(&test_person(30000.0), &params);
        // Taxable = 30000 - 12570 = 17430
        // Tax = 17430 * 0.20 = 3486
        assert!((result.income_tax - 3486.0).abs() < 1.0);
        assert!((result.personal_allowance - 12570.0).abs() < 0.01);
    }

    #[test]
    fn test_higher_rate_taxpayer() {
        let params = Parameters::for_year(2025).unwrap();
        let result = calculate(&test_person(60000.0), &params);
        // Taxable = 60000 - 12570 = 47430
        // Basic: 37700 * 0.20 = 7540
        // Higher: (47430 - 37700) * 0.40 = 3892
        // Total: 11432
        assert!((result.income_tax - 11432.0).abs() < 1.0);
    }

    #[test]
    fn test_pa_taper() {
        let params = Parameters::for_year(2025).unwrap();
        let result = calculate(&test_person(125140.0), &params);
        // PA fully tapered away at 125140
        assert!(result.personal_allowance < 1.0);
    }

    #[test]
    fn test_ni_class1() {
        let params = Parameters::for_year(2025).unwrap();
        let result = calculate(&test_person(30000.0), &params);
        // NI = (30000 - 12570) * 0.08 = 1394.40
        assert!((result.national_insurance - 1394.40).abs() < 1.0);
    }

    #[test]
    fn test_ni_class4() {
        let params = Parameters::for_year(2025).unwrap();
        let result = calculate(&test_person_se(40000.0), &params);
        // Class 4: (40000 - 12570) * 0.06 = 1645.80
        assert!((result.national_insurance - 1645.80).abs() < 1.0);
    }
}
