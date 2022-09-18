use std::collections::BTreeMap;

use serde::Deserialize;

use torn_api_macros::ApiCategory;

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "faction")]
pub enum Selection {
    #[api(type = "Basic", flatten)]
    Basic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Member {
    pub name: String,
    pub level: i16,
    pub days_in_faction: i16,
    pub position: String,
    pub status: super::user::Status,
    pub last_action: super::user::LastAction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Basic {
    #[serde(rename = "ID")]
    pub id: i32,
    pub name: String,
    pub leader: i32,

    pub respect: i32,
    pub age: i16,
    pub capacity: i16,
    pub best_chain: i32,

    pub members: BTreeMap<i32, Member>,
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
            .faction(|b| b.selections(&[Selection::Basic]))
            .await
            .unwrap();

        response.basic().unwrap();
    }
}
