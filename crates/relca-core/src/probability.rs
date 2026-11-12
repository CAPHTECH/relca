//! Shared exact numeric types, not execution or validation algorithms.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rational(pub BigRational);
impl Rational {
    pub fn zero() -> Self {
        Self(BigRational::zero())
    }
    pub fn one() -> Self {
        Self(BigRational::one())
    }
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    pub fn valid(&self) -> bool {
        self >= &Self::zero() && self <= &Self::one()
    }
    pub fn within_bits(&self, bits: u64) -> bool {
        self.0.denom().bits() <= bits
    }
    pub fn decimal(text: &str) -> Option<Self> {
        let negative = text.starts_with('-');
        let text = text.strip_prefix('-').unwrap_or(text);
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        if whole.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let mut n = BigInt::parse_bytes(format!("{whole}{fraction}").as_bytes(), 10)?;
        if negative {
            n = -n;
        }
        Some(Self(BigRational::new(
            n,
            BigInt::from(10u8).pow(fraction.len().try_into().ok()?),
        )))
    }
}
impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Serialize for Rational {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Rational {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let parsed = (|| {
            let (n, q) = text.split_once('/').unwrap_or((&text, "1"));
            if n.is_empty()
                || q.is_empty()
                || !n.bytes().chain(q.bytes()).all(|b| b.is_ascii_digit())
            {
                return None;
            }
            let n = BigInt::parse_bytes(n.as_bytes(), 10)?;
            let q = BigInt::parse_bytes(q.as_bytes(), 10)?;
            if q.is_zero() {
                return None;
            }
            let p = Self(BigRational::new(n, q));
            (p.valid() && p.to_string() == text).then_some(p)
        })();
        parsed.ok_or_else(|| D::Error::custom("expected canonical probability fraction in 0..1"))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbabilityRange {
    pub min: Rational,
    pub max: Rational,
}
impl<'de> Deserialize<'de> for ProbabilityRange {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            min: Rational,
            max: Rational,
        }
        let f = Fields::deserialize(d)?;
        if f.min > f.max {
            return Err(D::Error::custom("reversed probability range"));
        }
        Ok(Self {
            min: f.min,
            max: f.max,
        })
    }
}
