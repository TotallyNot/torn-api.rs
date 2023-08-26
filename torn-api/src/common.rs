use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::Deserialize;

use crate::de_util;

#[derive(Debug, Clone, Deserialize)]
pub enum OnlineStatus {
    Online,
    Offline,
    Idle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastAction {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub status: OnlineStatus,
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

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum AttackResult {
    Attacked,
    Mugged,
    Hospitalized,
    Lost,
    Arrested,
    Escape,
    Interrupted,
    Assist,
    Timeout,
    Stalemate,
    Special,
    Looted,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Attack<'a> {
    pub code: &'a str,
    #[serde(with = "ts_seconds")]
    pub timestamp_started: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timestamp_ended: DateTime<Utc>,

    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub attacker_id: Option<i32>,
    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub attacker_faction: Option<i32>,
    pub defender_id: i32,
    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub defender_faction: Option<i32>,
    pub result: AttackResult,

    #[serde(deserialize_with = "de_util::int_is_bool")]
    pub stealthed: bool,

    #[cfg(feature = "decimal")]
    pub respect: rust_decimal::Decimal,

    #[cfg(not(feature = "decimal"))]
    pub respect: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RespectModifiers {
    pub fair_fight: f32,
    pub war: f32,
    pub retaliation: f32,
    pub group_attack: f32,
    pub overseas: f32,
    pub chain_bonus: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttackFull<'a> {
    pub code: &'a str,
    #[serde(with = "ts_seconds")]
    pub timestamp_started: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timestamp_ended: DateTime<Utc>,

    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub attacker_id: Option<i32>,
    #[serde(deserialize_with = "de_util::empty_string_is_none")]
    pub attacker_name: Option<&'a str>,
    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub attacker_faction: Option<i32>,
    #[serde(
        deserialize_with = "de_util::empty_string_is_none",
        rename = "attacker_factionname"
    )]
    pub attacker_faction_name: Option<&'a str>,

    pub defender_id: i32,
    pub defender_name: &'a str,
    #[serde(deserialize_with = "de_util::empty_string_int_option")]
    pub defender_faction: Option<i32>,
    #[serde(
        deserialize_with = "de_util::empty_string_is_none",
        rename = "defender_factionname"
    )]
    pub defender_faction_name: Option<&'a str>,

    pub result: AttackResult,

    #[serde(deserialize_with = "de_util::int_is_bool")]
    pub stealthed: bool,
    #[serde(deserialize_with = "de_util::int_is_bool")]
    pub raid: bool,
    #[serde(deserialize_with = "de_util::int_is_bool")]
    pub ranked_war: bool,

    #[cfg(feature = "decimal")]
    pub respect: rust_decimal::Decimal,
    #[cfg(feature = "decimal")]
    pub respect_loss: rust_decimal::Decimal,

    #[cfg(not(feature = "decimal"))]
    pub respect: f32,
    #[cfg(not(feature = "decimal"))]
    pub respect_loss: f32,

    pub modifiers: RespectModifiers,
}
