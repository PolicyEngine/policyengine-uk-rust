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
    pub scottish_child_payment: f64,
    pub benefit_cap_reduction: f64,
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
    /// Modified OECD equivalisation factor for the household
    pub equivalisation_factor: f64,
    /// HBAI-definition equivalised net income BHC
    pub equivalised_net_income: f64,
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
}

impl Simulation {
    pub fn new(
        people: Vec<Person>,
        benunits: Vec<BenUnit>,
        households: Vec<Household>,
        parameters: Parameters,
    ) -> Self {
        Simulation { people, benunits, households, parameters }
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
        let br: Vec<BenUnitResult> = self.benunits.par_iter().map(|bu| {
            let hh = &self.households[bu.household_id];
            variables::benefits::calculate_benunit(bu, &self.people, &person_results, hh, &self.parameters)
        }).collect();
        benunit_results = br;

        // Phase 3: Household-level aggregation (parallelised)
        let hr: Vec<HouseholdResult> = self.households.par_iter().map(|hh| {
            let gross: f64 = hh.person_ids.iter()
                .map(|&pid| self.people[pid].total_income())
                .sum();

            let total_tax: f64 = hh.person_ids.iter()
                .map(|&pid| person_results[pid].income_tax + person_results[pid].national_insurance)
                .sum();

            let total_benefits: f64 = hh.benunit_ids.iter()
                .map(|&bid| benunit_results[bid].total_benefits)
                .sum();

            let net_income = gross - total_tax + total_benefits;

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

            HouseholdResult {
                gross_income: gross,
                total_tax,
                total_benefits,
                net_income,
                equivalisation_factor: eq_factor,
                equivalised_net_income: net_income / eq_factor,
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
