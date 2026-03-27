pub mod synthetic;
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
}
