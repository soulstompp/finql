use crate::currency::Currency;
use crate::data_handler::{DataError, QuoteHandler};
use crate::day_count_conv::DayCountConv;
use crate::fx_rates::get_fx_rate;
use crate::market::Market;
use crate::rates::{Compounding, DiscountError, Discounter, FlatRate};
use argmin::prelude::*;
use argmin::solver::brent::Brent;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::f64;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Neg;

/// Container for an amount of money in some currency
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq)]
pub struct CashAmount {
    pub amount: f64,
    pub currency: Currency,
}

pub fn round2digits(x: f64, digits: i32) -> f64 {
    (x * 10.0_f64.powi(digits)).round() / 10.0_f64.powi(digits)
}

impl CashAmount {
    pub fn add(
        &mut self,
        cash_amount: CashAmount,
        time: DateTime<Utc>,
        quotes: &mut dyn QuoteHandler,
        with_rounding: bool,
    ) -> Result<&mut Self, DataError> {
        if self.currency == cash_amount.currency {
            self.amount += cash_amount.amount;
            Ok(self)
        } else {
            let fx_rate = get_fx_rate(cash_amount.currency, self.currency, time, quotes)?;
            self.amount += fx_rate * cash_amount.amount;
            if with_rounding {
                let digits = quotes.get_rounding_digits(self.currency);
                self.amount = round2digits(self.amount, digits);
            }
            Ok(self)
        }
    }

    pub fn add_opt(
        &mut self,
        cash_amount: Option<CashAmount>,
        time: DateTime<Utc>,
        quotes: &mut dyn QuoteHandler,
        with_rounding: bool,
    ) -> Result<&mut Self, DataError> {
        match cash_amount {
            None => Ok(self),
            Some(cash_amount) => self.add(cash_amount, time, quotes, with_rounding),
        }
    }

    pub fn sub(
        &mut self,
        cash_amount: CashAmount,
        time: DateTime<Utc>,
        quotes: &mut dyn QuoteHandler,
        with_rounding: bool,
    ) -> Result<&mut Self, DataError> {
        if self.currency == cash_amount.currency {
            self.amount -= cash_amount.amount;
            Ok(self)
        } else {
            let fx_rate = get_fx_rate(cash_amount.currency, self.currency, time, quotes)?;
            self.amount -= fx_rate * cash_amount.amount;
            if with_rounding {
                let digits = quotes.get_rounding_digits(self.currency);
                self.amount = round2digits(self.amount, digits);
            }
            Ok(self)
        }
    }

    pub fn sub_opt(
        &mut self,
        cash_amount: Option<CashAmount>,
        time: DateTime<Utc>,
        quotes: &mut dyn QuoteHandler,
        with_rounding: bool,
    ) -> Result<&mut Self, DataError> {
        match cash_amount {
            None => Ok(self),
            Some(cash_amount) => self.sub(cash_amount, time, quotes, with_rounding),
        }
    }

    /// Round a cash amount to that number of decimals
    pub fn round(&self, digits: i32) -> CashAmount {
        CashAmount {
            amount: round2digits(self.amount, digits),
            currency: self.currency,
        }
    }

    /// Round Cash amount according to rounding conventions
    /// Lookup currency in rounding_conventions. If found, use the number of digits found for
    /// rounding to that number of decimals, otherwise round to two decimals.
    pub fn round_by_convention(&self, rounding_conventions: &BTreeMap<String, i32>) -> CashAmount {
        match rounding_conventions.get_key_value(&self.currency.to_string()) {
            Some((_, digits)) => self.round(*digits),
            None => self.round(2),
        }
    }
}

impl Display for CashAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:16.4} {}", self.amount, self.currency)
    }
}

impl Neg for CashAmount {
    type Output = CashAmount;

    fn neg(self) -> Self::Output {
        CashAmount {
            amount: -self.amount,
            currency: self.currency,
        }
    }
}

/// Container for a single cash flow
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct CashFlow {
    pub amount: CashAmount,
    pub date: NaiveDate,
}

impl CashFlow {
    /// Construct new cash flow
    pub fn new(amount: f64, currency: Currency, date: NaiveDate) -> CashFlow {
        CashFlow {
            amount: CashAmount { amount, currency },
            date,
        }
    }
    /// Check, whether cash flows could be aggregated
    pub fn aggregatable(&self, cf: &CashFlow) -> bool {
        if self.amount.currency != cf.amount.currency {
            false
        } else if self.date != cf.date {
            false
        } else {
            true
        }
    }

    /// Compare to cash flows for equality within a given absolute tolerance
    pub fn fuzzy_cash_flows_cmp_eq(&self, cf: &CashFlow, tol: f64) -> bool {
        if !self.aggregatable(cf) {
            false
        } else if self.amount.amount.is_nan()
            || cf.amount.amount.is_nan()
            || (self.amount.amount - cf.amount.amount).abs() > tol
        {
            false
        } else {
            true
        }
    }
}

impl Display for CashFlow {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.date, self.amount)
    }
}

impl Neg for CashFlow {
    type Output = CashFlow;

    fn neg(self) -> Self::Output {
        CashFlow {
            amount: -self.amount,
            date: self.date,
        }
    }
}

/// Get all future cash flows with respect to a given date
pub fn get_cash_flows_after(cash_flows: &Vec<CashFlow>, date: NaiveDate) -> Vec<CashFlow> {
    let mut new_cash_flows = Vec::new();
    for cf in cash_flows {
        if cf.date > date {
            new_cash_flows.push(cf.clone());
        }
    }
    new_cash_flows
}

pub trait FixedIncome {
    type Error: std::convert::From<DiscountError>;

    /// Transform product into series of cash flows
    fn rollout_cash_flows(
        &self,
        position: f64,
        market: &Market,
    ) -> Result<Vec<CashFlow>, Self::Error>;

    /// Calculate accrued interest for current coupon period
    fn accrued_interest(&self, today: NaiveDate) -> Result<f64, Self::Error>;

    /// Calculate the yield to maturity (YTM) given a purchase price and date
    fn calculate_ytm(
        &self,
        purchase_cash_flow: &CashFlow,
        market: &Market,
    ) -> Result<f64, Self::Error> {
        let cash_flows = self.rollout_cash_flows(1., market)?;
        let value = calculate_cash_flows_ytm(&cash_flows, &purchase_cash_flow)?;
        Ok(value)
    }
}

/// Calculate the internal rate of return of a stream of cash flows
/// The calculation assumes, that the notional payments and beginning and end are
/// included and calculates that annual rate, that gives total aggregate zero value
/// of all cash flows provided as `cash_flows`, if discounted to the payment date
/// of the first cash flow. It is assumed that all cash flow are in the same currency,
/// otherwise a `DiscountError` will be returned.
pub fn calculate_cash_flows_ytm(
    cash_flows: &Vec<CashFlow>,
    init_cash_flow: &CashFlow,
) -> Result<f64, DiscountError> {
    let rate = FlatRate::new(
        0.05,
        DayCountConv::Act365,
        Compounding::Annual,
        init_cash_flow.amount.currency,
    );
    let init_param = 0.5;
    let solver = Brent::new(0., 0.5, 1e-11);
    let func = FlatRateDiscounter {
        init_cash_flow: init_cash_flow,
        cash_flows: cash_flows,
        rate,
    };
    let res = Executor::new(func, solver, init_param).max_iters(100).run();
    match res {
        Ok(val) => Ok(val.state.get_param()),
        Err(_) => Err(DiscountError),
    }
}

/// Calculate discounted value for given flat rate
/// Since `argmin` requires `Serialize` and `Deserialize`,
/// we can't use reference here but must clone all data to this struct
#[derive(Clone)]
struct FlatRateDiscounter<'a> {
    init_cash_flow: &'a CashFlow,
    cash_flows: &'a Vec<CashFlow>,
    rate: FlatRate,
}

impl<'a> ArgminOp for FlatRateDiscounter<'a> {
    // one dimensional problem, no vector needed
    type Float = f64;
    type Param = f64;
    type Output = f64;
    type Hessian = ();
    type Jacobian = ();

    fn apply(&self, p: &Self::Param) -> Result<Self::Output, Error> {
        let mut discount_rate = self.rate.clone();
        discount_rate.rate = *p;
        let mut sum = self.init_cash_flow.amount.amount;
        let today = self.init_cash_flow.date;
        for cf in self.cash_flows.clone() {
            if cf.date > today {
                sum += discount_rate.discount_cash_flow(&cf, today)?.amount;
            }
        }
        Ok(sum)
    }
}

/// Dummy implementation of Serialize
impl<'a> Serialize for FlatRateDiscounter<'a> {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(format!(
            "serialization is disabled"
        )))
    }
}

/// Dummy implementation fo Deserialize
impl<'de> Deserialize<'de> for FlatRateDiscounter<'de> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(serde::de::Error::custom(format!(
            "deserialization is disabled"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx_rates::insert_fx_quote;
    use crate::sqlite_handler::SqliteDB;
    use chrono::{TimeZone, Utc};
    use std::str::FromStr;
    use rusqlite::Connection;

    #[test]
    fn yield_to_maturity() {
        let tol = 1e-11;
        let curr = Currency::from_str("EUR").unwrap();
        let cash_flows = vec![CashFlow::new(1050., curr, NaiveDate::from_ymd(2021, 10, 1))];
        let init_cash_flow = CashFlow::new(-1000., curr, NaiveDate::from_ymd(2020, 10, 1));

        let ytm = calculate_cash_flows_ytm(&cash_flows, &init_cash_flow).unwrap();
        assert_fuzzy_eq!(ytm, 0.05, tol);
    }

    #[test]
    fn cash_amount_arithmetic() {
        let tol = 1e-11;
        let time = Utc.ymd(2020, 4, 6).and_hms_milli(18, 0, 0, 0);

        let eur = Currency::from_str("EUR").unwrap();
        let jpy = Currency::from_str("JPY").unwrap();

        let fx_rate = 81.2345;
        // temporary storage for fx rates
        let mut conn = Connection::open(":memory:").unwrap();
        let mut fx_db = SqliteDB{ conn: &mut conn };
        fx_db.init().unwrap();
        insert_fx_quote(fx_rate, eur, jpy, time, &mut fx_db).unwrap();
        fx_db.set_rounding_digits(jpy, 0).unwrap();

        let eur_amount = CashAmount {
            amount: 100.0,
            currency: eur,
        };
        let jpy_amount = CashAmount {
            amount: 7500.0,
            currency: jpy,
        };
        let eur2_amount = CashAmount {
            amount: 200.0,
            currency: eur,
        };

        let mut tmp = CashAmount {
            amount: 0.0,
            currency: eur,
        };
        // Simple addition, same currency
        tmp.add(eur_amount, time, &mut fx_db, false).unwrap();
        assert_fuzzy_eq!(tmp.amount, 100.0, tol);
        // Adding optional cash amount
        tmp.add_opt(Some(eur2_amount), time, &mut fx_db, false)
            .unwrap();
        assert_fuzzy_eq!(tmp.amount, 300.0, tol);
        // Adding optional cash amount that is none
        tmp.add_opt(None, time, &mut fx_db, false).unwrap();
        assert_fuzzy_eq!(tmp.amount, 300.0, tol);
        // Adding optional foreign cash amount
        tmp.add_opt(Some(jpy_amount), time, &mut fx_db, false)
            .unwrap();
        assert_fuzzy_eq!(tmp.amount, 300.0 + 7500.0 / fx_rate, tol);
        // Substract foreign cash amount
        tmp.sub(jpy_amount, time, &mut fx_db, false).unwrap();
        assert_fuzzy_eq!(tmp.amount, 300.0, tol);
        // Substract optional None cash amount
        tmp.sub_opt(None, time, &mut fx_db, false).unwrap();
        assert_fuzzy_eq!(tmp.amount, 300.0, tol);
        // Substract optional cash amount, same currency
        tmp.sub_opt(Some(eur_amount), time, &mut fx_db, false)
            .unwrap();
        assert_fuzzy_eq!(tmp.amount, 200.0, tol);

        // Sum must be in EUR, since tmp was originally in EUR
        assert_eq!(tmp.currency.to_string(), "EUR");
        let mut curr_rounding_conventions = BTreeMap::new();
        curr_rounding_conventions.insert("JPY".to_string(), 0);

        let mut tmp = eur_amount;
        tmp.add(jpy_amount, time, &mut fx_db, false).unwrap();
        let tmp = tmp.round_by_convention(&curr_rounding_conventions);
        assert_fuzzy_eq!(
            tmp.amount,
            ((100.0 + 7500.0 / fx_rate) * 100.0_f64).round() / 100.0,
            tol
        );

        let mut tmp = jpy_amount;
        tmp.add(eur_amount, time, &mut fx_db, false).unwrap();
        // Sum must be in EUR, since tmp was originally in EUR
        assert_eq!(tmp.currency.to_string(), "JPY");
        assert_fuzzy_eq!(tmp.amount, 7500.0 + 100.0 * fx_rate, tol);
        let tmp = tmp.round_by_convention(&curr_rounding_conventions);
        assert_fuzzy_eq!(tmp.amount, (7500.0 + 100.0 * fx_rate).round(), tol);

        // With automatic rounding according to conventions
        let mut tmp = eur_amount;
        tmp.add(jpy_amount, time, &mut fx_db, true).unwrap();
        assert_fuzzy_eq!(
            tmp.amount,
            ((100.0 + 7500.0 / fx_rate) * 100.0_f64).round() / 100.0,
            tol
        );

        // With automatic rounding according to conventions
        let mut tmp = jpy_amount;
        tmp.add(eur_amount, time, &mut fx_db, true).unwrap();
        assert_fuzzy_eq!(tmp.amount, (7500.0 + 100.0 * fx_rate).round(), tol);
    }
}
