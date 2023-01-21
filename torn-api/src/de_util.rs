#![allow(unused)]

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::de::{Deserialize, Deserializer, Error, Unexpected, Visitor};

pub(crate) fn empty_string_is_none<'de, D>(deserializer: D) -> Result<Option<&'de str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

pub(crate) fn string_is_long<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        s.parse()
            .map(Some)
            .map_err(|_e| Error::invalid_type(Unexpected::Str(&s), &"i64"))
    }
}

pub(crate) fn zero_date_is_none<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
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

pub(crate) fn int_is_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let i = i64::deserialize(deserializer)?;

    match i {
        0 => Ok(false),
        1 => Ok(true),
        x => Err(Error::invalid_value(Unexpected::Signed(x), &"0 or 1")),
    }
}

pub(crate) fn empty_string_int_option<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DumbVisitor;

    impl<'de> Visitor<'de> for DumbVisitor {
        type Value = Option<i32>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "Empty string or integer")
        }

        // serde_json will treat all unsigned integers as u64
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Some(v as i32))
        }

        fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if v.is_empty() {
                Ok(None)
            } else {
                Err(E::invalid_value(Unexpected::Str(v), &self))
            }
        }
    }

    deserializer.deserialize_any(DumbVisitor)
}
