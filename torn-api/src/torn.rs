use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};

use torn_api_macros::ApiCategory;

use crate::{de_util, user};

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "torn")]
#[non_exhaustive]
pub enum TornSelection {
    #[api(
        field = "competition",
        with = "decode_competition",
        type = "Option<Competition>"
    )]
    Competition,

    #[api(
        type = "HashMap<String, TerritoryWar>",
        with = "decode_territory_wars",
        field = "territorywars"
    )]
    TerritoryWars,

    #[api(type = "HashMap<String, Racket>", field = "rackets")]
    Rackets,

    #[api(
        type = "HashMap<String, Territory>",
        with = "decode_territory",
        field = "territory"
    )]
    Territory,

    #[api(type = "TerritoryWarReport", field = "territorywarreport")]
    TerritoryWarReport,

    #[api(type = "BTreeMap<i32, Item>", field = "items")]
    Items,
}

pub type Selection = TornSelection;

#[derive(Debug, Clone, Deserialize)]
pub struct EliminationLeaderboard {
    pub position: i16,
    pub team: user::EliminationTeam,
    pub score: i16,
    pub lives: i16,
    pub participants: Option<i16>,
    pub wins: Option<i32>,
    pub losses: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum Competition {
    Elimination { teams: Vec<EliminationLeaderboard> },
    Unkown(String),
}

fn decode_territory_wars<'de, D>(deserializer: D) -> Result<HashMap<String, TerritoryWar>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let map: Option<_> = serde::Deserialize::deserialize(deserializer)?;

    Ok(map.unwrap_or_default())
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
    pub territory_war_id: i32,
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

    #[serde(rename = "faction")]
    pub faction_id: Option<i32>,
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

fn decode_territory<'de, D>(deserializer: D) -> Result<HashMap<String, Territory>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Debug, Deserialize)]
pub struct TerritoryWarReportTerritory {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerritoryWarOutcome {
    EndWithPeaceTreaty,
    EndWithDestroyDefense,
    FailAssault,
    SuccessAssault,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TerritoryWarReportWar {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub start: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub end: DateTime<Utc>,

    pub result: TerritoryWarOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerritoryWarReportRole {
    Aggressor,
    Defender,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerritoryWarReportFaction {
    pub name: String,
    pub score: i32,
    pub joins: i32,
    pub clears: i32,
    #[serde(rename = "type")]
    pub role: TerritoryWarReportRole,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TerritoryWarReport {
    pub territory: TerritoryWarReportTerritory,
    pub war: TerritoryWarReportWar,
    pub factions: HashMap<i32, TerritoryWarReportFaction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub enum ItemType {
    Primary,
    Secondary,
    Melee,
    Temporary,
    Defensive,
    Collectible,
    Medical,
    Drug,
    Booster,
    #[serde(rename = "Energy Drink")]
    EnergyDrink,
    Alcohol,
    Book,
    Candy,
    Car,
    Clothing,
    Electronic,
    Enhancer,
    Flower,
    Jewelry,
    Other,
    Special,
    #[serde(rename = "Supply Pack")]
    SupplyPack,
    Virus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
//Missing hand to hand because it is not possible as a weapon
pub enum WeaponType {
    Slashing,
    Rifle,
    SMG,
    Piercing,
    Clubbing,
    Pistol,
    #[serde(rename = "Machine gun")]
    MachineGun,
    Mechanical,
    Temporary,
    Heavy,
    Shotgun,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Item<'a> {
    pub name: String,
    pub description: String,
    #[serde(deserialize_with = "de_util::empty_string_is_none")]
    pub effect: Option<&'a str>,
    #[serde(deserialize_with = "de_util::empty_string_is_none")]
    pub requirement: Option<&'a str>,
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub weapon_type: Option<WeaponType>,
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub buy_price: Option<u64>,
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub sell_price: Option<u64>,
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub market_value: Option<u64>,
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub circulation: Option<u32>,
    pub image: String,
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
                b.selections([
                    TornSelection::Competition,
                    TornSelection::TerritoryWars,
                    TornSelection::Rackets,
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
            .torn(|b| b.selections([Selection::Territory]).id("NSC"))
            .await
            .unwrap();

        let territory = response.territory().unwrap();
        assert!(territory.contains_key("NSC"));
    }

    #[async_test]
    async fn invalid_territory() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .torn(|b| b.selections([Selection::Territory]).id("AAA"))
            .await
            .unwrap();

        assert!(response.territory().unwrap().is_empty());
    }

    #[async_test]
    async fn territory_war_report() {
        let key = setup();

        let response = Client::default()
            .torn_api(&key)
            .torn(|b| b.selections([Selection::TerritoryWarReport]).id(37403))
            .await
            .unwrap();

        assert_eq!(
            response.territory_war_report().unwrap().war.result,
            TerritoryWarOutcome::SuccessAssault
        );

        let response = Client::default()
            .torn_api(&key)
            .torn(|b| b.selections([Selection::TerritoryWarReport]).id(37502))
            .await
            .unwrap();

        assert_eq!(
            response.territory_war_report().unwrap().war.result,
            TerritoryWarOutcome::FailAssault
        );

        let response = Client::default()
            .torn_api(&key)
            .torn(|b| b.selections([Selection::TerritoryWarReport]).id(37860))
            .await
            .unwrap();

        assert_eq!(
            response.territory_war_report().unwrap().war.result,
            TerritoryWarOutcome::EndWithPeaceTreaty
        );

        let response = Client::default()
            .torn_api(&key)
            .torn(|b| b.selections([Selection::TerritoryWarReport]).id(23757))
            .await
            .unwrap();

        assert_eq!(
            response.territory_war_report().unwrap().war.result,
            TerritoryWarOutcome::EndWithDestroyDefense
        );
    }

    #[async_test]
    async fn item() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .torn(|b| b.selections([Selection::Items]).id(837))
            .await
            .unwrap();

        let item_list = response.items().unwrap();
        assert!(item_list.contains_key(&837));
    }
}
