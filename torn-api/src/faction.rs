use std::collections::BTreeMap;

use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::Deserialize;

use torn_api_macros::ApiCategory;

use crate::de_util;

pub use crate::common::{LastAction, Status};

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "faction")]
pub enum Selection {
    #[api(type = "Basic", flatten)]
    Basic,

    #[api(type = "BTreeMap<i32, Attack>", field = "attacks")]
    AttacksFull,

    #[api(type = "BTreeMap<i32, AttackFull>", field = "attacks")]
    Attacks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Member<'a> {
    pub name: &'a str,
    pub level: i16,
    pub days_in_faction: i16,
    pub position: &'a str,
    pub status: Status<'a>,
    pub last_action: LastAction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Basic<'a> {
    #[serde(rename = "ID")]
    pub id: i32,
    pub name: &'a str,
    pub leader: i32,

    pub respect: i32,
    pub age: i16,
    pub capacity: i16,
    pub best_chain: i32,

    #[serde(borrow)]
    pub members: BTreeMap<i32, Member<'a>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{async_test, setup, Client, ClientTrait};

    #[async_test]
    async fn faction() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .faction(|b| b.selections(&[Selection::Basic, Selection::Attacks]))
            .await
            .unwrap();

        response.basic().unwrap();
        response.attacks().unwrap();
        response.attacks_full().unwrap();
    }
}
