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
                v => Err(de::Error::unknown_variant(v, &["Elimination", ""])),
            }
        }
    }

    deserializer.deserialize_map(CompetitionVisitor)
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
            .torn(|b| b.selections(&[Selection::Competition]))
            .await
            .unwrap();

        response.competition().unwrap();
    }
}
