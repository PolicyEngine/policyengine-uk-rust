use crate::engine::entities::{Household, Person};
use crate::parameters::{
    CouncilTaxParams, CapitalGainsTaxParams, LandTransactionTaxParams,
    StampDutyParams, WealthTaxParams,
};

/// Determine the council tax band (0=A .. 7=H) from a 1991 property value.
///
/// The WAS `main_residence_value` is in current prices, not 1991 values, so this
/// is an approximation. For baseline runs we use the reported FRS council_tax; this
/// function is used for reform modelling (e.g. changing Band D rate).
pub fn council_tax_band(property_value: f64, thresholds: &[f64]) -> usize {
    for (i, &t) in thresholds.iter().enumerate().rev() {
        if property_value >= t {
            return i;
        }
    }
    0
}

/// Calculate council tax for a household under a reform scenario.
///
/// Uses the nation-specific Band D override if present for the household's region,
/// scaling proportionally from the reported FRS amount (so we get the right
/// distributional spread without relying on inaccurate 1991 band thresholds).
/// Falls back to the reported `hh.council_tax` if no applicable override is set.
///
/// This means: setting `wales_average_band_d: 0.0` abolishes council tax only for
/// Welsh households; setting `average_band_d` to a new value rescales all others.
pub fn calculate_council_tax(hh: &Household, params: &CouncilTaxParams) -> f64 {
    // Pick the relevant Band D rate for this household's nation.
    let nation_band_d: Option<f64> = if hh.region.is_wales() {
        params.wales_average_band_d
    } else if hh.region.is_scotland() {
        params.scotland_average_band_d
    } else {
        None
    };

    let band_d = nation_band_d.unwrap_or(params.average_band_d);

    // Scale the reported FRS council tax by (reform_band_d / baseline_band_d).
    // This preserves the within-nation distributional spread while applying the
    // reform rate, rather than recalculating from inaccurate 1991 property bands.
    if params.average_band_d <= 0.0 || hh.council_tax <= 0.0 {
        // No baseline rate to scale from — use reported amount directly.
        return hh.council_tax;
    }
    hh.council_tax * (band_d / params.average_band_d)
}

/// Calculate capital gains tax for a person.
///
/// Uses the `capital_gains` field directly. Defaults to zero when no capital
/// gains data is available (FRS, WAS, SPI do not record realised gains).
/// The `is_higher_rate` flag should be true if the person's taxable income exceeds
/// the basic rate limit (i.e. they pay income tax at the higher/additional rate).
pub fn calculate_capital_gains_tax(
    person: &Person,
    params: &CapitalGainsTaxParams,
    is_higher_rate: bool,
) -> f64 {
    let taxable_gains = (person.capital_gains - params.annual_exempt_amount).max(0.0);

    if taxable_gains <= 0.0 {
        return 0.0;
    }

    let rate = if is_higher_rate { params.higher_rate } else { params.basic_rate };
    taxable_gains * rate
}

/// Calculate stamp duty land tax on a property value using marginal bands.
///
/// SDLT is a slab/marginal tax: each band's rate applies only to the portion of the
/// price within that band (not to the entire price).
fn marginal_sdlt(property_value: f64, bands: &[crate::parameters::StampDutyBand]) -> f64 {
    if bands.is_empty() || property_value <= 0.0 {
        return 0.0;
    }

    let mut tax = 0.0;
    for i in 0..bands.len() {
        let lower = bands[i].threshold;
        let upper = if i + 1 < bands.len() { bands[i + 1].threshold } else { f64::MAX };
        let rate = bands[i].rate;

        if property_value <= lower {
            break;
        }

        let taxable = property_value.min(upper) - lower;
        tax += taxable.max(0.0) * rate;
    }

    tax
}

/// Calculate annualised stamp duty (SDLT) for an English/NI household.
///
/// Multiplies the one-off SDLT liability by the annual purchase probability
/// (1 / average holding period) to get an expected annual amount.
pub fn calculate_stamp_duty(hh: &Household, params: &StampDutyParams) -> f64 {
    let sdlt = marginal_sdlt(hh.main_residence_value, &params.bands);
    sdlt * params.annual_purchase_probability
}

/// Calculate annualised Land Transaction Tax (LTT) for a Welsh household.
///
/// LTT replaced SDLT in Wales from 1 April 2018. Uses the same marginal-rate
/// calculation as SDLT but with Welsh Government bands and rates.
/// Source: Land Transaction Tax and Anti-avoidance of Devolved Taxes (Wales) Act 2017.
pub fn calculate_land_transaction_tax(hh: &Household, params: &LandTransactionTaxParams) -> f64 {
    let ltt = marginal_sdlt(hh.main_residence_value, &params.bands);
    ltt * params.annual_purchase_probability
}

/// Calculate the appropriate property transaction tax for a household,
/// routing to LTT (Wales) or SDLT (England/NI) based on region.
///
/// Scottish LBTT is also distinct; not yet modelled separately — falls through
/// to SDLT as a conservative approximation until LBTT params are added.
pub fn calculate_property_transaction_tax(
    hh: &Household,
    sdlt_params: Option<&StampDutyParams>,
    ltt_params: Option<&LandTransactionTaxParams>,
) -> f64 {
    if hh.region.is_wales() {
        if let Some(ltt) = ltt_params {
            return calculate_land_transaction_tax(hh, ltt);
        }
    }
    if let Some(sdlt) = sdlt_params {
        return calculate_stamp_duty(hh, sdlt);
    }
    0.0
}

/// Calculate annual wealth tax for a household.
///
/// Hypothetical flat-rate tax on net wealth above a threshold.
pub fn calculate_wealth_tax(hh: &Household, params: &WealthTaxParams) -> f64 {
    if !params.enabled {
        return 0.0;
    }

    let total_wealth = hh.property_wealth + hh.corporate_wealth + hh.gross_financial_wealth;
    let taxable = (total_wealth - params.threshold).max(0.0);
    taxable * params.rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::entities::{Household, Person};
    use crate::parameters::{
        CouncilTaxParams, CapitalGainsTaxParams, StampDutyParams, StampDutyBand, WealthTaxParams,
    };

    #[test]
    fn council_tax_band_lookup() {
        let thresholds = vec![0.0, 40001.0, 52001.0, 68001.0, 88001.0, 120001.0, 160001.0, 320001.0];
        assert_eq!(council_tax_band(30000.0, &thresholds), 0); // Band A
        assert_eq!(council_tax_band(50000.0, &thresholds), 1); // Band B
        assert_eq!(council_tax_band(100000.0, &thresholds), 4); // Band E
        assert_eq!(council_tax_band(500000.0, &thresholds), 7); // Band H
    }

    #[test]
    fn council_tax_calculation() {
        // With no nation-specific override, the reported CT is preserved 1:1
        // (reform rate = baseline rate → scaling factor = 1.0).
        let params = CouncilTaxParams {
            average_band_d: 2280.0,
            wales_average_band_d: None,
            scotland_average_band_d: None,
            band_multipliers: vec![6.0/9.0, 7.0/9.0, 8.0/9.0, 1.0, 11.0/9.0, 13.0/9.0, 15.0/9.0, 18.0/9.0],
            band_thresholds: vec![0.0, 40001.0, 52001.0, 68001.0, 88001.0, 120001.0, 160001.0, 320001.0],
        };
        let mut hh = Household::default();
        hh.council_tax = 2280.0;
        let ct = calculate_council_tax(&hh, &params);
        assert!((ct - 2280.0).abs() < 1.0); // no change when reform rate = baseline

        // Wales abolition: wales_average_band_d = 0 → Welsh CT goes to zero
        let params_wales_zero = CouncilTaxParams {
            wales_average_band_d: Some(0.0),
            ..params.clone()
        };
        let mut hh_wales = Household::default();
        hh_wales.region = crate::engine::entities::Region::Wales;
        hh_wales.council_tax = 1955.0;
        let ct_wales = calculate_council_tax(&hh_wales, &params_wales_zero);
        assert_eq!(ct_wales, 0.0); // abolished

        // England unaffected
        let ct_england = calculate_council_tax(&hh, &params_wales_zero);
        assert!((ct_england - 2280.0).abs() < 1.0);
    }

    #[test]
    fn cgt_basic_rate() {
        let params = CapitalGainsTaxParams {
            annual_exempt_amount: 3000.0,
            basic_rate: 0.18,
            higher_rate: 0.24,
        };
        let mut p = Person::default();
        p.capital_gains = 8000.0;
        // taxable = 8000 - 3000 = 5000; cgt = 5000 * 0.18 = 900
        let cgt = calculate_capital_gains_tax(&p, &params, false);
        assert!((cgt - 900.0).abs() < 0.01);
    }

    #[test]
    fn cgt_higher_rate() {
        let params = CapitalGainsTaxParams {
            annual_exempt_amount: 3000.0,
            basic_rate: 0.18,
            higher_rate: 0.24,
        };
        let mut p = Person::default();
        p.capital_gains = 8000.0;
        // taxable = 5000; cgt = 5000 * 0.24 = 1200
        let cgt = calculate_capital_gains_tax(&p, &params, true);
        assert!((cgt - 1200.0).abs() < 0.01);
    }

    #[test]
    fn cgt_below_exempt() {
        let params = CapitalGainsTaxParams {
            annual_exempt_amount: 3000.0,
            basic_rate: 0.18,
            higher_rate: 0.24,
        };
        let mut p = Person::default();
        p.capital_gains = 1000.0; // below AEA
        assert_eq!(calculate_capital_gains_tax(&p, &params, false), 0.0);
    }

    #[test]
    fn cgt_zero_by_default() {
        let params = CapitalGainsTaxParams {
            annual_exempt_amount: 3000.0,
            basic_rate: 0.18,
            higher_rate: 0.24,
        };
        // No capital_gains set — should produce zero (FRS/WAS default behaviour)
        let p = Person::default();
        assert_eq!(calculate_capital_gains_tax(&p, &params, false), 0.0);
    }

    #[test]
    fn stamp_duty_marginal() {
        let params = StampDutyParams {
            bands: vec![
                StampDutyBand { rate: 0.0, threshold: 0.0 },
                StampDutyBand { rate: 0.02, threshold: 125001.0 },
                StampDutyBand { rate: 0.05, threshold: 250001.0 },
                StampDutyBand { rate: 0.10, threshold: 925001.0 },
                StampDutyBand { rate: 0.12, threshold: 1500001.0 },
            ],
            annual_purchase_probability: 1.0, // set to 1 for testing one-off amount
        };
        let mut hh = Household::default();
        hh.main_residence_value = 500000.0;
        // 0% on first £125k, 2% on £125k-£250k = £2,500, 5% on £250k-£500k = £12,500
        // total = £15,000
        let sdlt = calculate_stamp_duty(&hh, &params);
        assert!((sdlt - 15000.0).abs() < 1.0);
    }

    #[test]
    fn wealth_tax_disabled() {
        let params = WealthTaxParams { enabled: false, threshold: 10_000_000.0, rate: 0.01 };
        let mut hh = Household::default();
        hh.property_wealth = 50_000_000.0;
        assert_eq!(calculate_wealth_tax(&hh, &params), 0.0);
    }

    #[test]
    fn wealth_tax_above_threshold() {
        let params = WealthTaxParams { enabled: true, threshold: 10_000_000.0, rate: 0.01 };
        let mut hh = Household::default();
        hh.property_wealth = 12_000_000.0;
        hh.corporate_wealth = 3_000_000.0;
        hh.gross_financial_wealth = 0.0;
        // total = 15m; taxable = 5m; tax = 50,000
        let tax = calculate_wealth_tax(&hh, &params);
        assert!((tax - 50_000.0).abs() < 0.01);
    }
}
