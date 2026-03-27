/// Entity types in the UK tax-benefit system.
/// Person → BenUnit (benefit unit / family) → Household
///
/// A household contains one or more benefit units, each containing one or more persons.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Person {
    pub id: usize,
    pub benunit_id: usize,
    pub household_id: usize,
    pub age: f64,
    pub gender: Gender,
    pub is_benunit_head: bool,
    pub is_household_head: bool,

    // Income sources (annual)
    pub employment_income: f64,
    pub self_employment_income: f64,
    pub pension_income: f64,          // private pension income
    pub state_pension_reported: f64,   // reported state pension receipt
    pub savings_interest_income: f64,
    pub dividend_income: f64,
    pub property_income: f64,
    pub maintenance_income: f64,
    pub miscellaneous_income: f64,
    pub other_income: f64,

    // Employment
    pub is_in_scotland: bool,
    pub hours_worked: f64,             // annual hours

    // Disability/carer status
    pub is_disabled: bool,
    pub is_enhanced_disabled: bool,
    pub is_severely_disabled: bool,
    pub is_carer: bool,

    // Pension contributions (annual)
    pub employee_pension_contributions: f64,
    pub personal_pension_contributions: f64,

    // Childcare (annual)
    pub childcare_expenses: f64,

    // Reported benefit amounts (annual) — used for take-up and passthrough
    pub child_benefit_reported: f64,
    pub housing_benefit_reported: f64,
    pub income_support_reported: f64,
    pub pension_credit_reported: f64,
    pub child_tax_credit_reported: f64,
    pub working_tax_credit_reported: f64,
    pub universal_credit_reported: f64,
    pub dla_sc_reported: f64,
    pub dla_m_reported: f64,
    pub pip_dl_reported: f64,
    pub pip_m_reported: f64,
    pub carers_allowance_reported: f64,
    pub attendance_allowance_reported: f64,
    pub esa_income_reported: f64,
    pub esa_contrib_reported: f64,
    pub jsa_income_reported: f64,
    pub jsa_contrib_reported: f64,

    // Take-up flags
    pub would_claim_marriage_allowance: bool,
}

impl Default for Person {
    fn default() -> Self {
        Person {
            id: 0, benunit_id: 0, household_id: 0,
            age: 30.0,
            gender: Gender::Male,
            is_benunit_head: false,
            is_household_head: false,
            employment_income: 0.0,
            self_employment_income: 0.0,
            pension_income: 0.0,
            state_pension_reported: 0.0,
            savings_interest_income: 0.0,
            dividend_income: 0.0,
            property_income: 0.0,
            maintenance_income: 0.0,
            miscellaneous_income: 0.0,
            other_income: 0.0,
            is_in_scotland: false,
            hours_worked: 0.0,
            is_disabled: false,
            is_enhanced_disabled: false,
            is_severely_disabled: false,
            is_carer: false,
            employee_pension_contributions: 0.0,
            personal_pension_contributions: 0.0,
            childcare_expenses: 0.0,
            child_benefit_reported: 0.0,
            housing_benefit_reported: 0.0,
            income_support_reported: 0.0,
            pension_credit_reported: 0.0,
            child_tax_credit_reported: 0.0,
            working_tax_credit_reported: 0.0,
            universal_credit_reported: 0.0,
            dla_sc_reported: 0.0,
            dla_m_reported: 0.0,
            pip_dl_reported: 0.0,
            pip_m_reported: 0.0,
            carers_allowance_reported: 0.0,
            attendance_allowance_reported: 0.0,
            esa_income_reported: 0.0,
            esa_contrib_reported: 0.0,
            jsa_income_reported: 0.0,
            jsa_contrib_reported: 0.0,
            would_claim_marriage_allowance: false,
        }
    }
}

impl Person {
    /// Total gross income from all sources (excluding reported benefits).
    pub fn total_income(&self) -> f64 {
        self.employment_income
            + self.self_employment_income
            + self.pension_income
            + self.state_pension_reported
            + self.savings_interest_income
            + self.dividend_income
            + self.property_income
            + self.maintenance_income
            + self.miscellaneous_income
            + self.other_income
    }

    /// Earned income (employment + self-employment).
    pub fn earned_income(&self) -> f64 {
        self.employment_income + self.self_employment_income
    }

    pub fn is_adult(&self) -> bool {
        self.age >= 18.0
    }

    pub fn is_child(&self) -> bool {
        self.age < 18.0
    }

    /// Whether person is over state pension age (simplified: 66 for all).
    pub fn is_sp_age(&self) -> bool {
        self.age >= 66.0
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BenUnit {
    pub id: usize,
    pub household_id: usize,
    pub person_ids: Vec<usize>,
    pub would_claim_uc: bool,
    pub would_claim_child_benefit: bool,
    pub would_claim_pc: bool,
    pub would_claim_hb: bool,
    pub would_claim_ctc: bool,
    pub would_claim_wtc: bool,
    pub would_claim_is: bool,
    pub rent_monthly: f64,
    pub is_lone_parent: bool,
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
