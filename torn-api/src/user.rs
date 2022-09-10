use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};

use torn_api_macros::ApiCategory;

use super::de_util;

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "user")]
pub enum Selection {
    #[api(type = "Basic", flatten)]
    Basic,
    #[api(type = "Profile", flatten)]
    Profile,
    #[api(type = "Discord", field = "discord")]
    Discord,
    #[api(type = "PersonalStats", field = "personalstats")]
    PersonalStats,
    #[api(type = "CriminalRecord", field = "criminalrecord")]
    Crimes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Gender {
    Male,
    Female,
    Enby,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastAction {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Faction {
    pub faction_id: i32,
    pub faction_name: String,
    pub days_in_faction: i16,
    pub position: String,
    pub faction_tag: Option<String>,
}

fn deserialize_faction<'de, D>(deserializer: D) -> Result<Option<Faction>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Field {
        FactionId,
        FactionName,
        DaysInFaction,
        Position,
        FactionTag,
    }

    struct FactionVisitor;

    impl<'de> Visitor<'de> for FactionVisitor {
        type Value = Option<Faction>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("struct Faction")
        }

        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut faction_id = None;
            let mut faction_name = None;
            let mut days_in_faction = None;
            let mut position = None;
            let mut faction_tag = None;

            while let Some(key) = map.next_key()? {
                match key {
                    Field::FactionId => {
                        faction_id = Some(map.next_value()?);
                    }
                    Field::FactionName => {
                        faction_name = Some(map.next_value()?);
                    }
                    Field::DaysInFaction => {
                        days_in_faction = Some(map.next_value()?);
                    }
                    Field::Position => {
                        position = Some(map.next_value()?);
                    }
                    Field::FactionTag => {
                        faction_tag = map.next_value()?;
                    }
                }
            }
            let faction_id = faction_id.ok_or_else(|| de::Error::missing_field("faction_id"))?;
            let faction_name =
                faction_name.ok_or_else(|| de::Error::missing_field("faction_name"))?;
            let days_in_faction =
                days_in_faction.ok_or_else(|| de::Error::missing_field("days_in_faction"))?;
            let position = position.ok_or_else(|| de::Error::missing_field("position"))?;

            if faction_id == 0 {
                Ok(None)
            } else {
                Ok(Some(Faction {
                    faction_id,
                    faction_name,
                    days_in_faction,
                    position,
                    faction_tag,
                }))
            }
        }
    }

    const FIELDS: &[&str] = &[
        "faction_id",
        "faction_name",
        "days_in_faction",
        "position",
        "faction_tag",
    ];
    deserializer.deserialize_struct("Faction", FIELDS, FactionVisitor)
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
pub struct Status {
    pub description: String,
    #[serde(deserialize_with = "de_util::empty_string_is_none")]
    pub details: Option<String>,
    #[serde(rename = "color")]
    pub colour: StateColour,
    pub state: State,
    #[serde(deserialize_with = "de_util::zero_date_is_none")]
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Basic {
    pub player_id: i32,
    pub name: String,
    pub level: i16,
    pub gender: Gender,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Discord {
    #[serde(rename = "userID")]
    pub user_id: i32,
    #[serde(rename = "discordID", deserialize_with = "de_util::string_is_long")]
    pub discord_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LifeBar {
    pub current: i16,
    pub maximum: i16,
    pub increment: i16,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EliminationTeam {
    Firestarters,
    HardBoiled,
    QuackAddicts,
    RainMen,
    TotallyBoned,
    RawringThunder,
    DirtyCops,
    LaughingStock,
    JeanTherapy,
    #[serde(rename = "statants-soldiers")]
    SatansSoldiers,
    WolfPack,
    Sleepyheads,
}

#[derive(Debug, Clone)]
pub enum Competition {
    Elimination {
        score: i16,
        attacks: i16,
        team: EliminationTeam,
    },
}

fn deserialize_comp<'de, D>(deserializer: D) -> Result<Option<Competition>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    enum Field {
        Name,
        Score,
        Team,
        Attacks,
        TeamName,
    }

    #[derive(Deserialize)]
    enum CompetitionName {
        Elimination,
    }

    struct CompetitionVisitor;

    impl<'de> Visitor<'de> for CompetitionVisitor {
        type Value = Option<Competition>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("struct Competition")
        }

        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut team = None;
            let mut score = None;
            let mut attacks = None;
            let mut name = None;

            while let Some(key) = map.next_key()? {
                match key {
                    Field::Name => {
                        name = Some(map.next_value()?);
                    }
                    Field::Score => {
                        score = Some(map.next_value()?);
                    }
                    Field::Attacks => {
                        attacks = Some(map.next_value()?);
                    }
                    Field::Team => {
                        let team_raw: String = map.next_value()?;
                        team = if team_raw.is_empty() {
                            None
                        } else {
                            Some(match team_raw.as_str() {
                                "firestarters" => EliminationTeam::Firestarters,
                                "hard-boiled" => EliminationTeam::HardBoiled,
                                "quack-addicts" => EliminationTeam::QuackAddicts,
                                "rain-men" => EliminationTeam::RainMen,
                                "totally-boned" => EliminationTeam::TotallyBoned,
                                "rawring-thunder" => EliminationTeam::RawringThunder,
                                "dirty-cops" => EliminationTeam::DirtyCops,
                                "laughing-stock" => EliminationTeam::LaughingStock,
                                "jean-therapy" => EliminationTeam::JeanTherapy,
                                "satants-soldiers" => EliminationTeam::SatansSoldiers,
                                "wolf-pack" => EliminationTeam::WolfPack,
                                "sleepyheads" => EliminationTeam::Sleepyheads,
                                _ => Err(de::Error::unknown_variant(
                                    &team_raw,
                                    &[
                                        "firestarters",
                                        "hard-boiled",
                                        "quack-addicts",
                                        "rain-men",
                                        "totally-boned",
                                        "rawring-thunder",
                                        "dirty-cops",
                                        "laughing-stock",
                                        "jean-therapy",
                                        "satants-soldiers",
                                        "wolf-pack",
                                        "sleepyheads",
                                    ],
                                ))?,
                            })
                        }
                    }
                    _ => (),
                }
            }

            let name = name.ok_or_else(|| de::Error::missing_field("name"))?;

            match name {
                CompetitionName::Elimination => {
                    if let Some(team) = team {
                        let score = score.ok_or_else(|| de::Error::missing_field("score"))?;
                        let attacks = attacks.ok_or_else(|| de::Error::missing_field("attacks"))?;
                        Ok(Some(Competition::Elimination {
                            team,
                            score,
                            attacks,
                        }))
                    } else {
                        Ok(None)
                    }
                }
            }
        }
    }

    deserializer.deserialize_map(CompetitionVisitor)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub player_id: i32,
    pub name: String,
    pub rank: String,
    pub level: i16,
    pub gender: Gender,
    pub age: i32,

    pub life: LifeBar,
    pub last_action: LastAction,
    #[serde(deserialize_with = "deserialize_faction")]
    pub faction: Option<Faction>,
    pub status: Status,

    #[serde(deserialize_with = "deserialize_comp")]
    pub competition: Option<Competition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalStats {
    #[serde(rename = "attackswon")]
    pub attacks_won: i32,
    #[serde(rename = "attackslost")]
    pub attacks_lost: i32,
    #[serde(rename = "defendswon")]
    pub defends_won: i32,
    #[serde(rename = "defendslost")]
    pub defends_lost: i32,
    #[serde(rename = "statenhancersused")]
    pub stat_enhancers_used: i32,
    pub refills: i32,
    #[serde(rename = "drugsused")]
    pub drugs_used: i32,
    #[serde(rename = "xantaken")]
    pub xanax_taken: i32,
    #[serde(rename = "lsdtaken")]
    pub lsd_taken: i32,
    #[serde(rename = "networth")]
    pub net_worth: i64,
    #[serde(rename = "energydrinkused")]
    pub cans_used: i32,
    #[serde(rename = "boostersused")]
    pub boosters_used: i32,
    pub awards: i16,
    pub elo: i16,
    #[serde(rename = "daysbeendonator")]
    pub days_been_donator: i16,
    #[serde(rename = "bestdamage")]
    pub best_damage: i32,
}

#[derive(Deserialize)]
pub struct CriminalRecord {
    pub selling_illegal_products: i32,
    pub theft: i32,
    pub auto_theft: i32,
    pub drug_deals: i32,
    pub computer_crimes: i32,
    pub murder: i32,
    pub fraud_crimes: i32,
    pub other: i32,
    pub total: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{async_test, setup, Client, ClientTrait};

    #[async_test]
    async fn user() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| {
                b.selections(&[
                    Selection::Basic,
                    Selection::Discord,
                    Selection::Profile,
                    Selection::PersonalStats,
                    Selection::Crimes,
                ])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.discord().unwrap();
        response.profile().unwrap();
        response.personal_stats().unwrap();
        response.crimes().unwrap();
    }

    #[async_test]
    async fn not_in_faction() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| b.id(28).selections(&[Selection::Profile]))
            .await
            .unwrap();

        let faction = response.profile().unwrap().faction;

        assert!(faction.is_none());
    }

    #[async_test]
    async fn team_visible() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| b.selections(&[Selection::Profile]))
            .await
            .unwrap();

        let profile = response.profile().unwrap();
        assert!(profile.competition.is_some());
    }
}
