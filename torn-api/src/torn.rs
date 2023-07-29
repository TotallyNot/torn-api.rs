use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize,
};

use torn_api_macros::ApiCategory;

use crate::user;

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "torn")]
pub enum Selection {
    #[api(
        field = "competition",
        with = "decode_competition",
        type = "Option<Competition>"
    )]
    Competition,

    #[api(type = "HashMap<String, TerritoryWar>", field = "territorywars")]
    TerritoryWars,

    #[api(type = "HashMap<String, Racket>", field = "rackets")]
    Rackets,

    #[api(type = "HashMap<String, Territory>", field = "territory")]
    Territory,
}

#[derive(Deserialize)]
pub struct EliminationLeaderboard {
    pub position: i16,
    pub team: user::EliminationTeam,
    pub score: i16,
    pub lives: i16,
    pub participants: i16,
    pub wins: i32,
    pub losses: i32,
}

pub enum Competition {
    Elimination { teams: Vec<EliminationLeaderboard> },
    Unkown(String),
}

fn decode_competition<'de, D>(deserializer: D) -> Result<Option<Competition>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct CompetitionVisitor;

    impl<'de> Visitor<'de> for CompetitionVisitor {
        type Value = Option<Competition>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("struct Competition")
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_map(self)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut name = None;
            let mut teams = None;

            while let Some(key) = map.next_key()? {
                match key {
                    "name" => {
                        name = Some(map.next_value()?);
                    }
                    "teams" => {
                        teams = Some(map.next_value()?);
                    }
                    _ => (),
                };
            }

            let name = name.ok_or_else(|| de::Error::missing_field("name"))?;

            match name {
                "Elimination" => Ok(Some(Competition::Elimination {
                    teams: teams.ok_or_else(|| de::Error::missing_field("teams"))?,
                })),
                "" => Ok(None),
                v => Ok(Some(Competition::Unkown(v.to_owned()))),
            }
        }
    }

    deserializer.deserialize_option(CompetitionVisitor)
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerritoryWar {
    pub assaulting_faction: i32,
    pub defending_faction: i32,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub started: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub ends: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Racket {
    pub name: String,
    pub level: i16,
    pub reward: String,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub created: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub changed: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Territory {
    pub sector: i16,
    pub size: i16,
    pub slots: i16,
    pub daily_respect: i16,
    pub faction: i32,

    pub neighbors: Vec<String>,
    pub war: Option<TerritoryWar>,
    pub racket: Option<Racket>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{async_test, setup, Client, ClientTrait};

    #[async_test]
    async fn competition() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .torn(|b| {
                b.selections(&[
                    Selection::Competition,
                    Selection::TerritoryWars,
                    Selection::Rackets,
                ])
            })
            .await
            .unwrap();

        response.competition().unwrap();
        response.territory_wars().unwrap();
        response.rackets().unwrap();
    }

    #[async_test]
    async fn territory() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .torn(|b| b.selections(&[Selection::Territory]).id("NSC"))
            .await
            .unwrap();

        let territory = response.territory().unwrap();
        assert!(territory.contains_key("NSC"));
    }
}
