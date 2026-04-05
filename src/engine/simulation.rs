use rayon::prelude::*;
use crate::engine::entities::*;
use crate::parameters::Parameters;
use crate::variables;

/// Results for a single person
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct PersonResult {
    pub income_tax: f64,
    pub national_insurance: f64,
    pub employer_ni: f64,
    pub total_income: f64,
    pub taxable_income: f64,
    pub personal_allowance: f64,
    pub adjusted_net_income: f64,
    pub unused_personal_allowance: f64,
    pub marriage_allowance_deduction: f64,
    /// High Income Child Benefit Charge — income tax charge on the highest
    /// earner in a benefit unit receiving child benefit.
    pub hicbc: f64,
}

/// Results for a benefit unit
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct BenUnitResult {
    pub universal_credit: f64,
    pub child_benefit: f64,
    pub state_pension: f64,
    pub pension_credit: f64,
    pub housing_benefit: f64,
    pub child_tax_credit: f64,
    pub working_tax_credit: f64,
    pub income_support: f64,
    pub esa_income_related: f64,
    pub jsa_income_based: f64,
    pub carers_allowance: f64,
    pub scottish_child_payment: f64,
    pub benefit_cap_reduction: f64,
    /// Passthrough reported benefits not modelled (PIP, DLA, AA, ESA-C, JSA-C)
    pub passthrough_benefits: f64,
    pub total_benefits: f64,
    pub uc_max_amount: f64,
    pub uc_income_reduction: f64,
}

/// Results for a household
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct HouseholdResult {
    pub net_income: f64,
    pub total_tax: f64,
    pub total_benefits: f64,
    pub gross_income: f64,
    /// VAT paid by the household (estimated from consumption or disposable income)
    pub vat: f64,
    /// Modified OECD equivalisation factor for the household
    pub equivalisation_factor: f64,
    /// HBAI net income BHC (before housing costs)
    pub equivalised_net_income: f64,
    /// HBAI net income AHC (after housing costs = BHC - rent - council tax)
    pub net_income_ahc: f64,
    /// HBAI equivalised net income AHC
    pub equivalised_net_income_ahc: f64,
}

/// Complete simulation result set
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SimulationResults {
    pub person_results: Vec<PersonResult>,
    pub benunit_results: Vec<BenUnitResult>,
    pub household_results: Vec<HouseholdResult>,
}

/// The microsimulation engine.
pub struct Simulation {
    pub people: Vec<Person>,
    pub benunits: Vec<BenUnit>,
    pub households: Vec<Household>,
    pub parameters: Parameters,
    /// Baseline old basic SP weekly rate for scaling reported amounts under reforms.
    pub baseline_old_sp_weekly: f64,
    /// Fiscal year (e.g. 2025 for 2025/26) — used for new/basic SP cutoff.
    pub fiscal_year: u32,
}

impl Simulation {
    pub fn new(
        people: Vec<Person>,
        benunits: Vec<BenUnit>,
        households: Vec<Household>,
        parameters: Parameters,
        fiscal_year: u32,
    ) -> Self {
        let baseline_old_sp_weekly = parameters.state_pension.old_basic_pension_weekly;
        Simulation {
            people, benunits, households, parameters,
            baseline_old_sp_weekly, fiscal_year,
        }
    }

    /// Create a simulation with explicit baseline old SP rate (for reform simulations
    /// where the baseline rate differs from the reform parameters).
    pub fn new_with_baseline_sp(
        people: Vec<Person>,
        benunits: Vec<BenUnit>,
        households: Vec<Household>,
        parameters: Parameters,
        baseline_old_sp_weekly: f64,
        fiscal_year: u32,
    ) -> Self {
        Simulation {
            people, benunits, households, parameters,
            baseline_old_sp_weekly, fiscal_year,
        }
    }

    /// Run the full simulation. Calculates all tax-benefit variables for every entity.
    /// Uses Rayon for parallel computation across households.
    pub fn run(&self) -> SimulationResults {
        let mut person_results = vec![PersonResult::default(); self.people.len()];
        let mut benunit_results = vec![BenUnitResult::default(); self.benunits.len()];
        let mut household_results = vec![HouseholdResult::default(); self.households.len()];

        // Phase 1a: Calculate each person's state pension under the current policy.
        // State pension is taxable income so must be computed before income tax.
        let baseline_old_sp = self.baseline_old_sp_weekly;
        let fiscal_year = self.fiscal_year;
        let person_sp: Vec<f64> = self.people.par_iter().map(|p| {
            variables::benefits::person_state_pension(
                p, &self.parameters, baseline_old_sp, fiscal_year,
            )
        }).collect();

        // Phase 1b: Person-level tax calculations (parallelised).
        // Income tax receives the calculated SP amount so reforms flow through correctly.
        let pr: Vec<PersonResult> = self.people.par_iter().enumerate().map(|(i, person)| {
            variables::income_tax::calculate(person, &self.parameters, person_sp[i])
        }).collect();
        person_results = pr;

        // Phase 1c: Marriage allowance (benunit-level adjustment to person tax)
        // Cannot be parallelised as it mutates person_results across benunits
        for bu in &self.benunits {
            variables::income_tax::apply_marriage_allowance(
                bu, &self.people, &mut person_results, &self.parameters,
            );
        }

        // Phase 2: BenUnit-level calculations (parallelised)
        let br: Vec<BenUnitResult> = self.benunits.par_iter().map(|bu| {
            let hh = &self.households[bu.household_id];
            variables::benefits::calculate_benunit(
                bu, &self.people, &person_results, hh, &self.parameters,
                baseline_old_sp, fiscal_year,
            )
        }).collect();
        benunit_results = br;

        // Phase 2b: HICBC — the highest earner in each benunit pays back child
        // benefit as an income tax charge, tapered between hicbc_threshold and
        // hicbc_taper_end based on adjusted net income.
        for bu in &self.benunits {
            let cb = benunit_results[bu.id].child_benefit;
            if cb <= 0.0 { continue; }

            let threshold = self.parameters.child_benefit.hicbc_threshold;
            let taper_end = self.parameters.child_benefit.hicbc_taper_end;

            // Find the highest earner among adults
            let highest_pid = bu.person_ids.iter()
                .copied()
                .filter(|&pid| self.people[pid].is_adult())
                .max_by(|&a, &b| {
                    person_results[a].adjusted_net_income
                        .partial_cmp(&person_results[b].adjusted_net_income)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some(pid) = highest_pid {
                let ani = person_results[pid].adjusted_net_income;
                let charge = if ani <= threshold {
                    0.0
                } else if ani >= taper_end {
                    cb
                } else {
                    let fraction = (ani - threshold) / (taper_end - threshold);
                    cb * fraction
                };
                if charge > 0.0 {
                    person_results[pid].hicbc = charge;
                    person_results[pid].income_tax += charge;
                }
            }
        }

        // Phase 3: Household-level aggregation (parallelised)
        let hr: Vec<HouseholdResult> = self.households.par_iter().map(|hh| {
            // Gross income uses calculated SP (from Phase 1a) instead of reported amounts,
            // so SP reforms flow through to gross/net income correctly.
            let gross: f64 = hh.person_ids.iter()
                .map(|&pid| {
                    person_results[pid].total_income
                })
                .sum::<f64>();

            let calculated_sp: f64 = hh.person_ids.iter()
                .map(|&pid| person_sp[pid])
                .sum();

            let direct_tax: f64 = hh.person_ids.iter()
                .map(|&pid| person_results[pid].income_tax + person_results[pid].national_insurance)
                .sum();

            let total_benefits: f64 = hh.benunit_ids.iter()
                .map(|&bid| benunit_results[bid].total_benefits)
                .sum();

            // State pension is already in gross (adjusted above) so exclude
            // it from benefits when computing net income to avoid double-counting.
            let state_pension: f64 = calculated_sp;

            // Pension contributions are deducted from net income (as in FRS NINDINC/HBAI)
            let pension_contributions: f64 = hh.person_ids.iter()
                .map(|&pid| self.people[pid].employee_pension_contributions + self.people[pid].personal_pension_contributions)
                .sum();

            // In-kind benefits included in HBAI net income
            let in_kind_benefits: f64 = hh.benunit_ids.iter()
                .map(|&bid| {
                    let bu = &self.benunits[bid];
                    bu.free_school_meals + bu.free_school_fruit_veg + bu.free_school_milk
                        + bu.healthy_start_vouchers + bu.free_tv_licence
                })
                .sum();

            let net_income_before_vat = gross - direct_tax - pension_contributions
                + total_benefits - state_pension + in_kind_benefits;

            // VAT: computed from consumption data (EFRS) or estimated from disposable income
            let vat = variables::vat::calculate_household_vat(
                hh, net_income_before_vat, &self.parameters,
            );

            let total_tax = direct_tax + vat;
            let net_income = net_income_before_vat - vat;

            // Modified OECD equivalisation scale (used by HBAI):
            // First adult: 0.67, additional adults (14+): 0.33, children (<14): 0.20
            let mut adults = 0usize;
            let mut children = 0usize;
            for &pid in &hh.person_ids {
                if self.people[pid].age >= 14.0 {
                    adults += 1;
                } else {
                    children += 1;
                }
            }
            let eq_factor = if adults == 0 { 1.0 } else {
                0.67 + (adults.saturating_sub(1) as f64) * 0.33 + (children as f64) * 0.20
            };

            // AHC: subtract rent and council tax (housing costs)
            let housing_costs = hh.rent + hh.council_tax;
            let net_income_ahc = net_income - housing_costs;

            HouseholdResult {
                gross_income: gross,
                total_tax,
                total_benefits,
                net_income,
                vat,
                equivalisation_factor: eq_factor,
                equivalised_net_income: net_income / eq_factor,
                net_income_ahc,
                equivalised_net_income_ahc: net_income_ahc / eq_factor,
            }
        }).collect();
        household_results = hr;

        SimulationResults {
            person_results,
            benunit_results,
            household_results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::entities::{Person, BenUnit, Household, Region};

    fn make_hicbc_sim(income: f64, params: Parameters) -> Simulation {
        let mut adult = Person::default();
        adult.id = 0;
        adult.age = 35.0;
        adult.employment_income = income;
        adult.hours_worked = 37.5 * 52.0;

        let mut child = Person::default();
        child.id = 1;
        child.age = 5.0;

        let bu = BenUnit {
            id: 0, household_id: 0, person_ids: vec![0, 1],
            migration_seed: 0.0, would_claim_cb: true,
            ..BenUnit::default()
        };
        let hh = Household {
            id: 0, person_ids: vec![0, 1], benunit_ids: vec![0],
            weight: 1.0, region: Region::London, council_tax: 1500.0,
            ..Household::default()
        };

        Simulation::new(vec![adult, child], vec![bu], vec![hh], params, 2025)
    }

    #[test]
    fn hicbc_zero_below_threshold() {
        let params = Parameters::for_year(2025).unwrap();
        let sim = make_hicbc_sim(50000.0, params);
        let results = sim.run();
        assert!(results.person_results[0].hicbc < 0.01,
            "No HICBC below threshold, got {}", results.person_results[0].hicbc);
        assert!(results.benunit_results[0].child_benefit > 0.0,
            "Should receive full child benefit");
    }

    #[test]
    fn hicbc_full_above_taper_end() {
        let params = Parameters::for_year(2025).unwrap();
        let sim = make_hicbc_sim(90000.0, params);
        let results = sim.run();
        let cb = results.benunit_results[0].child_benefit;
        assert!(cb > 0.0, "Full child benefit should be paid");
        assert!((results.person_results[0].hicbc - cb).abs() < 1.0,
            "HICBC should equal full CB above taper end: hicbc={}, cb={}",
            results.person_results[0].hicbc, cb);
    }

    #[test]
    fn hicbc_partial_in_taper_zone() {
        let params = Parameters::for_year(2025).unwrap();
        // £70k is halfway between threshold (60k) and taper_end (80k)
        let sim = make_hicbc_sim(70000.0, params);
        let results = sim.run();
        let cb = results.benunit_results[0].child_benefit;
        let hicbc = results.person_results[0].hicbc;
        assert!(hicbc > 0.0, "HICBC should be positive in taper zone");
        assert!(hicbc < cb, "HICBC should be less than full CB in taper zone");
        // Roughly 50% clawback at midpoint (adjusted net income may differ slightly from gross)
        assert!(hicbc > cb * 0.3 && hicbc < cb * 0.7,
            "HICBC should be roughly 50% of CB at midpoint: hicbc={}, cb={}", hicbc, cb);
    }

    #[test]
    fn hicbc_threshold_param_responsive() {
        let mut params = Parameters::for_year(2025).unwrap();
        let sim_base = make_hicbc_sim(65000.0, params.clone());
        let base_hicbc = sim_base.run().person_results[0].hicbc;

        params.child_benefit.hicbc_threshold += 3000.0;
        let sim_reform = make_hicbc_sim(65000.0, params);
        let reform_hicbc = sim_reform.run().person_results[0].hicbc;

        assert!(reform_hicbc < base_hicbc,
            "Raising HICBC threshold should reduce charge: base={}, reform={}", base_hicbc, reform_hicbc);
    }

    #[test]
    fn hicbc_taper_end_param_responsive() {
        let mut params = Parameters::for_year(2025).unwrap();
        let sim_base = make_hicbc_sim(70000.0, params.clone());
        let base_hicbc = sim_base.run().person_results[0].hicbc;

        params.child_benefit.hicbc_taper_end += 10000.0;
        let sim_reform = make_hicbc_sim(70000.0, params);
        let reform_hicbc = sim_reform.run().person_results[0].hicbc;

        assert!(reform_hicbc < base_hicbc,
            "Raising HICBC taper end should reduce charge: base={}, reform={}", base_hicbc, reform_hicbc);
    }

    #[test]
    fn hicbc_included_in_income_tax() {
        let params = Parameters::for_year(2025).unwrap();
        let sim = make_hicbc_sim(90000.0, params);
        let results = sim.run();
        let hicbc = results.person_results[0].hicbc;
        let it = results.person_results[0].income_tax;
        assert!(hicbc > 0.0);
        // Income tax should include HICBC
        assert!(it > hicbc, "Income tax ({}) should be greater than HICBC ({}) alone", it, hicbc);
    }
}
