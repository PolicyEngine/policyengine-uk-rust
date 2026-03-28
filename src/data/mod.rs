pub mod frs;
pub mod clean;

use crate::engine::entities::*;

/// A complete dataset ready for microsimulation
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Dataset {
    pub people: Vec<Person>,
    pub benunits: Vec<BenUnit>,
    pub households: Vec<Household>,
    pub name: String,
    pub year: u32,
}

#[allow(dead_code)]
impl Dataset {
    pub fn num_households(&self) -> usize {
        self.households.len()
    }

    pub fn weighted_population(&self) -> f64 {
        self.households.iter().map(|h| h.weight).sum()
    }

    /// Uprate all monetary amounts from the dataset's current year to `target_year`
    /// using OBR earnings growth forecasts. Mutates in place and updates `self.year`.
    pub fn uprate_to(&mut self, target_year: u32) {
        let factor = earnings_uprating_factor(self.year, target_year);
        if (factor - 1.0).abs() < 1e-9 {
            self.year = target_year;
            return;
        }
        for p in &mut self.people {
            p.employment_income *= factor;
            p.self_employment_income *= factor;
            p.pension_income *= factor;
            p.state_pension *= factor;
            p.savings_interest_income *= factor;
            p.dividend_income *= factor;
            p.property_income *= factor;
            p.maintenance_income *= factor;
            p.miscellaneous_income *= factor;
            p.other_income *= factor;
            p.employee_pension_contributions *= factor;
            p.personal_pension_contributions *= factor;
            p.childcare_expenses *= factor;
            p.child_benefit *= factor;
            p.housing_benefit *= factor;
            p.income_support *= factor;
            p.pension_credit *= factor;
            p.child_tax_credit *= factor;
            p.working_tax_credit *= factor;
            p.universal_credit *= factor;
            p.dla_care *= factor;
            p.dla_mobility *= factor;
            p.pip_daily_living *= factor;
            p.pip_mobility *= factor;
            p.carers_allowance *= factor;
            p.attendance_allowance *= factor;
            p.esa_income *= factor;
            p.esa_contributory *= factor;
            p.jsa_income *= factor;
            p.jsa_contributory *= factor;
            p.other_benefits *= factor;
            p.adp_daily_living *= factor;
            p.adp_mobility *= factor;
            p.cdp_care *= factor;
            p.cdp_mobility *= factor;
        }
        for h in &mut self.households {
            h.rent *= factor;
            h.council_tax *= factor;
        }
        self.year = target_year;
        self.name = format!("{} (uprated to {}/{})", self.name, target_year, (target_year + 1) % 100);
    }
}

/// Cumulative earnings growth factor from `base_year` to `target_year` using OBR forecasts.
fn earnings_uprating_factor(base_year: u32, target_year: u32) -> f64 {
    // Annual earnings growth rates by fiscal year (OBR March 2026 EFO)
    let rates: &[(u32, f64)] = &[
        (2024, 0.0479),  // 2023/24 → 2024/25
        (2025, 0.0479),  // 2024/25 → 2025/26
        (2026, 0.03172), // 2025/26 → 2026/27
        (2027, 0.02192),
        (2028, 0.02121),
        (2029, 0.02253),
    ];
    let rate_for = |y: u32| -> f64 {
        rates.iter().find(|(yr, _)| *yr == y).map(|(_, r)| *r).unwrap_or(0.02)
    };

    if target_year == base_year {
        return 1.0;
    }
    if target_year > base_year {
        let mut factor = 1.0;
        for y in (base_year + 1)..=target_year {
            factor *= 1.0 + rate_for(y);
        }
        factor
    } else {
        let mut factor = 1.0;
        for y in (target_year + 1)..=base_year {
            factor /= 1.0 + rate_for(y);
        }
        factor
    }
}
