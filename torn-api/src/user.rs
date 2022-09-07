use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};

use macros::ApiCategory;

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

#[derive(Debug, Clone, Deserialize)]
pub struct Faction {
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub faction_id: Option<i32>,
    #[serde(deserialize_with = "de_util::none_is_none")]
    pub faction_name: Option<String>,
    #[serde(deserialize_with = "de_util::zero_is_none")]
    pub days_in_faction: Option<i16>,
    #[serde(deserialize_with = "de_util::none_is_none")]
    pub position: Option<String>,
    pub faction_tag: Option<String>,
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
    pub discord_id: i64,
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
    #[serde(rename_all = "lowercase")]
    enum Field {
        Name,
        Score,
        Team,
        Attacks,
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
            let mut team: Option<EliminationTeam> = None;
            let mut score = None;
            let mut attacks = None;
            let mut name: Option<CompetitionName> = None;

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
                }
            }

            match (name, team, score, attacks) {
                (Some(CompetitionName::Elimination), Some(team), Some(score), Some(attacks)) => {
                    Ok(Some(Competition::Elimination {
                        team,
                        score,
                        attacks,
                    }))
                }
                _ => Ok(None),
            }
        }
    }

    const FIELDS: &[&str] = &["name", "score", "team", "attacks"];
    deserializer.deserialize_struct("Competition", FIELDS, CompetitionVisitor)
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
    pub faction: Faction,
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
                ])
            })
            .await
            .unwrap();

        response.basic().unwrap();
        response.discord().unwrap();
        response.profile().unwrap();
        response.personal_stats().unwrap();
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

        assert!(faction.faction_id.is_none());
        assert!(faction.faction_name.is_none());
        assert!(faction.faction_tag.is_none());
        assert!(faction.days_in_faction.is_none());
        assert!(faction.position.is_none());
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

    #[async_test]
    async fn team_invisible() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| b.id(2526617).selections(&[Selection::Profile]))
            .await
            .unwrap();

        let profile = response.profile().unwrap();
        assert!(profile.competition.is_none());
    }

    #[async_test]
    async fn team_none() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| b.id(2681712).selections(&[Selection::Profile]))
            .await
            .unwrap();

        let profile = response.profile().unwrap();
        assert!(profile.competition.is_none());
    }
}
