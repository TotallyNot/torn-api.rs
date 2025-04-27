use futures::future::BoxFuture;
use indoc::formatdoc;
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

#[derive(Debug, Error)]
pub enum PgKeyPoolError<D>
where
    D: PgKeyDomain,
{
    #[error("Databank: {0}")]
    Pg(#[from] sqlx::Error),

    #[error("Network: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Parsing: {0}")]
    Parsing(#[from] serde_json::Error),

    #[error("Api: {0}")]
    Api(#[from] torn_api::ApiError),

    #[error("No key avalaible for domain {0:?}")]
    Unavailable(KeySelector<PgKey<D>, D>),

    #[error("Key not found: '{0:?}'")]
    KeyNotFound(KeySelector<PgKey<D>, D>),
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
    schema: Option<String>,
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
    pub fn new(pool: PgPool, limit: i16, schema: Option<String>) -> Self {
        Self {
            pool,
            limit,
            schema,
            _phantom: Default::default(),
        }
    }

    fn table_name(&self) -> String {
        match self.schema.as_ref() {
            Some(schema) => format!("{schema}.api_keys"),
            None => "api_keys".to_owned(),
        }
    }

    fn unique_array_fn(&self) -> String {
        match self.schema.as_ref() {
            Some(schema) => format!("{schema}.__unique_jsonb_array"),
            None => "__unique_jsonb_array".to_owned(),
        }
    }

    fn filter_array_fn(&self) -> String {
        match self.schema.as_ref() {
            Some(schema) => format!("{schema}.__filter_jsonb_array"),
            None => "__filter_jsonb_array".to_owned(),
        }
    }

    pub async fn initialise(&self) -> Result<(), PgKeyPoolError<D>> {
        if let Some(schema) = self.schema.as_ref() {
            sqlx::query(&format!("create schema if not exists {}", schema))
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(&formatdoc! {r#"
            CREATE TABLE IF NOT EXISTS {} (
                id serial primary key,
                user_id int4 not null,
                key char(16) not null,
                uses int2 not null default 0,
                domains jsonb not null default '{{}}'::jsonb,
                last_used timestamptz not null default now(),
                flag int2,
                cooldown timestamptz,
                constraint "uq:api_keys.key" UNIQUE(key)
            )"#,
            self.table_name()
        })
        .execute(&self.pool)
        .await?;

        sqlx::query(&formatdoc! {r#"
            CREATE INDEX IF NOT EXISTS "idx:api_keys.domains" ON {} USING GIN(domains jsonb_path_ops)
        "#, self.table_name()})
        .execute(&self.pool)
        .await?;

        sqlx::query(&formatdoc! {r#"
            CREATE INDEX IF NOT EXISTS "idx:api_keys.user_id" ON {} USING BTREE(user_id)
        "#, self.table_name()})
        .execute(&self.pool)
        .await?;

        sqlx::query(&formatdoc! {r#"
            create or replace function {}(jsonb) returns jsonb
                AS $$
                    select jsonb_agg(d::jsonb) from (
                        select distinct jsonb_array_elements_text($1) as d
                    ) t
                $$ language sql;
        "#, self.unique_array_fn()})
        .execute(&self.pool)
        .await?;

        sqlx::query(&formatdoc! {r#"
            create or replace function {}(jsonb, jsonb) returns jsonb
                AS $$
                    select jsonb_agg(d::jsonb) from (
                        select distinct jsonb_array_elements_text($1) as d
                    ) t where d<>$2::text
                $$ language sql;
        "#, self.filter_array_fn()})
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(feature = "tokio-runtime")]
async fn random_sleep() {
    use rand::{rng, Rng};
    let dur = tokio::time::Duration::from_millis(rng().random_range(1..50));
    tokio::time::sleep(dur).await;
}

impl<D> KeyPoolStorage for PgKeyPoolStorage<D>
where
    D: PgKeyDomain,
{
    type Key = PgKey<D>;
    type Domain = D;

    type Error = PgKeyPoolError<D>;

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

                let mut qb = QueryBuilder::new(&formatdoc! {
                    r#"
                    with key as (
                        select
                            id,
                            0::int2 as uses
                        from {} where last_used < date_trunc('minute', now())
                            and (cooldown is null or now() >= cooldown)
                            and "#,
                    self.table_name()
                });

                build_predicate(&mut qb, &selector);

                qb.push(formatdoc! {
                    "
                    \n    union (
                            select id, uses from {}
                            where last_used >= date_trunc('minute', now())
                                and (cooldown is null or now() >= cooldown)
                                and ",
                    self.table_name()
                });

                build_predicate(&mut qb, &selector);

                qb.push(formatdoc! {
                    "
                    \n        order by uses asc limit 1
                        )
                        order by uses asc limit 1
                    )
                    update {} as keys set
                        uses = key.uses + 1,
                        cooldown = null,
                        flag = null,
                        last_used = now()
                    from key where
                        keys.id=key.id and key.uses < ",
                    self.table_name()
                });

                qb.push_bind(self.limit);

                qb.push(indoc::indoc! { "
                    \nreturning keys.id, keys.user_id, keys.key, keys.uses, keys.domains"
                });

                let key = qb.build_query_as().fetch_optional(&mut *tx).await?;

                tx.commit().await?;

                Result::<Option<Self::Key>, sqlx::Error>::Ok(key)
            }
            .await;

            match attempt {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => {
                    fn recurse<D>(
                        storage: &PgKeyPoolStorage<D>,
                        selector: KeySelector<PgKey<D>, D>,
                    ) -> BoxFuture<Result<PgKey<D>, PgKeyPoolError<D>>>
                    where
                        D: PgKeyDomain,
                    {
                        Box::pin(storage.acquire_key(selector))
                    }

                    return recurse(
                        self,
                        selector
                            .fallback()
                            .ok_or_else(|| PgKeyPoolError::Unavailable(selector))?,
                    )
                    .await;
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

                let mut qb = QueryBuilder::new(&formatdoc! {
                    r#"select
                        id,
                        user_id,
                        key,
                        0::int2 as uses,
                        domains
                    from {} where last_used < date_trunc('minute', now())
                        and (cooldown is null or now() >= cooldown)
                        and "#,
                    self.table_name()
                });
                build_predicate(&mut qb, &selector);
                qb.push(formatdoc! {
                    "
                    \nunion
                    select
                        id,
                        user_id,
                        key,
                        uses,
                        domains
                    from {} where last_used >= date_trunc('minute', now())
                        and (cooldown is null or now() >= cooldown)
                        and ",
                    self.table_name()
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
                    result.extend(std::iter::repeat_n(key.clone(), using as usize));

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

                sqlx::query(&formatdoc! {r#"
                    update {} keys set
                        uses = tmp.uses,
                        cooldown = null,
                        flag = null,
                        last_used = now()
                    from (select unnest($1::int4[]) as id, unnest($2::int2[]) as uses) as tmp
                    where keys.id = tmp.id
                "#, self.table_name()})
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
                    fn recurse<D>(
                        storage: &PgKeyPoolStorage<D>,
                        selector: KeySelector<PgKey<D>, D>,
                        number: i64,
                    ) -> BoxFuture<Result<Vec<PgKey<D>>, PgKeyPoolError<D>>>
                    where
                        D: PgKeyDomain,
                    {
                        Box::pin(storage.acquire_many_keys(selector, number))
                    }

                    return recurse(
                        self,
                        selector
                            .fallback()
                            .ok_or_else(|| Self::Error::Unavailable(selector))?,
                        number,
                    )
                    .await;
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

    async fn timeout_key<S>(
        &self,
        selector: S,
        duration: std::time::Duration,
    ) -> Result<(), Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new(format!(
            "update {} set cooldown=now() + ",
            self.table_name()
        ));
        qb.push_bind(duration);
        qb.push(" where ");
        build_predicate(&mut qb, &selector);

        qb.build().fetch_optional(&self.pool).await?;

        Ok(())
    }

    async fn store_key(
        &self,
        user_id: i32,
        key: String,
        domains: Vec<D>,
    ) -> Result<Self::Key, Self::Error> {
        sqlx::query_as(&dbg!(formatdoc!(
            "insert into {} as api_keys(user_id, key, domains) values ($1, $2, $3) 
            on conflict(key) do update 
            set domains = {}(excluded.domains || api_keys.domains) returning *",
            self.table_name(),
            self.unique_array_fn()
        )))
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

        let mut qb = QueryBuilder::new(format!("select * from {} where ", self.table_name()));
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

        let mut qb = QueryBuilder::new(format!("select * from {} where ", self.table_name()));
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

        let mut qb = QueryBuilder::new(format!("delete from {} where ", self.table_name()));
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgKeyPoolError::KeyNotFound(selector))
    }

    async fn add_domain_to_key<S>(&self, selector: S, domain: D) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>,
    {
        let selector = selector.into_selector();

        let mut qb = QueryBuilder::new(format!(
            "update {} set domains = {}(domains || jsonb_build_array(",
            self.table_name(),
            self.unique_array_fn()
        ));
        qb.push_bind(sqlx::types::Json(domain));
        qb.push(")) where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgKeyPoolError::KeyNotFound(selector))
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

        let mut qb = QueryBuilder::new(format!(
            "update {} set domains = coalesce({}(domains, ",
            self.table_name(),
            self.filter_array_fn()
        ));
        qb.push_bind(sqlx::types::Json(domain));
        qb.push("), '[]'::jsonb) where ");
        build_predicate(&mut qb, &selector);
        qb.push(" returning *");

        qb.build_query_as()
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| PgKeyPoolError::KeyNotFound(selector))
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
            .ok_or_else(|| PgKeyPoolError::KeyNotFound(selector))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use std::{sync::Arc, time::Duration};

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

        let storage = PgKeyPoolStorage::new(pool.clone(), 1000, Some("test".to_owned()));
        storage.initialise().await.unwrap();

        let key = storage
            .store_key(1, std::env::var("API_KEY").unwrap(), vec![Domain::All])
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

            let uses: i16 = sqlx::query(&format!("select uses from {}", storage.table_name()))
                .fetch_one(&storage.pool)
                .await
                .unwrap()
                .get("uses");

            assert_eq!(uses, 100);

            sqlx::query(&format!("update {} set uses=0", storage.table_name()))
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

            sqlx::query(&format!("update {} set uses=0", storage.table_name()))
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

            let uses: i16 = sqlx::query(&format!("select uses from {}", storage.table_name()))
                .fetch_one(&storage.pool)
                .await
                .unwrap()
                .get("uses");

            assert_eq!(uses, 500);

            sqlx::query(&format!("update {} set uses=0", storage.table_name()))
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
    async fn timeout(pool: PgPool) {
        let (storage, key) = setup(pool).await;

        storage
            .timeout_key(KeySelector::Id(key.id()), Duration::from_secs(60))
            .await
            .unwrap();
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
