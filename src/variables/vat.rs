use crate::engine::entities::Household;
use crate::parameters::Parameters;

/// VAT paid by a household, computed from COICOP consumption categories.
///
/// If EFRS consumption data is available (non-zero), applies category-specific
/// VAT rates. Otherwise estimates from disposable income using ONS average
/// propensity to consume and standard/reduced/zero shares.
///
/// VAT is calculated as the tax-inclusive amount: if goods cost £120 inclusive
/// of 20% VAT, the VAT paid is £120 × 20/120 = £20. This is the tax fraction
/// method: rate / (1 + rate).
pub fn calculate_household_vat(
    hh: &Household,
    disposable_income: f64,
    params: &Parameters,
) -> f64 {
    let vat = match &params.vat {
        Some(v) => v,
        None => return 0.0,
    };

    let std_rate = vat.standard_rate;
    let red_rate = vat.reduced_rate;
    // zero_rate is 0.0 by definition but included for clarity

    // Tax-inclusive VAT fraction: rate / (1 + rate)
    let std_fraction = std_rate / (1.0 + std_rate);
    let red_fraction = red_rate / (1.0 + red_rate);

    // Check if we have EFRS consumption data (any non-zero consumption field)
    let total_consumption = hh.food_consumption
        + hh.alcohol_tobacco_consumption
        + hh.clothing_consumption
        + hh.furnishings_consumption
        + hh.health_consumption
        + hh.transport_consumption
        + hh.communication_consumption
        + hh.recreation_consumption
        + hh.education_consumption
        + hh.restaurants_consumption
        + hh.miscellaneous_consumption
        + hh.petrol_spending
        + hh.diesel_spending
        + hh.domestic_energy_consumption;

    if total_consumption > 100.0 {
        // EFRS data available — use category-specific rates
        // Zero-rated: food, education
        // Reduced rate (5%): domestic energy (electricity + gas)
        // Standard rate (20%): everything else
        let zero_rated = hh.food_consumption + hh.education_consumption;
        let reduced_rated = hh.electricity_consumption + hh.gas_consumption;
        let standard_rated = total_consumption - zero_rated - reduced_rated;

        let vat_on_standard = standard_rated.max(0.0) * std_fraction;
        let vat_on_reduced = reduced_rated.max(0.0) * red_fraction;
        // zero-rated contributes £0

        vat_on_standard + vat_on_reduced
    } else {
        // No EFRS data — estimate consumption from disposable income.
        // ONS Family Spending 2023: average propensity to consume varies by income.
        // Low income (~£15k): ~95% consumed. High income (~£100k): ~65% consumed.
        // Use a simple logistic-style curve.
        let income = disposable_income.max(0.0);
        let propensity = 0.65 + 0.30 / (1.0 + (income / 25000.0));
        let estimated_consumption = income * propensity;

        let std_share = vat.standard_rated_share;
        let red_share = vat.reduced_rated_share;
        // Rest is zero-rated (no VAT)

        let vat_on_standard = estimated_consumption * std_share * std_fraction;
        let vat_on_reduced = estimated_consumption * red_share * red_fraction;

        vat_on_standard + vat_on_reduced
    }
}
