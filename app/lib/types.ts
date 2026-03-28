export interface BudgetaryImpact {
  baseline_revenue: number;
  reform_revenue: number;
  revenue_change: number;
  baseline_benefits: number;
  reform_benefits: number;
  benefit_spending_change: number;
  net_cost: number;
}

export interface DecileImpact {
  decile: number;
  avg_baseline_income: number;
  avg_reform_income: number;
  avg_change: number;
  pct_change: number;
}

export interface WinnersLosers {
  winners_pct: number;
  losers_pct: number;
  unchanged_pct: number;
  avg_gain: number;
  avg_loss: number;
}

export interface ProgramBreakdown {
  income_tax: number;
  employee_ni: number;
  employer_ni: number;
  universal_credit: number;
  child_benefit: number;
  state_pension: number;
  pension_credit: number;
  housing_benefit: number;
  child_tax_credit: number;
  working_tax_credit: number;
  income_support: number;
  esa_income_related: number;
  jsa_income_based: number;
  carers_allowance: number;
  scottish_child_payment: number;
  benefit_cap_reduction: number;
  passthrough_benefits: number;
}

export interface Caseloads {
  income_tax_payers: number;
  ni_payers: number;
  employer_ni_payers: number;
  universal_credit: number;
  child_benefit: number;
  state_pension: number;
  pension_credit: number;
  housing_benefit: number;
  child_tax_credit: number;
  working_tax_credit: number;
  income_support: number;
  esa_income_related: number;
  jsa_income_based: number;
  carers_allowance: number;
  scottish_child_payment: number;
  benefit_cap_affected: number;
}

export interface IncomeBreakdown {
  employment_income: number;
  self_employment_income: number;
  pension_income: number;
  savings_interest_income: number;
  dividend_income: number;
  property_income: number;
  other_income: number;
}

export interface SimulationResult {
  fiscal_year: string;
  budgetary_impact: BudgetaryImpact;
  income_breakdown: IncomeBreakdown;
  program_breakdown: ProgramBreakdown;
  caseloads: Caseloads;
  decile_impacts: DecileImpact[];
  winners_losers: WinnersLosers;
  avg_hbai_net_income: number;
  cpi_index: number;
}

export interface SliderConfig {
  key: string;
  label: string;
  section: string;
  path: string[];
  min: number;
  max: number;
  step: number;
  format: "currency" | "percent";
}
