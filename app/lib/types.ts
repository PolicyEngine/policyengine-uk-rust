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

export interface SimulationResult {
  fiscal_year: string;
  budgetary_impact: BudgetaryImpact;
  decile_impacts: DecileImpact[];
  winners_losers: WinnersLosers;
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
