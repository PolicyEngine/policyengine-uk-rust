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
    /// Baseline SP weekly rates for scaling reported amounts under reforms.
    pub baseline_new_sp_weekly: f64,
    pub baseline_old_sp_weekly: f64,
}

impl Simulation {
    pub fn new(
        people: Vec<Person>,
        benunits: Vec<BenUnit>,
        households: Vec<Household>,
        parameters: Parameters,
    ) -> Self {
        let baseline_new_sp_weekly = parameters.state_pension.new_state_pension_weekly;
        let baseline_old_sp_weekly = parameters.state_pension.old_basic_pension_weekly;
        Simulation {
            people, benunits, households, parameters,
            baseline_new_sp_weekly, baseline_old_sp_weekly,
        }
    }

    /// Create a simulation with explicit baseline SP rates (for reform simulations
    /// where the baseline rates differ from the reform parameters).
    pub fn new_with_baseline_sp(
        people: Vec<Person>,
        benunits: Vec<BenUnit>,
        households: Vec<Household>,
        parameters: Parameters,
        baseline_new_sp_weekly: f64,
        baseline_old_sp_weekly: f64,
    ) -> Self {
        Simulation {
            people, benunits, households, parameters,
            baseline_new_sp_weekly, baseline_old_sp_weekly,
        }
    }

    /// Run the full simulation. Calculates all tax-benefit variables for every entity.
    /// Uses Rayon for parallel computation across households.
    pub fn run(&self) -> SimulationResults {
        let mut person_results = vec![PersonResult::default(); self.people.len()];
        let mut benunit_results = vec![BenUnitResult::default(); self.benunits.len()];
        let mut household_results = vec![HouseholdResult::default(); self.households.len()];

        // Phase 1: Person-level calculations (parallelised)
        let pr: Vec<PersonResult> = self.people.par_iter().map(|person| {
            variables::income_tax::calculate(person, &self.parameters)
        }).collect();
        person_results = pr;

        // Phase 1b: Marriage allowance (benunit-level adjustment to person tax)
        // Cannot be parallelised as it mutates person_results across benunits
        for bu in &self.benunits {
            variables::income_tax::apply_marriage_allowance(
                bu, &self.people, &mut person_results, &self.parameters,
            );
        }

        // Phase 2: BenUnit-level calculations (parallelised)
        let baseline_new_sp = self.baseline_new_sp_weekly;
        let baseline_old_sp = self.baseline_old_sp_weekly;
        let br: Vec<BenUnitResult> = self.benunits.par_iter().map(|bu| {
            let hh = &self.households[bu.household_id];
            variables::benefits::calculate_benunit(
                bu, &self.people, &person_results, hh, &self.parameters,
                baseline_new_sp, baseline_old_sp,
            )
        }).collect();
        benunit_results = br;

        // Phase 3: Household-level aggregation (parallelised)
        let hr: Vec<HouseholdResult> = self.households.par_iter().map(|hh| {
            // Gross income uses reported amounts. When SP parameters change,
            // we need to adjust the reported SP component to match the reform.
            let reported_sp: f64 = hh.person_ids.iter()
                .map(|&pid| self.people[pid].state_pension)
                .sum();
            let calculated_sp: f64 = hh.benunit_ids.iter()
                .map(|&bid| benunit_results[bid].state_pension)
                .sum();
            // SP adjustment = difference between calculated (reform-scaled) and reported
            let sp_adjustment = calculated_sp - reported_sp;

            let gross: f64 = hh.person_ids.iter()
                .map(|&pid| self.people[pid].total_income())
                .sum::<f64>() + sp_adjustment;

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
