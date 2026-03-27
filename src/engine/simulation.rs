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
    pub total_income: f64,
    pub taxable_income: f64,
    pub personal_allowance: f64,
    pub adjusted_net_income: f64,
}

/// Results for a benefit unit
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct BenUnitResult {
    pub universal_credit: f64,
    pub child_benefit: f64,
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

        // Phase 2: BenUnit-level calculations (parallelised)
        let br: Vec<BenUnitResult> = self.benunits.par_iter().map(|bu| {
            variables::benefits::calculate_benunit(bu, &self.people, &person_results, &self.parameters)
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

            HouseholdResult {
                gross_income: gross,
                total_tax,
                total_benefits,
                net_income: gross - total_tax + total_benefits,
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
