#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use futures::{future::BoxFuture, FutureExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;
use torn_api::{
    executor::Executor,
    request::{ApiRequest, ApiResponse},
    ApiError,
};

pub trait ApiKeyId: Clone + PartialEq + Eq + std::hash::Hash + Send + Sync {}

impl<T> ApiKeyId for T where T: Clone + PartialEq + Eq + std::hash::Hash + Send + Sync {}

pub trait ApiKey: Send + Sync + Clone + 'static {
    type IdType: ApiKeyId;

    fn value(&self) -> &str;

    fn id(&self) -> Self::IdType;

    fn selector<D>(&self) -> KeySelector<Self, D>
    where
        D: KeyDomain,
    {
        KeySelector::Id(self.id())
    }
}

pub trait KeyDomain: Clone + std::fmt::Debug + Send + Sync + 'static {
    fn fallback(&self) -> Option<Self> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    Key(String),
    Id(K::IdType),
    UserId(i32),
    Has(Vec<D>),
    OneOf(Vec<D>),
}

impl<K, D> KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    pub(crate) fn fallback(&self) -> Option<Self> {
        match self {
            Self::Key(_) | Self::UserId(_) | Self::Id(_) => None,
            Self::Has(domains) => {
                let fallbacks: Vec<_> = domains.iter().filter_map(|d| d.fallback()).collect();
                if fallbacks.is_empty() {
                    None
                } else {
                    Some(Self::Has(fallbacks))
                }
            }
            Self::OneOf(domains) => {
                let fallbacks: Vec<_> = domains.iter().filter_map(|d| d.fallback()).collect();
                if fallbacks.is_empty() {
                    None
                } else {
                    Some(Self::OneOf(fallbacks))
                }
            }
        }
    }
}

pub trait IntoSelector<K, D>: Send
where
    K: ApiKey,
    D: KeyDomain,
{
    fn into_selector(self) -> KeySelector<K, D>;
}

impl<K, D> IntoSelector<K, D> for D
where
    K: ApiKey,
    D: KeyDomain,
{
    fn into_selector(self) -> KeySelector<K, D> {
        KeySelector::Has(vec![self])
    }
}

impl<K, D> IntoSelector<K, D> for KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    fn into_selector(self) -> KeySelector<K, D> {
        self
    }
}

pub trait KeyPoolStorage: Send + Sync {
    type Key: ApiKey;
    type Domain: KeyDomain;
    type Error: From<reqwest::Error> + From<serde_json::Error> + From<torn_api::ApiError> + Send;

    fn acquire_key<S>(
        &self,
        selector: S,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn acquire_many_keys<S>(
        &self,
        selector: S,
        number: i64,
    ) -> impl Future<Output = Result<Vec<Self::Key>, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn store_key(
        &self,
        user_id: i32,
        key: String,
        domains: Vec<Self::Domain>,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send;

    fn read_key<S>(
        &self,
        selector: S,
    ) -> impl Future<Output = Result<Option<Self::Key>, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn read_keys<S>(
        &self,
        selector: S,
    ) -> impl Future<Output = Result<Vec<Self::Key>, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn remove_key<S>(
        &self,
        selector: S,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn add_domain_to_key<S>(
        &self,
        selector: S,
        domain: Self::Domain,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn remove_domain_from_key<S>(
        &self,
        selector: S,
        domain: Self::Domain,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn set_domains_for_key<S>(
        &self,
        selector: S,
        domains: Vec<Self::Domain>,
    ) -> impl Future<Output = Result<Self::Key, Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    fn timeout_key<S>(
        &self,
        selector: S,
        duration: Duration,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        S: IntoSelector<Self::Key, Self::Domain>;
}

#[derive(Default)]
pub struct PoolOptions<S>
where
    S: KeyPoolStorage,
{
    comment: Option<String>,
    #[allow(clippy::type_complexity)]
    error_hooks: HashMap<
        u16,
        Box<
            dyn for<'a> Fn(&'a S, &'a S::Key) -> BoxFuture<'a, Result<bool, S::Error>>
                + Send
                + Sync,
        >,
    >,
}

pub struct KeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pool: &'p KeyPool<S>,
    selector: KeySelector<S::Key, S::Domain>,
}

impl<'p, S> KeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pub fn new(pool: &'p KeyPool<S>, selector: KeySelector<S::Key, S::Domain>) -> Self {
        Self { pool, selector }
    }

    async fn execute_request<D>(&self, request: ApiRequest<D>) -> Result<ApiResponse<D>, S::Error>
    where
        D: Send,
    {
        let key = self.pool.storage.acquire_key(self.selector.clone()).await?;

        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("ApiKey {}", key.value())).unwrap(),
        );

        let resp = self
            .pool
            .client
            .get(request.url())
            .headers(headers)
            .send()
            .await?;

        let status = resp.status();

        let bytes = resp.bytes().await?;

        if let Some(err) = decode_error(&bytes)? {
            if let Some(handler) = self.pool.options.error_hooks.get(&err.code()) {
                let retry = (*handler)(&self.pool.storage, &key).await?;

                if retry {
                    return Box::pin(self.execute_request(request)).await;
                }
            }
            Err(err.into())
        } else {
            Ok(ApiResponse {
                discriminant: request.disriminant,
                body: Some(bytes),
                status,
            })
        }
    }
}

pub struct PoolBuilder<S>
where
    S: KeyPoolStorage,
{
    client: reqwest::Client,
    storage: S,
    options: crate::PoolOptions<S>,
}

impl<S> PoolBuilder<S>
where
    S: KeyPoolStorage,
{
    pub fn new(storage: S) -> Self {
        Self {
            client: reqwest::Client::builder()
                .brotli(true)
                .http2_keep_alive_timeout(Duration::from_secs(60))
                .http2_keep_alive_interval(Duration::from_secs(5))
                .https_only(true)
                .build()
                .unwrap(),
            storage,
            options: PoolOptions {
                comment: None,
                error_hooks: Default::default(),
            },
        }
    }

    pub fn comment(mut self, c: impl ToString) -> Self {
        self.options.comment = Some(c.to_string());
        self
    }

    pub fn error_hook<F>(mut self, code: u16, handler: F) -> Self
    where
        F: for<'a> Fn(&'a S, &'a S::Key) -> BoxFuture<'a, Result<bool, S::Error>>
            + Send
            + Sync
            + 'static,
    {
        self.options.error_hooks.insert(code, Box::new(handler));

        self
    }

    pub fn use_default_hooks(self) -> Self {
        self.error_hook(2, |storage, key| {
            async move {
                storage.remove_key(KeySelector::Id(key.id())).await?;
                Ok(true)
            }
            .boxed()
        })
        .error_hook(5, |storage, key| {
            async move {
                storage
                    .timeout_key(KeySelector::Id(key.id()), Duration::from_secs(60))
                    .await?;
                Ok(true)
            }
            .boxed()
        })
        .error_hook(10, |storage, key| {
            async move {
                storage.remove_key(KeySelector::Id(key.id())).await?;
                Ok(true)
            }
            .boxed()
        })
        .error_hook(13, |storage, key| {
            async move {
                storage
                    .timeout_key(KeySelector::Id(key.id()), Duration::from_secs(24 * 3_600))
                    .await?;
                Ok(true)
            }
            .boxed()
        })
        .error_hook(18, |storage, key| {
            async move {
                storage
                    .timeout_key(KeySelector::Id(key.id()), Duration::from_secs(24 * 3_600))
                    .await?;
                Ok(true)
            }
            .boxed()
        })
    }

    pub fn build(self) -> KeyPool<S> {
        KeyPool {
            client: self.client,
            storage: self.storage,
            options: Arc::new(self.options),
        }
    }
}

pub struct KeyPool<S>
where
    S: KeyPoolStorage,
{
    pub client: reqwest::Client,
    pub storage: S,
    pub options: Arc<PoolOptions<S>>,
}

impl<S> KeyPool<S>
where
    S: KeyPoolStorage + Send + Sync + 'static,
{
    pub fn torn_api<I>(&self, selector: I) -> KeyPoolExecutor<S>
    where
        I: IntoSelector<S::Key, S::Domain>,
    {
        KeyPoolExecutor::new(self, selector.into_selector())
    }
}

fn decode_error(buf: &[u8]) -> Result<Option<ApiError>, serde_json::Error> {
    if buf.starts_with(br#"{"error":{"#) {
        #[derive(Deserialize)]
        struct ErrorBody<'a> {
            code: u16,
            error: &'a str,
        }
        #[derive(Deserialize)]
        struct ErrorContainer<'a> {
            #[serde(borrow)]
            error: ErrorBody<'a>,
        }

        let error: ErrorContainer = serde_json::from_slice(buf)?;
        Ok(Some(crate::ApiError::new(
            error.error.code,
            error.error.error,
        )))
    } else {
        Ok(None)
    }
}

impl<S> Executor for KeyPoolExecutor<'_, S>
where
    S: KeyPoolStorage,
{
    type Error = S::Error;

    async fn execute<R>(
        &self,
        request: R,
    ) -> Result<torn_api::request::ApiResponse<R::Discriminant>, Self::Error>
    where
        R: torn_api::request::IntoRequest,
    {
        let request = request.into_request();

        self.execute_request(request).await
    }
}

#[cfg(test)]
mod test {
    use torn_api::executor::ExecutorExt;

    use crate::postgres;

    use super::*;

    #[sqlx::test]
    fn name(pool: sqlx::PgPool) {
        let (storage, _) = postgres::test::setup(pool).await;

        let pool = PoolBuilder::new(storage)
            .use_default_hooks()
            .comment("test_runner")
            .build();

        pool.torn_api(postgres::test::Domain::All)
            .faction()
            .basic(|b| b)
            .await
            .unwrap();
    }
}
