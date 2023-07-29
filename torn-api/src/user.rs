use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};

use torn_api_macros::ApiCategory;

use crate::de_util;

pub use crate::common::{LastAction, Status};

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

#[derive(Debug, Clone)]
pub struct Faction<'a> {
    pub faction_id: i32,
    pub faction_name: &'a str,
    pub days_in_faction: i16,
    pub position: &'a str,
    pub faction_tag: Option<&'a str>,
}

fn deserialize_faction<'de, D>(deserializer: D) -> Result<Option<Faction<'de>>, D::Error>
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
        type Value = Option<Faction<'de>>;

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

#[derive(Debug, Clone, Deserialize)]
pub struct Basic<'a> {
    pub player_id: i32,
    pub name: &'a str,
    pub level: i16,
    pub gender: Gender,
    pub status: Status<'a>,
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
    #[serde(rename = "satants-soldiers")]
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
    Unknown,
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
        #[serde(other)]
        Ignore,
    }

    #[derive(Deserialize)]
    enum CompetitionName {
        Elimination,
        #[serde(other)]
        Unknown,
    }

    struct CompetitionVisitor;

    impl<'de> Visitor<'de> for CompetitionVisitor {
        type Value = Option<Competition>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("struct Competition")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_map(self)
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
                        let team_raw: &str = map.next_value()?;
                        team = if team_raw.is_empty() {
                            None
                        } else {
                            Some(match team_raw {
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
                                    team_raw,
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
                CompetitionName::Unknown => Ok(Some(Competition::Unknown)),
            }
        }
    }

    deserializer.deserialize_option(CompetitionVisitor)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile<'a> {
    pub player_id: i32,
    pub name: &'a str,
    pub rank: &'a str,
    pub level: i16,
    pub gender: Gender,
    pub age: i32,

    pub life: LifeBar,
    pub last_action: LastAction,
    #[serde(deserialize_with = "deserialize_faction")]
    pub faction: Option<Faction<'a>>,
    pub status: Status<'a>,

    #[serde(deserialize_with = "deserialize_comp")]
    pub competition: Option<Competition>,

    #[serde(deserialize_with = "de_util::int_is_bool")]
    pub revivable: bool,
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
pub struct Crimes1 {
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

#[derive(Deserialize)]
pub struct Crimes2 {
    pub vandalism: i32,
    pub theft: i32,
    pub counterfeiting: i32,
    pub fraud: i32,
    #[serde(rename = "illicitservices")]
    pub illicit_services: i32,
    #[serde(rename = "cybercrime")]
    pub cyber_crime: i32,
    pub extortion: i32,
    #[serde(rename = "illegalproduction")]
    pub illegal_production: i32,
    pub total: i32,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CriminalRecord {
    Crimes1(Crimes1),
    Crimes2(Crimes2),
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
    async fn bulk() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .users([1, 2111649, 374272176892674048i64], |b| {
                b.selections(&[Selection::Basic])
            })
            .await;

        response.get(&1).as_ref().unwrap().as_ref().unwrap();
        response.get(&2111649).as_ref().unwrap().as_ref().unwrap();
    }

    #[async_test]
    async fn discord() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(|b| b.id(374272176892674048i64).selections(&[Selection::Basic]))
            .await
            .unwrap();

        assert_eq!(response.basic().unwrap().player_id, 2111649);
    }
}
