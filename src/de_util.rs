use chrono::{DateTime, NaiveDateTime, Utc};
use num_traits::{PrimInt, Zero};
use serde::de::{Deserialize, Deserializer, Error, Unexpected};

pub fn empty_string_is_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

pub fn string_is_long<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse()
        .map_err(|_e| Error::invalid_type(Unexpected::Str(&s), &"i64"))
}

pub fn zero_date_is_none<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let i = i64::deserialize(deserializer)?;
    if i == 0 {
        Ok(None)
    } else {
        let naive = NaiveDateTime::from_timestamp(i, 0);
        Ok(Some(DateTime::from_utc(naive, Utc)))
    }
}

pub fn zero_is_none<'de, D, I>(deserializer: D) -> Result<Option<I>, D::Error>
where
    D: Deserializer<'de>,
    I: PrimInt + Zero + Deserialize<'de>,
{
    let i = I::deserialize(deserializer)?;
    if i == I::zero() {
        Ok(None)
    } else {
        Ok(Some(i))
    }
}

pub fn none_is_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "None" {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}
