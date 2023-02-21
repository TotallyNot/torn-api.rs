use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::Deserialize;

use crate::de_util;

#[derive(Debug, Clone, Deserialize)]
pub struct LastAction {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum State {
    Okay,
    Traveling,
    Hospital,
    Abroad,
    Jail,
    Federal,
    Fallen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateColour {
    Green,
    Red,
    Blue,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Status<'a> {
    pub description: &'a str,
    #[serde(deserialize_with = "de_util::empty_string_is_none")]
    pub details: Option<&'a str>,
    #[serde(rename = "color")]
    pub colour: StateColour,
    pub state: State,
    #[serde(deserialize_with = "de_util::zero_date_is_none")]
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Territory {
    pub sector: i16,
    pub size: i16,
    pub density: i16,
    pub daily_respect: i16,
    pub faction: i32,

    #[cfg(feature = "decimal")]
    #[serde(deserialize_with = "de_util::string_or_decimal")]
    pub coordinate_x: rust_decimal::Decimal,

    #[cfg(feature = "decimal")]
    #[serde(deserialize_with = "de_util::string_or_decimal")]
    pub coordinate_y: rust_decimal::Decimal,
}
