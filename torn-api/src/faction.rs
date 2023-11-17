use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, TimeZone, Utc};
use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer,
};

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

    #[api(type = "Option<Chain>", field = "chain", with = "deserialize_chain")]
    Chain,
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

#[derive(Debug)]
pub struct Chain {
    pub current: i32,
    pub max: i32,
    #[cfg(feature = "decimal")]
    pub modifier: rust_decimal::Decimal,
    pub timeout: Option<i32>,
    pub cooldown: Option<i32>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

fn deserialize_chain<'de, D>(deserializer: D) -> Result<Option<Chain>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ChainVisitor;

    impl<'de> Visitor<'de> for ChainVisitor {
        type Value = Option<Chain>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("struct Chain")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum Fields {
                Current,
                Max,
                Modifier,
                Timeout,
                Cooldown,
                Start,
                End,
                #[serde(other)]
                Ignore,
            }

            let mut current = None;
            let mut max = None;
            #[cfg(feature = "decimal")]
            let mut modifier = None;
            let mut timeout = None;
            let mut cooldown = None;
            let mut start = None;
            let mut end = None;

            while let Some(key) = map.next_key()? {
                match key {
                    Fields::Current => {
                        let value = map.next_value()?;
                        if value != 0 {
                            current = Some(value);
                        }
                    }
                    Fields::Max => {
                        max = Some(map.next_value()?);
                    }
                    Fields::Modifier => {
                        #[cfg(feature = "decimal")]
                        {
                            modifier = Some(map.next_value()?);
                        }
                    }
                    Fields::Timeout => {
                        match map.next_value()? {
                            0 => timeout = Some(None),
                            val => timeout = Some(Some(val)),
                        };
                    }
                    Fields::Cooldown => {
                        match map.next_value()? {
                            0 => cooldown = Some(None),
                            val => cooldown = Some(Some(val)),
                        };
                    }
                    Fields::Start => {
                        let ts: i64 = map.next_value()?;
                        start = Some(Utc.timestamp_opt(ts, 0).single().ok_or_else(|| {
                            A::Error::invalid_value(Unexpected::Signed(ts), &"Epoch timestamp")
                        })?);
                    }
                    Fields::End => {
                        let ts: i64 = map.next_value()?;
                        end = Some(Utc.timestamp_opt(ts, 0).single().ok_or_else(|| {
                            A::Error::invalid_value(Unexpected::Signed(ts), &"Epoch timestamp")
                        })?);
                    }
                    Fields::Ignore => (),
                }
            }

            let Some(current) = current else {
                return Ok(None);
            };
            let max = max.ok_or_else(|| A::Error::missing_field("max"))?;
            let timeout = timeout.ok_or_else(|| A::Error::missing_field("timeout"))?;
            let cooldown = cooldown.ok_or_else(|| A::Error::missing_field("cooldown"))?;
            let start = start.ok_or_else(|| A::Error::missing_field("start"))?;
            let end = end.ok_or_else(|| A::Error::missing_field("end"))?;

            Ok(Some(Chain {
                current,
                max,
                #[cfg(feature = "decimal")]
                modifier: modifier.ok_or_else(|| A::Error::missing_field("modifier"))?,
                timeout,
                cooldown,
                start,
                end,
            }))
        }
    }

    deserializer.deserialize_map(ChainVisitor)
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
                b.selections([
                    Selection::Basic,
                    Selection::Attacks,
                    Selection::Territory,
                    Selection::Chain,
                ])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.attacks().unwrap();
        response.attacks_full().unwrap();
        response.territory().unwrap();
        response.chain().unwrap();
    }

    #[async_test]
    async fn faction_public() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .faction(|b| {
                b.id(7049)
                    .selections([Selection::Basic, Selection::Territory, Selection::Chain])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.territory().unwrap();
        response.chain().unwrap();
    }

    #[async_test]
    async fn destroyed_faction() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .faction(|b| {
                b.id(8981)
                    .selections([Selection::Basic, Selection::Territory, Selection::Chain])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.territory().unwrap();
        assert!(response.chain().unwrap().is_none());
    }
}
