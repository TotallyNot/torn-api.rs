use std::sync::Arc;

use async_trait::async_trait;
use indoc::indoc;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use thiserror::Error;

use crate::{ApiKey, IntoSelector, KeyDomain, KeyPoolStorage, KeySelector};

pub trait PgKeyDomain:
    KeyDomain + serde::Serialize + serde::de::DeserializeOwned + Eq + Unpin
{
}

impl<T> PgKeyDomain for T where
    T: KeyDomain + serde::Serialize + serde::de::DeserializeOwned + Eq + Unpin
{
}

#[derive(Debug, Error, Clone)]
pub enum PgStorageError<D>
where
    D: PgKeyDomain,
{
    #[error(transparent)]
    Pg(Arc<sqlx::Error>),

    #[error("No key avalaible for domain {0:?}")]
    Unavailable(KeySelector<PgKey<D>, D>),

    #[error("Key not found: '{0:?}'")]
    KeyNotFound(KeySelector<PgKey<D>, D>),
}

impl<D> From<sqlx::Error> for PgStorageError<D>
where
    D: PgKeyDomain,
{
    fn from(value: sqlx::Error) -> Self {
        Self::Pg(Arc::new(value))
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct PgKey<D>
where
    D: PgKeyDomain,
{
    pub id: i32,
    pub user_id: i32,
    pub key: String,
    pub uses: i16,
    pub domains: sqlx::types::Json<Vec<D>>,
}

#[inline(always)]
fn build_predicate<'b, D>(
    builder: &mut QueryBuilder<'b, Postgres>,
    selector: &'b KeySelector<PgKey<D>, D>,
) where
    D: PgKeyDomain,
{
    match selector {
        KeySelector::Id(id) => builder.push("id=").push_bind(id),
        KeySelector::UserId(user_id) => builder.push("user_id=").push_bind(user_id),
        KeySelector::Key(key) => builder.push("key=").push_bind(key),
        KeySelector::Has(domains) => builder
            .push("domains @> ")
            .push_bind(sqlx::types::Json(domains)),
        KeySelector::OneOf(domains) => {
            if domains.is_empty() {
                builder.push("false");
                return;
            }

            for (idx, domain) in domains.iter().enumerate() {
                if idx == 0 {
                    builder.push("(");
                } else {
                    builder.push(" or ");
                }
                builder
                    .push("domains @> ")
                    .push_bind(sqlx::types::Json(vec![domain]));
            }
            builder.push(")")
        }
    };
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
    type IdType = i32;

    #[inline(always)]
    fn value(&self) -> &str {
        &self.key
    }

    #[inline(always)]
    fn id(&self) -> Self::IdType {
        self.id
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
                user_id int4 not null,
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

        sqlx::query(indoc! {r#"
            CREATE INDEX IF NOT EXISTS "idx:api_keys.user_id" ON api_keys USING BTREE(user_id)
        "#})
        .execute(&self.pool)
        .await?;

        sqlx::query(indoc! {r#"
            create or replace function __unique_jsonb_array(jsonb) returns jsonb
                AS $$
                    select jsonb_agg(d::jsonb) from (
                        select distinct jsonb_array_elements_text($1) as d
                    ) t
                $$ language sql;
        "#})
        .execute(&self.pool)
        .await?;

        sqlx::query(indoc! {r#"
            create or replace function __filter_jsonb_array(jsonb, jsonb) returns jsonb
                AS $$
                    select jsonb_agg(d::jsonb) from (
                        select distinct jsonb_array_elements_text($1) as d
                    ) t where d<>$2::text
                $$ language sql;
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

    async fn acquire_key<S>(&self, selector: S) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();
        loop {
            let attempt = async {
                let mut tx = self.pool.begin().await?;

                sqlx::query("set transaction isolation level repeatable read")
                    .execute(&mut *tx)
                    .await?;

                let mut qb = QueryBuilder::new(indoc::indoc! {
                    r#"
                    with key as (
                        select 
                            id,
                            0::int2 as uses
                        from api_keys where last_used < date_trunc('minute', now()) 
                            and (cooldown is null or now() >= cooldown)
                            and "#
                });

                build_predicate(&mut qb, &selector);

                qb.push(indoc::indoc! {
                    "
                    \n    union (
                            select id, uses from api_keys 
                            where last_used >= date_trunc('minute', now()) 
                                and (cooldown is null or now() >= cooldown) 
                                and "
                });

                build_predicate(&mut qb, &selector);

                qb.push(indoc::indoc! {
                    "
                    \n        order by uses asc limit 1
                        )
                        order by uses asc limit 1
                    )
                    update api_keys set
                        uses = key.uses + 1,
                        cooldown = null,
                        flag = null,
                        last_used = now()
                    from key where 
                        api_keys.id=key.id and key.uses < "
                });

                qb.push_bind(self.limit);

                qb.push(indoc::indoc! { "
                    \nreturning
                        api_keys.id,
                        api_keys.user_id,
                        api_keys.key,
                        api_keys.uses,
                        api_keys.domains"
                });

                let key = qb.build_query_as().fetch_optional(&mut *tx).await?;

                tx.commit().await?;

                Result::<Option<Self::Key>, sqlx::Error>::Ok(key)
            }
            .await;

            match attempt {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => {
                    return self
                        .acquire_key(
                            selector
                                .fallback()
                                .ok_or_else(|| PgStorageError::Unavailable(selector))?,
                        )
                        .await
                }
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

    async fn acquire_many_keys<S>(
        &self,
        selector: S,
        number: i64,
    ) -> Result<Vec<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();
        loop {
            let attempt = async {
                let mut tx = self.pool.begin().await?;

                sqlx::query("set transaction isolation level repeatable read")
                    .execute(&mut *tx)
                    .await?;

                let mut qb = QueryBuilder::new(indoc::indoc! {
                    r#"select
                        id,
                        user_id,
                        key,
                        0::int2 as uses,
                        domains
                    from api_keys where last_used < date_trunc('minute', now())
                        and (cooldown is null or now() >= cooldown)
                        and "#
                });
                build_predicate(&mut qb, &selector);
                qb.push(indoc::indoc! {
                    "
                    \nunion
                    select
                        id,
                        user_id,
                        key,
                        uses,
                        domains
                    from api_keys where last_used >= date_trunc('minute', now())
                        and (cooldown is null or now() >= cooldown)
                        and "
                });
                build_predicate(&mut qb, &selector);
                qb.push("\norder by uses limit ");
                qb.push_bind(self.limit);

                let mut keys: Vec<Self::Key> = qb.build_query_as().fetch_all(&mut *tx).await?;

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
                .execute(&mut *tx)
                .await?;

                tx.commit().await?;

                Result::<Option<Vec<Self::Key>>, sqlx::Error>::Ok(Some(result))
            }
            .await;

            match attempt {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => {
                    return self
                        .acquire_many_keys(
                            selector
                                .fallback()
                                .ok_or_else(|| Self::Error::Unavailable(selector))?,
                            number,
                        )
                        .await
                }
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
                sqlx::query(
                    "update api_keys set cooldown=date_trunc('min', now()) + interval '1 min', \
                     flag=5 where id=$1",
                )
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
                sqlx::query(
                    "update api_keys set cooldown=date_trunc('day', now()) + interval '1 day', \
                     flag=14 where id=$1",
                )
                .bind(key.id)
                .execute(&self.pool)
                .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn store_key(
        &self,
        user_id: i32,
        key: String,
        domains: Vec<D>,
    ) -> Result<Self::Key, Self::Error> {
        sqlx::query_as(
            "insert into api_keys(user_id, key, domains) values ($1, $2, $3) on conflict on \
             constraint \"uq:api_keys.key\" do update set domains = \
             __unique_jsonb_array(excluded.domains || api_keys.domains) returning *",
        )
        .bind(user_id)
        .bind(&key)
        .bind(sqlx::types::Json(domains))
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn read_key<S>(&self, selector: S) -> Result<Option<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new("select * from api_keys where ");
        build_predicate(&mut qb, &selector);

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    async fn read_keys<S>(&self, selector: S) -> Result<Vec<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new("select * from api_keys where ");
        build_predicate(&mut qb, &selector);

        qb.build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    async fn remove_key<S>(&self, selector: S) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new("delete from api_keys where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(selector))
    }

    async fn add_domain_to_key<S>(&self, selector: S, domain: D) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new(
            "update api_keys set domains = __unique_jsonb_array(domains || jsonb_build_array(",
        );
        qb.push_bind(sqlx::types::Json(domain));
        qb.push(")) where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(selector))
    }

    async fn remove_domain_from_key<S>(
        &self,
        selector: S,
        domain: D,
    ) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new(
            "update api_keys set domains = coalesce(__filter_jsonb_array(domains, ",
        );
        qb.push_bind(sqlx::types::Json(domain));
        qb.push("), '[]'::jsonb) where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(selector))
    }

    async fn set_domains_for_key<S>(
        &self,
        selector: S,
        domains: Vec<D>,
    ) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new("update api_keys set domains = ");
        qb.push_bind(sqlx::types::Json(domains));
        qb.push(" where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgStorageError::KeyNotFound(selector))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use std::sync::Arc;

    use sqlx::Row;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum Domain {
        All,
        Guild { id: i64 },
        User { id: i32 },
        Faction { id: i32 },
    }

    impl KeyDomain for Domain {
        fn fallback(&self) -> Option<Self> {
            match self {
                Self::Guild { id: _ } => Some(Self::All),
                _ => None,
            }
        }
    }

    pub(crate) async fn setup(pool: PgPool) -> (PgKeyPoolStorage<Domain>, PgKey<Domain>) {
        sqlx::query("DROP TABLE IF EXISTS api_keys")
            .execute(&pool)
            .await
            .unwrap();

        let storage = PgKeyPoolStorage::new(pool.clone(), 1000);
        storage.initialise().await.unwrap();

        let key = storage
            .store_key(1, std::env::var("APIKEY").unwrap(), vec![Domain::All])
            .await
            .unwrap();

        (storage, key)
    }

    #[sqlx::test]
    async fn test_initialise(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        if let Err(e) = storage.initialise().await {
            panic!("Initialising key storage failed: {:?}", e);
        }
    }

    #[sqlx::test]
    async fn test_store_duplicate_key(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .store_key(1, key.key, vec![Domain::User { id: 1 }])
            .await
            .unwrap();

        assert_eq!(key.domains.0.len(), 2);
    }

    #[sqlx::test]
    async fn test_store_duplicate_key_duplicate_domain(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .store_key(1, key.key, vec![Domain::All])
            .await
            .unwrap();

        assert_eq!(key.domains.0.len(), 1);
    }

    #[sqlx::test]
    async fn test_add_domain(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .add_domain_to_key(KeySelector::Key(key.key), Domain::User { id: 12345 })
            .await
            .unwrap();

        assert!(key.domains.0.contains(&Domain::User { id: 12345 }));
    }

    #[sqlx::test]
    async fn test_add_domain_id(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .add_domain_to_key(KeySelector::Id(key.id), Domain::User { id: 12345 })
            .await
            .unwrap();

        assert!(key.domains.0.contains(&Domain::User { id: 12345 }));
    }

    #[sqlx::test]
    async fn test_add_duplicate_domain(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .add_domain_to_key(KeySelector::Key(key.key), Domain::All)
            .await
            .unwrap();
        assert_eq!(
            key.domains
                .0
                .into_iter()
                .filter(|d| *d == Domain::All)
                .count(),
            1
        );
    }

    #[sqlx::test]
    async fn test_remove_domain(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        storage
            .add_domain_to_key(KeySelector::Key(key.key.clone()), Domain::User { id: 1 })
            .await
            .unwrap();
        let key = storage
            .remove_domain_from_key(KeySelector::Key(key.key.clone()), Domain::User { id: 1 })
            .await
            .unwrap();

        assert_eq!(key.domains.0, vec![Domain::All]);
    }

    #[sqlx::test]
    async fn test_remove_domain_id(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        storage
            .add_domain_to_key(KeySelector::Id(key.id), Domain::User { id: 1 })
            .await
            .unwrap();
        let key = storage
            .remove_domain_from_key(KeySelector::Id(key.id), Domain::User { id: 1 })
            .await
            .unwrap();

        assert_eq!(key.domains.0, vec![Domain::All]);
    }

    #[sqlx::test]
    async fn test_remove_last_domain(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage
            .remove_domain_from_key(KeySelector::Key(key.key), Domain::All)
            .await
            .unwrap();

        assert!(key.domains.0.is_empty());
    }

    #[sqlx::test]
    async fn test_store_key(pool: PgPool) {
        let (storage, _) = setup(pool).await;
        let key = storage
            .store_key(1, "ABCDABCDABCDABCD".to_owned(), vec![])
            .await
            .unwrap();
        assert_eq!(key.value(), "ABCDABCDABCDABCD");
    }

    #[sqlx::test]
    async fn test_read_user_keys(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let keys = storage.read_keys(KeySelector::UserId(1)).await.unwrap();
        assert_eq!(keys.len(), 1);
    }

    #[sqlx::test]
    async fn acquire_one(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        if let Err(e) = storage.acquire_key(Domain::All).await {
            panic!("Acquiring key failed: {:?}", e);
        }
    }

    #[sqlx::test]
    async fn uses_spread(pool: PgPool) {
        let (storage, _) = setup(pool).await;
        storage
            .store_key(1, "ABC".to_owned(), vec![Domain::All])
            .await
            .unwrap();

        for _ in 0..10 {
            _ = storage.acquire_key(Domain::All).await.unwrap();
        }

        let keys = storage.read_keys(KeySelector::UserId(1)).await.unwrap();
        assert_eq!(keys.len(), 2);
        for key in keys {
            assert_eq!(key.uses, 5);
        }
    }

    #[sqlx::test]
    async fn test_flag_key_one(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        assert!(storage.flag_key(key, 2).await.unwrap());

        match storage.acquire_key(Domain::All).await.unwrap_err() {
            PgStorageError::Unavailable(KeySelector::Has(domains)) => {
                assert_eq!(domains, vec![Domain::All])
            }
            why => panic!("Expected domain unavailable error but found '{why}'"),
        }
    }

    #[sqlx::test]
    async fn test_flag_key_many(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        assert!(storage.flag_key(key, 2).await.unwrap());

        match storage.acquire_many_keys(Domain::All, 5).await.unwrap_err() {
            PgStorageError::Unavailable(KeySelector::Has(domains)) => {
                assert_eq!(domains, vec![Domain::All])
            }
            why => panic!("Expected domain unavailable error but found '{why}'"),
        }
    }

    #[sqlx::test]
    async fn acquire_many(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        match storage.acquire_many_keys(Domain::All, 30).await {
            Err(e) => panic!("Acquiring key failed: {:?}", e),
            Ok(keys) => assert_eq!(keys.len(), 30),
        }
    }

    // HACK: this test is time sensitive and will fail if runs at the top of the minute
    #[sqlx::test]
    async fn test_concurrent(pool: PgPool) {
        let storage = Arc::new(setup(pool).await.0);

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

    #[sqlx::test]
    async fn test_concurrent_spread(pool: PgPool) {
        let storage = Arc::new(setup(pool).await.0);

        for i in 0..24 {
            storage
                .store_key(1, format!("{}", i), vec![Domain::All])
                .await
                .unwrap();
        }

        for _ in 0..10 {
            let mut set = tokio::task::JoinSet::new();

            for _ in 0..50 {
                let storage = storage.clone();
                set.spawn(async move {
                    storage.acquire_key(Domain::All).await.unwrap();
                });
            }

            for _ in 0..50 {
                set.join_next().await.unwrap().unwrap();
            }

            let keys = storage.read_keys(KeySelector::UserId(1)).await.unwrap();

            assert_eq!(keys.len(), 25);

            for key in keys {
                assert_eq!(key.uses, 2);
            }

            sqlx::query("update api_keys set uses=0")
                .execute(&storage.pool)
                .await
                .unwrap();
        }
    }

    // HACK: this test is time sensitive and will fail if runs at the top of the minute
    #[sqlx::test]
    async fn test_concurrent_many(pool: PgPool) {
        let storage = Arc::new(setup(pool).await.0);
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

    #[sqlx::test]
    async fn read_key(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        let key = storage.read_key(KeySelector::Key(key.key)).await.unwrap();
        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn read_key_id(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        let key = storage.read_key(KeySelector::Id(key.id)).await.unwrap();
        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn read_nonexistent_key(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let key = storage.read_key(KeySelector::Id(-1)).await.unwrap();
        assert!(key.is_none());
    }

    #[sqlx::test]
    async fn query_key(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let key = storage.read_key(Domain::All).await.unwrap();
        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn query_nonexistent_key(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let key = storage.read_key(Domain::Guild { id: 0 }).await.unwrap();
        assert!(key.is_none());
    }

    #[sqlx::test]
    async fn query_all(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let keys = storage.read_keys(Domain::All).await.unwrap();
        assert!(keys.len() == 1);
    }

    #[sqlx::test]
    async fn query_by_id(pool: PgPool) {
        let (storage, _) = setup(pool).await;
        let key = storage.read_key(KeySelector::Id(1)).await.unwrap();

        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn query_by_key(pool: PgPool) {
        let (storage, key) = setup(pool).await;
        let key = storage.read_key(KeySelector::Key(key.key)).await.unwrap();

        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn query_by_set(pool: PgPool) {
        let (storage, _key) = setup(pool).await;
        let key = storage
            .read_key(KeySelector::OneOf(vec![
                Domain::All,
                Domain::Guild { id: 0 },
                Domain::Faction { id: 0 },
            ]))
            .await
            .unwrap();

        assert!(key.is_some());
    }

    #[sqlx::test]
    async fn all_selector(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        storage
            .add_domain_to_key(key.selector(), Domain::Faction { id: 1 })
            .await
            .unwrap();

        let key = storage
            .read_key(KeySelector::Has(vec![
                Domain::Faction { id: 1 },
                Domain::All,
            ]))
            .await
            .unwrap();

        assert!(key.is_some());

        let key = storage
            .read_key(KeySelector::Has(vec![
                Domain::All,
                Domain::Faction { id: 1 },
            ]))
            .await
            .unwrap();

        assert!(key.is_some());

        let key = storage
            .read_key(KeySelector::Has(vec![
                Domain::All,
                Domain::Faction { id: 2 },
                Domain::Faction { id: 1 },
            ]))
            .await
            .unwrap();

        assert!(key.is_none());
    }
}
