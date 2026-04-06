use crate::engine::entities::Household;
use crate::parameters::{FuelDutyParams, AlcoholDutyParams, TobaccoDutyParams};

/// Calculate fuel duty for a household from petrol and diesel spending.
///
/// Fuel duty is per-litre, so we convert £ spending to litres using average pump prices,
/// then apply the duty rate. Fuel duty is already included in the pump price, so we
/// extract the duty component: litres = spending / price_per_litre; duty = litres * rate.
pub fn calculate_fuel_duty(hh: &Household, params: &FuelDutyParams) -> f64 {
    let petrol_litres = if params.average_petrol_price_per_litre > 0.0 {
        hh.petrol_spending / params.average_petrol_price_per_litre
    } else {
        0.0
    };
    let diesel_litres = if params.average_diesel_price_per_litre > 0.0 {
        hh.diesel_spending / params.average_diesel_price_per_litre
    } else {
        0.0
    };

    petrol_litres * params.petrol_rate_per_litre
        + diesel_litres * params.diesel_rate_per_litre
}

/// Calculate alcohol duty for a household from alcohol spending.
///
/// Uses a simplified effective-rate approach: duty = spending * rate / (1 + rate).
/// This treats the spending as tax-inclusive (i.e. the household paid price includes duty).
pub fn calculate_alcohol_duty(hh: &Household, params: &AlcoholDutyParams) -> f64 {
    if params.effective_rate <= 0.0 || hh.alcohol_consumption <= 0.0 {
        return 0.0;
    }
    hh.alcohol_consumption * params.effective_rate / (1.0 + params.effective_rate)
}

/// Calculate tobacco duty for a household from tobacco spending.
///
/// Same effective-rate approach as alcohol duty.
pub fn calculate_tobacco_duty(hh: &Household, params: &TobaccoDutyParams) -> f64 {
    if params.effective_rate <= 0.0 || hh.tobacco_consumption <= 0.0 {
        return 0.0;
    }
    hh.tobacco_consumption * params.effective_rate / (1.0 + params.effective_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::entities::Household;
    use crate::parameters::{FuelDutyParams, AlcoholDutyParams, TobaccoDutyParams};

    fn hh_with_fuel(petrol: f64, diesel: f64) -> Household {
        let mut hh = Household::default();
        hh.petrol_spending = petrol;
        hh.diesel_spending = diesel;
        hh
    }

    #[test]
    fn fuel_duty_basic() {
        let params = FuelDutyParams {
            petrol_rate_per_litre: 0.5295,
            diesel_rate_per_litre: 0.5295,
            average_petrol_price_per_litre: 1.35,
            average_diesel_price_per_litre: 1.40,
        };
        // £1350 petrol spend = 1000 litres; duty = 1000 * 0.5295 = £529.50
        let hh = hh_with_fuel(1350.0, 0.0);
        let duty = calculate_fuel_duty(&hh, &params);
        assert!((duty - 529.50).abs() < 0.01);
    }

    #[test]
    fn alcohol_duty_basic() {
        let params = AlcoholDutyParams { effective_rate: 0.40 };
        let mut hh = Household::default();
        hh.alcohol_consumption = 1400.0; // £1400/yr tax-inclusive
        // duty = 1400 * 0.40 / 1.40 = £400
        let duty = calculate_alcohol_duty(&hh, &params);
        assert!((duty - 400.0).abs() < 0.01);
    }

    #[test]
    fn tobacco_duty_basic() {
        let params = TobaccoDutyParams { effective_rate: 0.72 };
        let mut hh = Household::default();
        hh.tobacco_consumption = 1720.0; // £1720/yr tax-inclusive
        // duty = 1720 * 0.72 / 1.72 = £720
        let duty = calculate_tobacco_duty(&hh, &params);
        assert!((duty - 720.0).abs() < 0.01);
    }

    #[test]
    fn zero_spending_returns_zero() {
        let fuel = FuelDutyParams {
            petrol_rate_per_litre: 0.5295,
            diesel_rate_per_litre: 0.5295,
            average_petrol_price_per_litre: 1.35,
            average_diesel_price_per_litre: 1.40,
        };
        let alc = AlcoholDutyParams { effective_rate: 0.40 };
        let tab = TobaccoDutyParams { effective_rate: 0.72 };
        let hh = Household::default();
        assert_eq!(calculate_fuel_duty(&hh, &fuel), 0.0);
        assert_eq!(calculate_alcohol_duty(&hh, &alc), 0.0);
        assert_eq!(calculate_tobacco_duty(&hh, &tab), 0.0);
    }
}
