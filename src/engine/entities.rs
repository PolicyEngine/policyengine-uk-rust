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
    pub state_pension: f64,
    pub savings_interest_income: f64,
    pub dividend_income: f64,
    pub property_income: f64,
    pub maintenance_income: f64,
    pub miscellaneous_income: f64,
    pub other_income: f64,

    // Employment
    pub is_in_scotland: bool,
    pub hours_worked: f64,             // annual hours

    // Disability/carer status — granular rate-band flags derived from FRS benefit amounts
    // DLA care component (SSCBA 1992 Sch.2 para.2 as amended)
    pub dla_care_low: bool,     // lowest rate
    pub dla_care_mid: bool,     // middle rate
    pub dla_care_high: bool,    // highest rate
    // DLA mobility component (SSCBA 1992 Sch.2 para.3)
    pub dla_mob_low: bool,
    pub dla_mob_high: bool,
    // PIP daily living component (WRA 2012 s.79 / PIP Regs 2013 SI 2013/377)
    pub pip_dl_std: bool,
    pub pip_dl_enh: bool,
    // PIP mobility component (WRA 2012 s.79)
    pub pip_mob_std: bool,
    pub pip_mob_enh: bool,
    // Attendance Allowance (SSCBA 1992 s.64)
    pub aa_low: bool,
    pub aa_high: bool,
    // Convenience aggregates (kept for backwards compat with UC/IS/HB logic)
    pub is_disabled: bool,          // any PIP/DLA/AA receipt
    pub is_enhanced_disabled: bool, // DLA care high OR PIP DL enhanced (disabled child higher rate)
    pub is_severely_disabled: bool, // PIP DL enhanced or DLA care high (SDP proxy)
    pub is_carer: bool,             // CA receipt
    // Employment/health status from FRS (for ESA/JSA eligibility)
    pub limitill: bool,     // LIMITILL: has limiting long-standing illness
    pub esa_group: i64,     // ESAGRP: 1=support, 2=WRAG, 3=assessment, 0=none/unknown
    pub emp_status: i64,    // EMPSTATB: 1=employed, 2=self-employed, 3=unemployed, 4=inactive
    pub looking_for_work: bool,     // LOOKWK: actively looking for work
    pub is_self_identified_carer: bool, // CARER1: identifies as unpaid carer

    // Pension contributions (annual)
    pub employee_pension_contributions: f64,
    pub personal_pension_contributions: f64,

    // Childcare (annual)
    pub childcare_expenses: f64,

    // Benefit amounts (annual) — from FRS microdata, used for take-up and passthrough
    pub child_benefit: f64,
    pub housing_benefit: f64,
    pub income_support: f64,
    pub pension_credit: f64,
    pub child_tax_credit: f64,
    pub working_tax_credit: f64,
    pub universal_credit: f64,
    pub dla_care: f64,
    pub dla_mobility: f64,
    pub pip_daily_living: f64,
    pub pip_mobility: f64,
    pub carers_allowance: f64,
    pub attendance_allowance: f64,
    pub esa_income: f64,
    pub esa_contributory: f64,
    pub jsa_income: f64,
    pub jsa_contributory: f64,
    /// Aggregate of unmodelled passthrough benefits (bereavement, maternity, winter fuel, etc.)
    pub other_benefits: f64,
    /// Scottish disability replacements (ADP replaces PIP for Scottish adults)
    pub adp_daily_living: f64,
    pub adp_mobility: f64,
    /// Scottish child disability (CDP replaces DLA for Scottish children)
    pub cdp_care: f64,
    pub cdp_mobility: f64,

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
            state_pension: 0.0,
            savings_interest_income: 0.0,
            dividend_income: 0.0,
            property_income: 0.0,
            maintenance_income: 0.0,
            miscellaneous_income: 0.0,
            other_income: 0.0,
            is_in_scotland: false,
            hours_worked: 0.0,
            dla_care_low: false,
            dla_care_mid: false,
            dla_care_high: false,
            dla_mob_low: false,
            dla_mob_high: false,
            pip_dl_std: false,
            pip_dl_enh: false,
            pip_mob_std: false,
            pip_mob_enh: false,
            aa_low: false,
            aa_high: false,
            is_disabled: false,
            is_enhanced_disabled: false,
            is_severely_disabled: false,
            is_carer: false,
            limitill: false,
            esa_group: 0,
            emp_status: 0,
            looking_for_work: false,
            is_self_identified_carer: false,
            employee_pension_contributions: 0.0,
            personal_pension_contributions: 0.0,
            childcare_expenses: 0.0,
            child_benefit: 0.0,
            housing_benefit: 0.0,
            income_support: 0.0,
            pension_credit: 0.0,
            child_tax_credit: 0.0,
            working_tax_credit: 0.0,
            universal_credit: 0.0,
            dla_care: 0.0,
            dla_mobility: 0.0,
            pip_daily_living: 0.0,
            pip_mobility: 0.0,
            carers_allowance: 0.0,
            attendance_allowance: 0.0,
            esa_income: 0.0,
            esa_contributory: 0.0,
            jsa_income: 0.0,
            jsa_contributory: 0.0,
            other_benefits: 0.0,
            adp_daily_living: 0.0,
            adp_mobility: 0.0,
            cdp_care: 0.0,
            cdp_mobility: 0.0,
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
            + self.state_pension
            + self.savings_interest_income
            + self.dividend_income
            + self.property_income
            + self.maintenance_income
            + self.miscellaneous_income
            + self.other_income
    }

    /// Earned income (employment + self-employment).
    #[allow(dead_code)]
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
#[derive(Debug, Clone, Default)]
pub struct BenUnit {
    pub id: usize,
    pub household_id: usize,
    pub person_ids: Vec<usize>,
    /// Random seed [0, 1) for take-up decisions — deterministic per benunit.
    pub take_up_seed: f64,
    /// Whether this benunit reported UC receipt in the FRS.
    pub on_uc: bool,
    /// Whether this benunit reported any legacy means-tested benefit (HB/CTC/WTC/IS) in the FRS.
    pub on_legacy: bool,
    pub rent_monthly: f64,
    pub is_lone_parent: bool,

    // Reported receipt flags (true = any member reported non-zero amount)
    pub reported_cb: bool,
    pub reported_uc: bool,
    pub reported_hb: bool,
    pub reported_pc: bool,
    pub reported_ctc: bool,
    pub reported_wtc: bool,
    pub reported_is: bool,

    // Entitled Non-Recipient flags (computed at extract time from baseline policy)
    // True = model says entitled under baseline policy but no reported receipt.
    pub is_enr_uc: bool,
    pub is_enr_hb: bool,
    pub is_enr_pc: bool,
    pub is_enr_cb: bool,
    pub is_enr_ctc: bool,
    pub is_enr_wtc: bool,

    // In-kind benefits (annual, from FRS DVs — included in HBAI net income)
    pub free_school_meals: f64,      // FSMBU
    pub free_school_fruit_veg: f64,  // FSFVBU
    pub free_school_milk: f64,       // FSMLKBU
    pub healthy_start_vouchers: f64, // HEARTBU
    pub free_tv_licence: f64,        // BUTVLIC
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
