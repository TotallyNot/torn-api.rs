use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::Deserialize;

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum State {
    Okay,
    Traveling,
    Hospital,
    Abroad,
    Jail,
    Federal,
    Fallen,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
    use crate::{tests::{setup, Client, async_test}, ApiClient};

    #[async_test]
    async fn user() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .user(None)
            .selections(&[Selection::Basic, Selection::Discord, Selection::Profile, Selection::PersonalStats])
            .send()
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
            .user(Some(28))
            .selections(&[ Selection::Profile])
            .send()
            .await
            .unwrap();

        let faction = response.profile().unwrap().faction;

        assert!(faction.faction_id.is_none());
        assert!(faction.faction_name.is_none());
        assert!(faction.faction_tag.is_none());
        assert!(faction.days_in_faction.is_none());
        assert!(faction.position.is_none());
    }
}
