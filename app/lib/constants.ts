import { SliderConfig } from "./types";

export const YEARS = [2025, 2026, 2027, 2028, 2029];

export const SLIDERS: SliderConfig[] = [
  // Income Tax
  {
    key: "personal_allowance",
    label: "Personal Allowance",
    section: "Income Tax",
    path: ["income_tax", "personal_allowance"],
    min: 0,
    max: 25000,
    step: 100,
    format: "currency",
  },
  {
    key: "basic_rate",
    label: "Basic Rate",
    section: "Income Tax",
    path: ["income_tax", "uk_brackets", "0", "rate"],
    min: 0,
    max: 0.5,
    step: 0.01,
    format: "percent",
  },
  {
    key: "higher_rate",
    label: "Higher Rate",
    section: "Income Tax",
    path: ["income_tax", "uk_brackets", "1", "rate"],
    min: 0,
    max: 0.7,
    step: 0.01,
    format: "percent",
  },
  {
    key: "additional_rate",
    label: "Additional Rate",
    section: "Income Tax",
    path: ["income_tax", "uk_brackets", "2", "rate"],
    min: 0,
    max: 0.8,
    step: 0.01,
    format: "percent",
  },

  // National Insurance
  {
    key: "ni_main_rate",
    label: "Employee NI Rate",
    section: "National Insurance",
    path: ["national_insurance", "main_rate"],
    min: 0,
    max: 0.2,
    step: 0.005,
    format: "percent",
  },
  {
    key: "ni_employer_rate",
    label: "Employer NI Rate",
    section: "National Insurance",
    path: ["national_insurance", "employer_rate"],
    min: 0,
    max: 0.25,
    step: 0.005,
    format: "percent",
  },

  // Universal Credit
  {
    key: "uc_taper_rate",
    label: "UC Taper Rate",
    section: "Universal Credit",
    path: ["universal_credit", "taper_rate"],
    min: 0,
    max: 1.0,
    step: 0.01,
    format: "percent",
  },
  {
    key: "uc_standard_single_over25",
    label: "UC Standard (single 25+, monthly)",
    section: "Universal Credit",
    path: ["universal_credit", "standard_allowance_single_over25"],
    min: 0,
    max: 800,
    step: 10,
    format: "currency",
  },
  {
    key: "uc_work_allowance_lower",
    label: "UC Work Allowance (lower, monthly)",
    section: "Universal Credit",
    path: ["universal_credit", "work_allowance_lower"],
    min: 0,
    max: 1000,
    step: 10,
    format: "currency",
  },

  // Child Benefit
  {
    key: "cb_eldest_weekly",
    label: "Eldest Child (weekly)",
    section: "Child Benefit",
    path: ["child_benefit", "eldest_weekly"],
    min: 0,
    max: 50,
    step: 0.5,
    format: "currency",
  },
  {
    key: "hicbc_threshold",
    label: "HICBC Threshold",
    section: "Child Benefit",
    path: ["child_benefit", "hicbc_threshold"],
    min: 0,
    max: 150000,
    step: 1000,
    format: "currency",
  },
];

export const SECTIONS = [
  "Income Tax",
  "National Insurance",
  "Universal Credit",
  "Child Benefit",
];

export const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";
