/// Entity types in the UK tax-benefit system.
/// Person → BenUnit (benefit unit / family) → Household
///
/// A household contains one or more benefit units, each containing one or more persons.

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Person {
    pub id: usize,
    pub benunit_id: usize,
    pub household_id: usize,
    pub age: f64,
    pub employment_income: f64,
    pub self_employment_income: f64,
    pub pension_income: f64,
    pub savings_interest_income: f64,
    pub dividend_income: f64,
    pub property_income: f64,
    pub other_income: f64,
    pub is_in_scotland: bool,
    pub hours_worked: f64,
    pub is_disabled: bool,
    pub is_carer: bool,
}

impl Person {
    pub fn total_income(&self) -> f64 {
        self.employment_income
            + self.self_employment_income
            + self.pension_income
            + self.savings_interest_income
            + self.dividend_income
            + self.property_income
            + self.other_income
    }

    pub fn is_adult(&self) -> bool {
        self.age >= 18.0
    }

    pub fn is_child(&self) -> bool {
        self.age < 18.0
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BenUnit {
    pub id: usize,
    pub household_id: usize,
    pub person_ids: Vec<usize>,
    pub would_claim_uc: bool,
    pub rent_monthly: f64,
}

impl BenUnit {
    pub fn num_adults(&self, people: &[Person]) -> usize {
        self.person_ids.iter()
            .filter(|&&pid| people[pid].is_adult())
            .count()
    }

    pub fn num_children(&self, people: &[Person]) -> usize {
        self.person_ids.iter()
            .filter(|&&pid| people[pid].is_child())
            .count()
    }

    pub fn is_couple(&self, people: &[Person]) -> bool {
        self.num_adults(people) >= 2
    }

    pub fn eldest_adult_age(&self, people: &[Person]) -> f64 {
        self.person_ids.iter()
            .filter(|&&pid| people[pid].is_adult())
            .map(|&pid| people[pid].age)
            .fold(0.0_f64, f64::max)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Household {
    pub id: usize,
    pub benunit_ids: Vec<usize>,
    pub person_ids: Vec<usize>,
    pub weight: f64,
    pub region: Region,
    pub rent: f64,
    pub council_tax: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    NorthEast,
    NorthWest,
    Yorkshire,
    EastMidlands,
    WestMidlands,
    EastOfEngland,
    London,
    SouthEast,
    SouthWest,
    Wales,
    Scotland,
    NorthernIreland,
}

#[allow(dead_code)]
impl Region {
    pub fn is_scotland(&self) -> bool {
        matches!(self, Region::Scotland)
    }

    pub fn from_frs_code(code: i32) -> Self {
        match code {
            1 => Region::NorthEast,
            2 => Region::NorthWest,
            4 => Region::Yorkshire,
            5 => Region::EastMidlands,
            6 => Region::WestMidlands,
            7 => Region::EastOfEngland,
            8 => Region::London,
            9 => Region::SouthEast,
            10 => Region::SouthWest,
            11 => Region::Wales,
            12 => Region::Scotland,
            13 => Region::NorthernIreland,
            _ => Region::London,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Region::NorthEast => "North East",
            Region::NorthWest => "North West",
            Region::Yorkshire => "Yorkshire",
            Region::EastMidlands => "East Midlands",
            Region::WestMidlands => "West Midlands",
            Region::EastOfEngland => "East of England",
            Region::London => "London",
            Region::SouthEast => "South East",
            Region::SouthWest => "South West",
            Region::Wales => "Wales",
            Region::Scotland => "Scotland",
            Region::NorthernIreland => "Northern Ireland",
        }
    }
}
