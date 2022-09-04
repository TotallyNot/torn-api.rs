use async_trait::async_trait;
use chrono::{DateTime, Utc};
use indoc::indoc;
use sqlx::{FromRow, PgPool};
use thiserror::Error;

use crate::{ApiKey, KeyDomain, KeyPool, KeyPoolStorage};

#[derive(Debug, Error)]
pub enum PgStorageError {
    #[error(transparent)]
    Pg(#[from] sqlx::Error),

    #[error("No key avalaible for domain {0:?}")]
    Unavailable(KeyDomain),
}

#[derive(Debug, Clone, FromRow)]
pub struct PgKey {
    pub id: i32,
    pub user_id: i32,
    pub faction_id: Option<i32>,
    pub key: String,
    pub uses: i16,
    pub user: bool,
    pub faction: bool,
    pub last_used: DateTime<Utc>,
}

impl ApiKey for PgKey {
    fn value(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct PgKeyPoolStorage {
    pool: PgPool,
    limit: i16,
}

impl PgKeyPoolStorage {
    pub fn new(pool: PgPool, limit: i16) -> Self {
        Self { pool, limit }
    }

    pub async fn initialise(&self) -> Result<(), PgStorageError> {
        sqlx::query(indoc! {r#"
            CREATE TABLE IF NOT EXISTS api_keys (
                id serial primary key,
                user_id int4 not null,
                faction_id int4,
                key char(16) not null,
                uses int2 not null default 0,
                "user" bool not null,
                faction bool not null,
                last_used timestamptz not null default now()
            )"#})
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl KeyPoolStorage for PgKeyPoolStorage {
    type Key = PgKey;

    type Error = PgStorageError;

    async fn acquire_key(&self, domain: KeyDomain) -> Result<Self::Key, Self::Error> {
        let predicate = match domain {
            KeyDomain::Public => "".to_owned(),
            KeyDomain::User(id) => format!("where and user_id={} and user", id),
            KeyDomain::Faction(id) => format!("where and faction_id={} and faction", id),
        };
        let key: Option<PgKey> = sqlx::query_as(&indoc::formatdoc!(
            r#"
            with key as (
                select 
                    id,
                    user_id,
                    faction_id,
                    key,
                    case
                        when extract(minute from last_used)=extract(minute from now()) then uses
                        else 0::smallint
                    end as uses,
                    user,
                    faction,
                    last_used
                from api_keys {}
                order by last_used asc limit 1 FOR UPDATE
            )
            update api_keys set
                uses = key.uses + 1,
                last_used = now()
            from key where 
                api_keys.id=key.id and key.uses < $1
            returning
                api_keys.id,
                api_keys.user_id,
                api_keys.faction_id,
                api_keys.key,
                api_keys.uses,
                api_keys.user,
                api_keys.faction,
                api_keys.last_used
        "#,
            predicate
        ))
        .bind(self.limit)
        .fetch_optional(&self.pool)
        .await?;

        key.ok_or(PgStorageError::Unavailable(domain))
    }

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Error> {
        // TODO: put keys in cooldown when appropriate
        match code {
            2 | 10 | 13 => {
                sqlx::query("delete from api_keys where id=$1")
                    .bind(key.id)
                    .execute(&self.pool)
                    .await?;
                Ok(true)
            }
            9 => Ok(false),
            _ => Ok(true),
        }
    }
}

pub type PgKeyPool<A> = KeyPool<A, PgKeyPoolStorage>;

impl<A> PgKeyPool<A>
where
    A: torn_api::ApiClient,
{
    pub async fn connect(
        client: A,
        database_url: &str,
        limit: i16,
    ) -> Result<Self, PgStorageError> {
        let db_pool = PgPool::connect(database_url).await?;
        let storage = PgKeyPoolStorage::new(db_pool, limit);
        storage.initialise().await?;

        let key_pool = Self::new(client, storage);

        Ok(key_pool)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Once;

    use tokio::test;

    use super::*;

    static INIT: Once = Once::new();

    pub(crate) async fn setup() -> PgKeyPoolStorage {
        INIT.call_once(|| {
            dotenv::dotenv().ok();
        });

        let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        PgKeyPoolStorage::new(pool, 3)
    }

    #[test]
    async fn test_initialise() {
        let storage = setup().await;

        if let Err(e) = storage.initialise().await {
            panic!("Initialising key storage failed: {:?}", e);
        }
    }

    #[test]
    async fn acquire_one() {
        let storage = setup().await;

        if let Err(e) = storage.acquire_key(KeyDomain::Public).await {
            panic!("Acquiring key failed: {:?}", e);
        }
    }
}
