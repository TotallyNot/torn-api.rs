use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use torn_api_macros::ApiCategory;

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "key")]
pub enum Selection {
    #[api(type = "Info", flatten)]
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessType {
    #[serde(rename = "Custom")]
    Custom,

    #[serde(rename = "Public Only")]
    Public,

    #[serde(rename = "Minimal Access")]
    Minimal,

    #[serde(rename = "Limited Access")]
    Limited,

    #[serde(rename = "Full Access")]
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeySelection {
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserSelection {
    Ammo,
    Attacks,
    AttacksFull,
    Bars,
    Basic,
    BattleStats,
    Bazaar,
    Cooldowns,
    Crimes,
    Discord,
    Display,
    Education,
    Events,
    Gym,
    Hof,
    Honors,
    Icons,
    Inventory,
    JobPoints,
    Log,
    Medals,
    Merits,
    Messages,
    Missions,
    Money,
    Networth,
    NewEvents,
    NewMessages,
    Notifications,
    Perks,
    PersonalStats,
    Profile,
    Properties,
    ReceivedEvents,
    Refills,
    Reports,
    Revives,
    RevivesFull,
    Skills,
    Stocks,
    Timestamp,
    Travel,
    WeaponExp,
    WorkStats,
    Lookup,
    PublicStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FactionSelection {
    Applications,
    Armor,
    ArmoryNews,
    AttackNews,
    Attacks,
    AttacksFull,
    Basic,
    Boosters,
    Cesium,
    Chain,
    ChainReport,
    Chains,
    Contributors,
    Crimenews,
    Crimes,
    Currency,
    Donations,
    Drugs,
    FundsNews,
    MainNews,
    Medical,
    MembershipNews,
    Positions,
    Reports,
    Revives,
    RevivesFull,
    Stats,
    Temporary,
    Territory,
    TerritoryNews,
    Timestamp,
    Upgrades,
    Weapons,
    Lookup,
    Caches,
    CrimeExp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompanySelection {
    Applications,
    Companies,
    Detailed,
    Employees,
    News,
    NewsFull,
    Profile,
    Stock,
    Timestamp,
    Lookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TornSelection {
    Bank,
    Cards,
    ChainReport,
    Companies,
    Competition,
    Education,
    FactionTree,
    Gyms,
    Honors,
    Items,
    ItemStats,
    LogCategories,
    LogTypes,
    Medals,
    OrganisedCrimes,
    PawnShop,
    PokerTables,
    Properties,
    Rackets,
    Raids,
    RankedWars,
    RankedWarReport,
    Stats,
    Stocks,
    Territory,
    TerritoryWars,
    Timestamp,
    Lookup,
    CityShops,
    ItemDetails,
    TerritoryNames,
    TerritoryWarReport,
    RaidReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketSelection {
    Bazaar,
    ItemMarket,
    PointsMarket,
    Timestamp,
    Lookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertySelection {
    Property,
    Timestamp,
    Lookup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selections {
    pub user: HashSet<UserSelection>,
    pub faction: HashSet<FactionSelection>,
    pub company: HashSet<CompanySelection>,
    pub torn: HashSet<TornSelection>,
    pub market: HashSet<MarketSelection>,
    pub property: HashSet<PropertySelection>,
    pub key: HashSet<KeySelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub access_level: i16,
    pub access_type: AccessType,
    pub selections: Selections,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{async_test, setup, Client, ClientTrait};

    #[async_test]
    async fn key() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .key(|b| b.selections(&[Selection::Info]))
            .await
            .unwrap();

        response.info().unwrap();
    }
}
