use std::error;
use std::fmt;
use std::str::FromStr;

use async_trait::async_trait;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use chrono::{DateTime, Local};


/// Error type related to the Currency
#[derive(Debug, Clone, PartialEq)]
pub enum CurrencyError {
    InvalidLength,
    InvalidCharacter,
    DeserializationFailed,
    ConversionFailed,
}

impl fmt::Display for CurrencyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CurrencyError::InvalidLength => {
                write!(f, "currency codes must consist of exactly three characters")
            }
            CurrencyError::InvalidCharacter => write!(
                f,
                "currency codes must contain only alphabetic ASCII characters"
            ),
            CurrencyError::DeserializationFailed => write!(f, "currency deserialization failed"),
            CurrencyError::ConversionFailed => write!(f, "currency conversion failed"),
        }
    }
}

/// This is important for other errors to wrap this one.
impl error::Error for CurrencyError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl de::Error for CurrencyError {
    fn custom<T: fmt::Display>(_: T) -> Self {
        CurrencyError::DeserializationFailed
    }
}

/// Special type for currencies
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Currency {
    iso_code: [char; 3],
    rounding_digits: i32,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.iso_code[0], self.iso_code[1], self.iso_code[2]
        )
    }
}

fn default_rounding_digits(curr: &str) -> i32 {
    match curr {
        "JPY" | "TRL" => 0,
        _ => 2
    }
}

/// Transform a string into a Currency
impl FromStr for Currency {
    type Err = CurrencyError;

    fn from_str(curr: &str) -> Result<Currency, CurrencyError> {
        let rounding_digits = default_rounding_digits(curr);
        let mut iso_code = [' ', ' ', ' '];
        let mut idx = 0;
        for c in curr.chars() {
            if idx >= 3 {
                return Err(CurrencyError::InvalidLength);
            }
            let c = c.to_ascii_uppercase();
            if c.is_ascii_alphabetic() {
                iso_code[idx] = c.to_ascii_uppercase();
                idx += 1;
            } else {
                return Err(CurrencyError::InvalidCharacter);
            }
        }
        if idx != 3 {
            Err(CurrencyError::InvalidLength)
        } else {
            Ok(Currency{iso_code, rounding_digits} )
        }
    }
}

impl Serialize for Currency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", &self))
    }
}

struct CurrencyVisitor;

impl<'de> Visitor<'de> for CurrencyVisitor {
    type Value = Currency;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a currency code must consist of three alphabetic characters")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Currency::from_str(value) {
            Ok(val) => Ok(val),
            Err(err) => Err(E::custom(format!("{}", err))),
        }
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D>(deserializer: D) -> Result<Currency, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CurrencyVisitor)
    }
}

impl Currency {
    pub fn rounding_digits(&self) -> i32 {
        self.rounding_digits
    }
}

/// Trait for calculating FX rates for currency conversion
#[async_trait]
pub trait CurrencyConverter {
    /// returns the price of 1 unit of foreign currency in terms of domestic currency
    async fn fx_rate(&self, foreign_currency: Currency, domestic_currency: Currency, time: DateTime<Local>) -> Result<f64, CurrencyError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_currency() {
        // valid iso code
        let currency = Currency::from_str("EUR").unwrap();
        assert_eq!(format!("{}", currency), "EUR".to_string());

        // case ignorance
        let currency = Currency::from_str("euR").unwrap();
        assert_eq!(format!("{}", currency), "EUR".to_string());

        // to long
        let currency = Currency::from_str("EURO");
        assert_eq!(currency, Err(CurrencyError::InvalidLength));

        // to short
        let currency = Currency::from_str("EU");
        assert_eq!(currency, Err(CurrencyError::InvalidLength));

        // invalid character1
        let currency = Currency::from_str("éUR");
        assert_eq!(currency, Err(CurrencyError::InvalidCharacter));

        // invalid character2
        let currency = Currency::from_str("EU1");
        assert_eq!(currency, Err(CurrencyError::InvalidCharacter));
    }

    #[test]
    fn deserialize_currency() {
        let input = r#""EUR""#;

        let curr: Currency = serde_json::from_str(input).unwrap();
        assert_eq!(format!("{}", curr), "EUR");
    }
    #[test]
    fn serialize_currency() {
        let curr = Currency {
            iso_code: ['E', 'U', 'R'], rounding_digits: 2
        };
        let json = serde_json::to_string(&curr).unwrap();
        assert_eq!(json, r#""EUR""#);
    }
}
