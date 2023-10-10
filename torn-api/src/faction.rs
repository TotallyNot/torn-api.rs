use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use torn_api_macros::{ApiCategory, IntoOwned};

use crate::de_util::{self, null_is_empty_dict};

pub use crate::common::{Attack, AttackFull, LastAction, Status, Territory};

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "faction")]
#[non_exhaustive]
pub enum FactionSelection {
    #[api(type = "Basic", flatten)]
    Basic,

    #[api(type = "BTreeMap<i32, Attack>", field = "attacks")]
    AttacksFull,

    #[api(type = "BTreeMap<i32, AttackFull>", field = "attacks")]
    Attacks,

    #[api(
        type = "HashMap<String, Territory>",
        field = "territory",
        with = "null_is_empty_dict"
    )]
    Territory,
}

pub type Selection = FactionSelection;

#[derive(Debug, IntoOwned, Deserialize)]
pub struct Member<'a> {
    pub name: &'a str,
    pub level: i16,
    pub days_in_faction: i16,
    pub position: &'a str,
    pub status: Status<'a>,
    pub last_action: LastAction,
}

#[derive(Debug, IntoOwned, Deserialize)]
pub struct FactionTerritoryWar<'a> {
    pub territory_war_id: i32,
    pub territory: &'a str,
    pub assaulting_faction: i32,
    pub defending_faction: i32,
    pub score: i32,
    pub required_score: i32,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub start_time: DateTime<Utc>,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub end_time: DateTime<Utc>,
}

#[derive(Debug, IntoOwned, Deserialize)]
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

    #[serde(deserialize_with = "de_util::datetime_map")]
    pub peace: BTreeMap<i32, DateTime<Utc>>,

    #[serde(borrow, deserialize_with = "de_util::empty_dict_is_empty_array")]
    pub territory_wars: Vec<FactionTerritoryWar<'a>>,
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
            .faction(|b| {
                b.selections(&[Selection::Basic, Selection::Attacks, Selection::Territory])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.attacks().unwrap();
        response.attacks_full().unwrap();
        response.territory().unwrap();
    }

    #[async_test]
    async fn faction_public() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .faction(|b| {
                b.id(7049)
                    .selections(&[Selection::Basic, Selection::Territory])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.territory().unwrap();
    }

    #[async_test]
    async fn destroyed_faction() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .faction(|b| {
                b.id(8981)
                    .selections(&[Selection::Basic, Selection::Territory])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.territory().unwrap();
    }
}
