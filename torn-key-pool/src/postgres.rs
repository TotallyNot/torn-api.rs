use async_trait::async_trait;
use indoc::indoc;
use sqlx::{FromRow, PgPool};
use thiserror::Error;

use crate::{ApiKey, KeyDomain, KeyPoolStorage};

pub trait PgKeyDomain:
    KeyDomain + serde::Serialize + serde::de::DeserializeOwned + Eq + Unpin
{
}

impl<T> PgKeyDomain for T where
    T: KeyDomain + serde::Serialize + serde::de::DeserializeOwned + Eq + Unpin
{
}

#[derive(Debug, Error)]
pub enum PgStorageError<D>
where
    D: std::fmt::Debug,
{
    #[error(transparent)]
    Pg(#[from] sqlx::Error),

    #[error("No key avalaible for domain {0:?}")]
    Unavailable(D),

    #[error("Duplicate key '{0}'")]
    DuplicateKey(String),

    #[error("Duplicate domain '{0:?}'")]
    DuplicateDomain(D),

    #[error("Key not found: '{0}'")]
    KeyNotFound(String),
}

#[derive(Debug, Clone, FromRow)]
pub struct PgKey<D>
where
    D: PgKeyDomain,
{
    pub id: i32,
    pub key: String,
    pub uses: i16,
    pub domains: sqlx::types::Json<Vec<D>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PgKeyPoolStorage<D>
where
    D: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
{
    pool: PgPool,
    limit: i16,
    _phantom: std::marker::PhantomData<D>,
}

impl<D> ApiKey for PgKey<D>
where
    D: PgKeyDomain,
{
    fn value(&self) -> &str {
        &self.key
    }
}

impl<D> PgKeyPoolStorage<D>
where
    D: PgKeyDomain,
{
    pub fn new(pool: PgPool, limit: i16) -> Self {
        Self {
            pool,
            limit,
            _phantom: Default::default(),
        }
    }

    pub async fn initialise(&self) -> Result<(), PgStorageError<D>> {
        sqlx::query(indoc! {r#"
            CREATE TABLE IF NOT EXISTS api_keys (
                id serial primary key,
                key char(16) not null,
                uses int2 not null default 0,
                domains jsonb not null default '{}'::jsonb,
                last_used timestamptz not null default now(),
                flag int2,
                cooldown timestamptz,
                constraint "uq:api_keys.key" UNIQUE(key)
            )"#
        })
        .execute(&self.pool)
        .await?;

        sqlx::query(indoc! {r#"
            CREATE INDEX IF NOT EXISTS "idx:api_keys.domains" ON api_keys USING GIN(domains jsonb_path_ops)
        "#})
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(feature = "tokio-runtime")]
async fn random_sleep() {
    use rand::{thread_rng, Rng};
    let dur = tokio::time::Duration::from_millis(thread_rng().gen_range(1..50));
    tokio::time::sleep(dur).await;
}

#[cfg(all(not(feature = "tokio-runtime"), feature = "actix-runtime"))]
async fn random_sleep() {
    use rand::{thread_rng, Rng};
    let dur = std::time::Duration::from_millis(thread_rng().gen_range(1..50));
    actix_rt::time::sleep(dur).await;
}

#[async_trait]
impl<D> KeyPoolStorage for PgKeyPoolStorage<D>
where
    D: PgKeyDomain,
{
    type Key = PgKey<D>;
    type Domain = D;

    type Error = PgStorageError<D>;

    async fn acquire_key(&self, domain: D) -> Result<Self::Key, Self::Error> {
        loop {
            let attempt = async {
                let mut tx = self.pool.begin().await?;

                sqlx::query("set transaction isolation level repeatable read")
                    .execute(&mut tx)
                    .await?;

                let key = sqlx::query_as(&indoc::formatdoc!(
                    r#"
                    with key as (
                        select 
                            id,
                            0::int2 as uses
                        from api_keys where last_used < date_trunc('minute', now()) and (cooldown is null or now() >= cooldown) and domains @> $1
                        union (
                            select id, uses from api_keys 
                            where last_used >= date_trunc('minute', now()) and (cooldown is null or now() >= cooldown) and domains @> $1
                            order by uses asc
                        )
                        limit 1
                    )
                    update api_keys set
                        uses = key.uses + 1,
                        cooldown = null,
                        flag = null,
                        last_used = now()
                    from key where 
                        api_keys.id=key.id and key.uses < $2
                    returning
                        api_keys.id,
                        api_keys.key,
                        api_keys.uses,
                        api_keys.domains
                    "#,
                ))
                .bind(sqlx::types::Json(vec![&domain]))
                .bind(self.limit)
                .fetch_optional(&mut tx)
                .await?;

                            tx.commit().await?;

                Result::<Option<Self::Key>, sqlx::Error>::Ok(
                    key
                )
            }
            .await;

            match attempt {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => return Err(PgStorageError::Unavailable(domain)),
                Err(error) => {
                    if let Some(db_error) = error.as_database_error() {
                        let pg_error: &sqlx::postgres::PgDatabaseError = db_error.downcast_ref();
                        if pg_error.code() == "40001" {
                            random_sleep().await;
                        } else {
                            return Err(error.into());
                        }
                    } else {
                        return Err(error.into());
                    }
                }
            }
        }
    }

    async fn acquire_many_keys(
        &self,
        domain: D,
        number: i64,
    ) -> Result<Vec<Self::Key>, Self::Error> {
        loop {
            let attempt = async {
                let mut tx = self.pool.begin().await?;

                sqlx::query("set transaction isolation level repeatable read")
                    .execute(&mut tx)
                    .await?;

                let mut keys: Vec<Self::Key> = sqlx::query_as(&indoc::formatdoc!(
                    r#"select
                        id,
                        key,
                        0::int2 as uses,
                        domains
                    from api_keys where last_used < date_trunc('minute', now()) and (cooldown is null or now() >= cooldown) and domains @> $1
                    union
                    select
                        id,
                        key,
                        uses,
                        domains
                    from api_keys where last_used >= date_trunc('minute', now()) and (cooldown is null or now() >= cooldown) and domains @> $1
                    order by uses limit $2
                "#,
                ))
                .bind(sqlx::types::Json(vec![&domain]))
                .bind(number)
                .fetch_all(&mut tx)
                .await?;

                if keys.is_empty() {
                    tx.commit().await?;
                    return Ok(None);
                }

                keys.sort_unstable_by(|k1, k2| k1.uses.cmp(&k2.uses));

                let mut result = Vec::with_capacity(number as usize);
                let (max, rest) = keys.split_last_mut().unwrap();
                for key in rest {
                    let available = max.uses - key.uses;
                    let using = std::cmp::min(available, (number as i16) - (result.len() as i16));
                    key.uses += using;
                    result.extend(std::iter::repeat(key.clone()).take(using as usize));

                    if result.len() == number as usize {
                        break;
                    }
                }

                while result.len() < (number as usize) {
                    if keys[0].uses == self.limit {
                        break;
                    }

                    let take = std::cmp::min(keys.len(), (number as usize) - result.len());
                    let slice = &mut keys[0..take];
                    slice.iter_mut().for_each(|k| k.uses += 1);
                    result.extend_from_slice(slice);
                }

                sqlx::query(indoc! {r#"
                    update api_keys set
                        uses = tmp.uses,
                        cooldown = null,
                        flag = null,
                        last_used = now()
                    from (select unnest($1::int4[]) as id, unnest($2::int2[]) as uses) as tmp
                    where api_keys.id = tmp.id
                "#})
                .bind(keys.iter().map(|k| k.id).collect::<Vec<_>>())
                .bind(keys.iter().map(|k| k.uses).collect::<Vec<_>>())
                .execute(&mut tx)
                .await?;

                tx.commit().await?;

                Result::<Option<Vec<Self::Key>>, sqlx::Error>::Ok(Some(result))
            }
            .await;

            match attempt {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => return Err(Self::Error::Unavailable(domain)),
                Err(error) => {
                    if let Some(db_error) = error.as_database_error() {
                        let pg_error: &sqlx::postgres::PgDatabaseError = db_error.downcast_ref();
                        if pg_error.code() == "40001" {
                            random_sleep().await;
                        } else {
                            return Err(error.into());
                        }
                    } else {
                        return Err(error.into());
                    }
                }
            }
        }
    }

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Error> {
        // TODO: put keys in cooldown when appropriate
        match code {
            2 | 10 | 13 => {
                // invalid key, owner fedded or owner inactive
                sqlx::query(
                    "update api_keys set cooldown='infinity'::timestamptz, flag=$1 where id=$2",
                )
                .bind(code as i16)
                .bind(key.id)
                .execute(&self.pool)
                .await?;
                Ok(true)
            }
            5 => {
                // too many requests
                sqlx::query("update api_keys set cooldown=date_trunc('min', now()) + interval '1 min', flag=5 where id=$1")
                    .bind(key.id)
                    .execute(&self.pool)
                    .await?;
                Ok(true)
            }
            8 => {
                // IP block
                sqlx::query("update api_keys set cooldown=now() + interval '5 min', flag=8")
                    .execute(&self.pool)
                    .await?;
                Ok(false)
            }
            9 => {
                // API disabled
                sqlx::query("update api_keys set cooldown=now() + interval '1 min', flag=9")
                    .execute(&self.pool)
                    .await?;
                Ok(false)
            }
            14 => {
                // daily read limit reached
                sqlx::query("update api_keys set cooldown=date_trunc('day', now()) + interval '1 day', flag=14 where id=$1")
                    .bind(key.id)
                    .execute(&self.pool)
                    .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn store_key(&self, key: String, domains: Vec<D>) -> Result<Self::Key, Self::Error> {
        sqlx::query_as("insert into api_keys(key, domains) values ($1, $2) returning *")
            .bind(&key)
            .bind(sqlx::types::Json(domains))
            .fetch_one(&self.pool)
            .await
            .map_err(|why| {
                if let Some(error) = why.as_database_error() {
                    let pg_error: &sqlx::postgres::PgDatabaseError = error.downcast_ref();
                    if pg_error.code() == "23505" {
                        return PgStorageError::DuplicateKey(key);
                    }
                }
                PgStorageError::Pg(why)
            })
    }

    async fn read_key(&self, key: String) -> Result<Self::Key, Self::Error> {
        sqlx::query_as("select * from api_keys where key=$1")
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(key))
    }

    async fn remove_key(&self, key: String) -> Result<Self::Key, Self::Error> {
        sqlx::query_as("delete from api_keys where key=$1 returning *")
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(key))
    }

    async fn add_domain_to_key(&self, key: String, domain: D) -> Result<Self::Key, Self::Error> {
        let mut tx = self.pool.begin().await?;
        match sqlx::query_as::<sqlx::Postgres, PgKey<D>>(
            "update api_keys set domains = domains || jsonb_build_array($1) where key=$2 returning *",
        )
        .bind(sqlx::types::Json(domain.clone()))
        .bind(&key)
        .fetch_optional(&mut tx)
        .await?
        {
            None => Err(PgStorageError::KeyNotFound(key)),
            Some(key) => {
                if key.domains.0.iter().filter(|d| **d == domain).count() > 1 {
                    tx.rollback().await?;
                    return Err(PgStorageError::DuplicateDomain(domain));
                }
                tx.commit().await?;
                Ok(key)
            }
        }
    }

    async fn remove_domain_from_key(
        &self,
        key: String,
        domain: D,
    ) -> Result<Self::Key, Self::Error> {
        // FIX: potential race condition
        let api_key = self.read_key(key.clone()).await?;
        let domains = api_key
            .domains
            .0
            .into_iter()
            .filter(|d| *d != domain)
            .collect();

        self.set_domains_for_key(key, domains).await
    }

    async fn set_domains_for_key(
        &self,
        key: String,
        domains: Vec<D>,
    ) -> Result<Self::Key, Self::Error> {
        sqlx::query_as::<sqlx::Postgres, PgKey<D>>(
            "update api_keys set domains = $1 where key=$2 returning *",
        )
        .bind(sqlx::types::Json(domains))
        .bind(&key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| PgStorageError::KeyNotFound(key))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use std::sync::{Arc, Once};

    use sqlx::Row;
    use tokio::test;

    use super::*;

    static INIT: Once = Once::new();

    #[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum Domain {
        All,
        User { id: i32 },
        Faction { id: i32 },
    }

    pub(crate) async fn setup() -> PgKeyPoolStorage<Domain> {
        INIT.call_once(|| {
            dotenv::dotenv().ok();
        });

        let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        sqlx::query("DROP TABLE IF EXISTS api_keys")
            .execute(&pool)
            .await
            .unwrap();

        let storage = PgKeyPoolStorage::new(pool.clone(), 1000);
        storage.initialise().await.unwrap();

        storage
            .store_key(std::env::var("APIKEY").unwrap(), vec![Domain::All])
            .await
            .unwrap();

        storage
    }

    #[test]
    async fn test_initialise() {
        let storage = setup().await;

        if let Err(e) = storage.initialise().await {
            panic!("Initialising key storage failed: {:?}", e);
        }
    }

    #[test]
    async fn test_store_duplicate() {
        let storage = setup().await;
        match storage
            .store_key(std::env::var("APIKEY").unwrap(), vec![])
            .await
            .unwrap_err()
        {
            PgStorageError::DuplicateKey(key) => {
                assert_eq!(key, std::env::var("APIKEY").unwrap())
            }
            why => panic!("Expected duplicate key error but found '{why}'"),
        };
    }

    #[test]
    async fn test_add_domain() {
        let storage = setup().await;
        let key = storage
            .add_domain_to_key(std::env::var("APIKEY").unwrap(), Domain::User { id: 12345 })
            .await
            .unwrap();

        assert!(key.domains.0.contains(&Domain::User { id: 12345 }));
    }

    #[test]
    async fn test_add_duplicate_domain() {
        let storage = setup().await;
        match storage
            .add_domain_to_key(std::env::var("APIKEY").unwrap(), Domain::All)
            .await
            .unwrap_err()
        {
            PgStorageError::DuplicateDomain(d) => assert_eq!(d, Domain::All),
            why => panic!("Expected duplicate domain error but found '{why}'"),
        };
    }

    #[test]
    async fn test_remove_domain() {
        let storage = setup().await;
        let key = storage
            .remove_domain_from_key(std::env::var("APIKEY").unwrap(), Domain::All)
            .await
            .unwrap();

        assert!(key.domains.0.is_empty());
    }

    #[test]
    async fn test_store_key() {
        let storage = setup().await;
        let key = storage
            .store_key("ABCDABCDABCDABCD".to_owned(), vec![])
            .await
            .unwrap();
        assert_eq!(key.value(), "ABCDABCDABCDABCD");
    }

    #[test]
    async fn acquire_one() {
        let storage = setup().await;

        if let Err(e) = storage.acquire_key(Domain::All).await {
            panic!("Acquiring key failed: {:?}", e);
        }
    }

    #[test]
    async fn test_flag_key_one() {
        let storage = setup().await;
        let key = storage
            .read_key(std::env::var("APIKEY").unwrap())
            .await
            .unwrap();

        assert!(storage.flag_key(key, 2).await.unwrap());

        match storage.acquire_key(Domain::All).await.unwrap_err() {
            PgStorageError::Unavailable(d) => assert_eq!(d, Domain::All),
            why => panic!("Expected domain unavailable error but found '{why}'"),
        }
    }

    #[test]
    async fn test_flag_key_many() {
        let storage = setup().await;
        let key = storage
            .read_key(std::env::var("APIKEY").unwrap())
            .await
            .unwrap();

        assert!(storage.flag_key(key, 2).await.unwrap());

        match storage.acquire_many_keys(Domain::All, 5).await.unwrap_err() {
            PgStorageError::Unavailable(d) => assert_eq!(d, Domain::All),
            why => panic!("Expected domain unavailable error but found '{why}'"),
        }
    }

    #[test]
    async fn acquire_many() {
        let storage = setup().await;

        match storage.acquire_many_keys(Domain::All, 30).await {
            Err(e) => panic!("Acquiring key failed: {:?}", e),
            Ok(keys) => assert_eq!(keys.len(), 30),
        }
    }

    #[test]
    async fn test_concurrent() {
        let storage = Arc::new(setup().await);

        for _ in 0..10 {
            let mut set = tokio::task::JoinSet::new();

            for _ in 0..100 {
                let storage = storage.clone();
                set.spawn(async move {
                    storage.acquire_key(Domain::All).await.unwrap();
                });
            }

            for _ in 0..100 {
                set.join_next().await.unwrap().unwrap();
            }

            let uses: i16 = sqlx::query("select uses from api_keys")
                .fetch_one(&storage.pool)
                .await
                .unwrap()
                .get("uses");

            assert_eq!(uses, 100);

            sqlx::query("update api_keys set uses=0")
                .execute(&storage.pool)
                .await
                .unwrap();
        }
    }

    #[test]
    async fn test_concurrent_many() {
        let storage = Arc::new(setup().await);
        for _ in 0..10 {
            let mut set = tokio::task::JoinSet::new();

            for _ in 0..100 {
                let storage = storage.clone();
                set.spawn(async move {
                    storage.acquire_many_keys(Domain::All, 5).await.unwrap();
                });
            }

            for _ in 0..100 {
                set.join_next().await.unwrap().unwrap();
            }

            let uses: i16 = sqlx::query("select uses from api_keys")
                .fetch_one(&storage.pool)
                .await
                .unwrap()
                .get("uses");

            assert_eq!(uses, 500);

            sqlx::query("update api_keys set uses=0")
                .execute(&storage.pool)
                .await
                .unwrap();
        }
    }
}
