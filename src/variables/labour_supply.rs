/// OBR labour supply response (intensive margin).
///
/// Implements the Slutsky decomposition from:
/// OBR (2023) "Costing a cut in National Insurance contributions: the impact on labour supply"
/// https://obr.uk/docs/dlm_uploads/NICS-Cut-Impact-on-Labour-Supply-Note.pdf
///
/// For each working adult we compute:
///   ΔE = E_base × (η_s × Δw/w  +  η_i × Δy/y)
///
/// where:
///   η_s = substitution elasticity (marginal net wage change)
///   η_i = income elasticity (net income change)
///   Δw/w = relative change in marginal net wage (1 − marginal effective tax rate)
///   Δy/y = relative change in household net income
///
/// The marginal net wage is estimated numerically: perturb each adult's employment income
/// by DELTA, re-run the simulation, measure how much household net income changes.
/// mtr = 1 − (Δhousehold_net_income / DELTA)
/// net_wage_retention = 1 − mtr = Δhousehold_net_income / DELTA

use crate::engine::entities::{Gender, Person, BenUnit, Household};
use crate::engine::simulation::Simulation;
use crate::parameters::{LabourSupplyParams, Parameters};

/// Perturbation size for numerical MRT derivative (£).
const DELTA: f64 = 1_000.0;

/// Excluded from labour supply responses:
/// - self-employed (emp_status 2)
/// - full-time students (emp_status 5 or hours near zero and young)
/// - aged 60+
/// - zero baseline employment income
fn is_excluded(person: &Person) -> bool {
    person.age >= 60.0
        || person.emp_status == 2  // self-employed
        || person.employment_income <= 0.0
}

/// Youngest child age in a benefit unit (returns f64::MAX if no children).
fn youngest_child_age(bu: &BenUnit, people: &[Person]) -> f64 {
    bu.person_ids.iter()
        .filter(|&&pid| people[pid].is_child())
        .map(|&pid| people[pid].age)
        .fold(f64::MAX, f64::min)
}

/// Select the substitution elasticity for a person given their demographic group.
pub fn substitution_elasticity(
    person: &Person,
    bu: &BenUnit,
    people: &[Person],
    ls: &LabourSupplyParams,
) -> f64 {
    let is_female = person.gender == Gender::Female;
    let is_coupled = bu.is_couple(people);
    let has_children = bu.num_children(people) > 0;

    if is_female && is_coupled {
        if !has_children {
            ls.subst_married_women_no_children
        } else {
            let yca = youngest_child_age(bu, people);
            if yca <= 2.0 { ls.subst_married_women_child_0_2 }
            else if yca <= 4.0 { ls.subst_married_women_child_3_4 }
            else if yca <= 10.0 { ls.subst_married_women_child_5_10 }
            else { ls.subst_married_women_child_11_plus }
        }
    } else if is_female && !is_coupled && has_children {
        // lone parent
        let yca = youngest_child_age(bu, people);
        if yca <= 4.0 { ls.subst_lone_parents_child_0_4 }
        else if yca <= 10.0 { ls.subst_lone_parents_child_5_10 }
        else { ls.subst_lone_parents_child_11_18 }
    } else {
        // men (excl. lone fathers) and single women without children
        ls.subst_men_and_single_women
    }
}

/// Select the income elasticity for a person given their demographic group.
pub fn income_elasticity(
    person: &Person,
    bu: &BenUnit,
    people: &[Person],
    ls: &LabourSupplyParams,
) -> f64 {
    let is_female = person.gender == Gender::Female;
    let is_coupled = bu.is_couple(people);
    let has_children = bu.num_children(people) > 0;

    if is_female && is_coupled {
        if !has_children {
            ls.income_married_women_no_children
        } else {
            let yca = youngest_child_age(bu, people);
            if yca <= 2.0 { ls.income_married_women_child_0_2 }
            else if yca <= 4.0 { ls.income_married_women_child_3_4 }
            else if yca <= 10.0 { ls.income_married_women_child_5_10 }
            else { ls.income_married_women_child_11_plus }
        }
    } else if is_female && !is_coupled && has_children {
        let yca = youngest_child_age(bu, people);
        if yca <= 4.0 { ls.income_lone_parents_child_0_4 }
        else if yca <= 10.0 { ls.income_lone_parents_child_5_10 }
        else { ls.income_lone_parents_child_11_18 }
    } else {
        ls.income_men_and_single_women
    }
}

/// Compute the household net income for a given simulation state.
/// Returns a vec indexed by household id.
fn run_net_incomes(
    people: &[Person],
    benunits: &[BenUnit],
    households: &[Household],
    params: &Parameters,
    fiscal_year: u32,
    baseline_old_sp: f64,
) -> Vec<f64> {
    let sim = Simulation::new_with_baseline_sp(
        people.to_vec(),
        benunits.to_vec(),
        households.to_vec(),
        params.clone(),
        baseline_old_sp,
        fiscal_year,
    );
    let results = sim.run();
    results.household_results.iter().map(|hr| hr.net_income).collect()
}

/// Apply OBR labour supply responses to the policy dataset, returning an adjusted
/// copy of `people` with employment incomes updated.
///
/// `baseline_net` — household net incomes from the baseline simulation (already run).
/// `policy_params` — the reform parameters (used to compute policy-side marginals).
/// `baseline_params` — baseline parameters (used to compute baseline marginals).
pub fn apply_labour_supply_responses(
    people: &[Person],
    benunits: &[BenUnit],
    households: &[Household],
    baseline_params: &Parameters,
    policy_params: &Parameters,
    baseline_net: &[f64],
    fiscal_year: u32,
) -> Vec<Person> {
    let ls = &policy_params.labour_supply;
    if !ls.enabled {
        return people.to_vec();
    }

    let baseline_old_sp = baseline_params.state_pension.old_basic_pension_weekly;
    let n_people = people.len();

    // ── Step 1: compute unperturbed baseline and policy net incomes ──
    let unperturbed_baseline_net = run_net_incomes(
        people, benunits, households,
        baseline_params, fiscal_year, baseline_old_sp,
    );
    let unperturbed_policy_net = run_net_incomes(
        people, benunits, households,
        policy_params, fiscal_year, baseline_old_sp,
    );

    // ── Step 2: compute baseline marginal net wage (retention rate) per person ──
    // Δhousehold_net / DELTA  =  fraction of extra £1 earned that the household keeps.
    let mut baseline_retention = vec![f64::NAN; n_people];
    {
        let mut perturbed = people.to_vec();
        for pid in 0..n_people {
            if is_excluded(&people[pid]) { continue; }
            perturbed[pid].employment_income = people[pid].employment_income + DELTA;
            let perturbed_net = run_net_incomes(
                &perturbed, benunits, households,
                baseline_params, fiscal_year, baseline_old_sp,
            );
            let hid = people[pid].household_id;
            baseline_retention[pid] = ((perturbed_net[hid] - unperturbed_baseline_net[hid]) / DELTA)
                .clamp(0.0, 1.0);
            perturbed[pid].employment_income = people[pid].employment_income; // restore
        }
    }

    // ── Step 3: compute policy marginal net wage (retention rate) per person ──
    let mut policy_retention = vec![f64::NAN; n_people];
    {
        let mut perturbed = people.to_vec();
        for pid in 0..n_people {
            if is_excluded(&people[pid]) { continue; }
            perturbed[pid].employment_income = people[pid].employment_income + DELTA;
            let perturbed_net = run_net_incomes(
                &perturbed, benunits, households,
                policy_params, fiscal_year, baseline_old_sp,
            );
            let hid = people[pid].household_id;
            policy_retention[pid] = ((perturbed_net[hid] - unperturbed_policy_net[hid]) / DELTA)
                .clamp(0.0, 1.0);
            perturbed[pid].employment_income = people[pid].employment_income;
        }
    }

    // ── Step 4: compute ΔE for each person and return adjusted people ──
    let mut adjusted = people.to_vec();

    for pid in 0..n_people {
        if is_excluded(&people[pid]) { continue; }

        let person = &people[pid];
        let hid = person.household_id;
        let bid = person.benunit_id;
        let bu = &benunits[bid];

        // Relative change in marginal net wage
        let w_base = baseline_retention[pid];
        let w_policy = policy_retention[pid];
        let dw_over_w = if w_base.abs() > 1e-9 {
            ((w_policy - w_base) / w_base).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // Relative change in household net income (static reform effect)
        let y_base = baseline_net[hid];
        let y_policy = unperturbed_policy_net[hid];
        let dy_over_y = if y_base.abs() > 1e-9 {
            ((y_policy - y_base) / y_base).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        let eta_s = substitution_elasticity(person, bu, people, ls);
        let eta_i = income_elasticity(person, bu, people, ls);

        let delta_e = person.employment_income * (eta_s * dw_over_w + eta_i * dy_over_y);

        // Employment income can't go below zero
        adjusted[pid].employment_income =
            (person.employment_income + delta_e).max(0.0);
    }

    adjusted
}
