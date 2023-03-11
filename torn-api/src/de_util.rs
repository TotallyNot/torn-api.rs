#![allow(unused)]

use std::collections::BTreeMap;

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
        let naive = NaiveDateTime::from_timestamp_opt(i, 0)
            .ok_or_else(|| D::Error::invalid_value(Unexpected::Signed(i), &"Epoch timestamp"))?;
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

pub(crate) fn datetime_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<i32, chrono::DateTime<chrono::Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    struct UnixTimestamp(
        #[serde(with = "chrono::serde::ts_seconds")] chrono::DateTime<chrono::Utc>,
    );

    struct MapVisitor;

    impl<'de> Visitor<'de> for MapVisitor {
        type Value = BTreeMap<i32, chrono::DateTime<chrono::Utc>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "map of unix timestamps")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some(key) = map.next_key::<&'de str>()? {
                let id = key
                    .parse()
                    .map_err(|_e| A::Error::invalid_value(Unexpected::Str(key), &"integer"))?;

                let ts: UnixTimestamp = map.next_value()?;
                result.insert(id, ts.0);
            }

            Ok(result)
        }
    }

    deserializer.deserialize_map(MapVisitor)
}

pub(crate) fn empty_dict_is_empty_array<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct ArrayVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> Visitor<'de> for ArrayVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "vec or empty object")
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            match map.size_hint() {
                Some(0) | None => Ok(Vec::default()),
                Some(len) => Err(A::Error::invalid_length(len, &"empty dict")),
            }
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut result = match seq.size_hint() {
                Some(len) => Vec::with_capacity(len),
                None => Vec::default(),
            };

            while let Some(element) = seq.next_element()? {
                result.push(element);
            }

            Ok(result)
        }
    }

    deserializer.deserialize_any(ArrayVisitor(std::marker::PhantomData))
}

#[cfg(feature = "decimal")]
pub(crate) fn string_or_decimal<'de, D>(deserializer: D) -> Result<rust_decimal::Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    struct DumbVisitor;

    impl<'de> Visitor<'de> for DumbVisitor {
        type Value = rust_decimal::Decimal;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "integer or float as string")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v.into())
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v.into())
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            rust_decimal::Decimal::from_str_exact(v).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(DumbVisitor)
}
