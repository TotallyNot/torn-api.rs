#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use futures::{future::BoxFuture, FutureExt, Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;
use tokio_stream::StreamExt as TokioStreamExt;
use torn_api::{
    executor::{BulkExecutor, Executor},
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

impl<K, D> From<&str> for KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    fn from(value: &str) -> Self {
        Self::Key(value.to_owned())
    }
}

impl<K, D> From<D> for KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    fn from(value: D) -> Self {
        Self::Has(vec![value])
    }
}

impl<K, D> From<&[D]> for KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    fn from(value: &[D]) -> Self {
        Self::Has(value.to_vec())
    }
}

impl<K, D> From<Vec<D>> for KeySelector<K, D>
where
    K: ApiKey,
    D: KeyDomain,
{
    fn from(value: Vec<D>) -> Self {
        Self::Has(value)
    }
}

pub trait IntoSelector<K, D>: Send
where
    K: ApiKey,
    D: KeyDomain,
{
    fn into_selector(self) -> KeySelector<K, D>;
}

impl<K, D, T> IntoSelector<K, D> for T
where
    K: ApiKey,
    D: KeyDomain,
    T: Into<KeySelector<K, D>> + Send,
{
    fn into_selector(self) -> KeySelector<K, D> {
        self.into()
    }
}

pub trait KeyPoolError:
    From<reqwest::Error> + From<serde_json::Error> + From<torn_api::ApiError> + From<Arc<Self>> + Send
{
}

impl<T> KeyPoolError for T where
    T: From<reqwest::Error>
        + From<serde_json::Error>
        + From<torn_api::ApiError>
        + From<Arc<Self>>
        + Send
{
}

pub trait KeyPoolStorage: Send + Sync {
    type Key: ApiKey;
    type Domain: KeyDomain;
    type Error: KeyPoolError;

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
            inner: Arc::new(KeyPoolInner {
                client: self.client,
                storage: self.storage,
                options: self.options,
            }),
        }
    }
}

struct KeyPoolInner<S>
where
    S: KeyPoolStorage,
{
    pub client: reqwest::Client,
    pub storage: S,
    pub options: PoolOptions<S>,
}

impl<S> KeyPoolInner<S>
where
    S: KeyPoolStorage,
{
    async fn execute_with_key(
        &self,
        key: &S::Key,
        request: &ApiRequest,
    ) -> Result<RequestResult, S::Error> {
        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("ApiKey {}", key.value())).unwrap(),
        );

        let resp = self
            .client
            .get(request.url())
            .headers(headers)
            .send()
            .await?;

        let status = resp.status();

        let bytes = resp.bytes().await?;

        if let Some(err) = decode_error(&bytes)? {
            if let Some(handler) = self.options.error_hooks.get(&err.code()) {
                let retry = (*handler)(&self.storage, key).await?;

                if retry {
                    return Ok(RequestResult::Retry);
                }
            }
            Err(err.into())
        } else {
            Ok(RequestResult::Response(ApiResponse {
                body: Some(bytes),
                status,
            }))
        }
    }

    async fn execute_request(
        &self,
        selector: KeySelector<S::Key, S::Domain>,
        request: ApiRequest,
    ) -> Result<ApiResponse, S::Error> {
        loop {
            let key = self.storage.acquire_key(selector.clone()).await?;
            match self.execute_with_key(&key, &request).await {
                Ok(RequestResult::Response(resp)) => return Ok(resp),
                Ok(RequestResult::Retry) => (),
                Err(why) => return Err(why),
            }
        }
    }

    async fn execute_bulk_requests<D, T: IntoIterator<Item = (D, ApiRequest)>>(
        &self,
        selector: KeySelector<S::Key, S::Domain>,
        requests: T,
    ) -> impl Stream<Item = (D, Result<ApiResponse, S::Error>)> + use<'_, D, S, T> {
        let requests: Vec<_> = requests.into_iter().collect();

        let keys: Vec<_> = match self
            .storage
            .acquire_many_keys(selector.clone(), requests.len() as i64)
            .await
        {
            Ok(keys) => keys.into_iter().map(Ok).collect(),
            Err(why) => {
                let why = Arc::new(why);
                std::iter::repeat_n(why, requests.len())
                    .map(|e| Err(S::Error::from(e)))
                    .collect()
            }
        };

        StreamExt::map(
            futures::stream::iter(std::iter::zip(requests, keys)),
            move |((discriminant, request), mut maybe_key)| {
                let selector = selector.clone();
                async move {
                    loop {
                        let key = match maybe_key {
                            Ok(key) => key,
                            Err(why) => return (discriminant, Err(why)),
                        };
                        match self.execute_with_key(&key, &request).await {
                            Ok(RequestResult::Response(resp)) => return (discriminant, Ok(resp)),
                            Ok(RequestResult::Retry) => (),
                            Err(why) => return (discriminant, Err(why)),
                        }
                        maybe_key = self.storage.acquire_key(selector.clone()).await;
                    }
                }
            },
        )
        .buffer_unordered(25)
    }
}

pub struct KeyPool<S>
where
    S: KeyPoolStorage,
{
    inner: Arc<KeyPoolInner<S>>,
}

enum RequestResult {
    Response(ApiResponse),
    Retry,
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

    pub fn throttled_torn_api<I>(
        &self,
        selector: I,
        distance: Duration,
    ) -> ThrottledKeyPoolExecutor<S>
    where
        I: IntoSelector<S::Key, S::Domain>,
    {
        ThrottledKeyPoolExecutor::new(self, selector.into_selector(), distance)
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

pub struct KeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pool: &'p KeyPoolInner<S>,
    selector: KeySelector<S::Key, S::Domain>,
}

impl<'p, S> KeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pub fn new(pool: &'p KeyPool<S>, selector: KeySelector<S::Key, S::Domain>) -> Self {
        Self {
            pool: &pool.inner,
            selector,
        }
    }
}

impl<S> Executor for KeyPoolExecutor<'_, S>
where
    S: KeyPoolStorage + 'static,
{
    type Error = S::Error;

    async fn execute<R>(self, request: R) -> (R::Discriminant, Result<ApiResponse, Self::Error>)
    where
        R: torn_api::request::IntoRequest,
    {
        let (d, request) = request.into_request();

        (d, self.pool.execute_request(self.selector, request).await)
    }
}

impl<'p, S> BulkExecutor<'p> for KeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage + 'static,
{
    type Error = S::Error;

    fn execute<R>(
        self,
        requests: impl IntoIterator<Item = R>,
    ) -> impl futures::Stream<Item = (R::Discriminant, Result<ApiResponse, Self::Error>)>
    where
        R: torn_api::request::IntoRequest,
    {
        self.pool
            .execute_bulk_requests(
                self.selector.clone(),
                requests.into_iter().map(|r| r.into_request()),
            )
            .into_stream()
            .flatten()
    }
}

pub struct ThrottledKeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pool: &'p KeyPoolInner<S>,
    selector: KeySelector<S::Key, S::Domain>,
    distance: Duration,
}

impl<S> Clone for ThrottledKeyPoolExecutor<'_, S>
where
    S: KeyPoolStorage,
{
    fn clone(&self) -> Self {
        Self {
            pool: self.pool,
            selector: self.selector.clone(),
            distance: self.distance,
        }
    }
}

impl<S> ThrottledKeyPoolExecutor<'_, S>
where
    S: KeyPoolStorage,
{
    async fn execute_request(self, request: ApiRequest) -> Result<ApiResponse, S::Error> {
        self.pool.execute_request(self.selector, request).await
    }
}

impl<'p, S> ThrottledKeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage,
{
    pub fn new(
        pool: &'p KeyPool<S>,
        selector: KeySelector<S::Key, S::Domain>,
        distance: Duration,
    ) -> Self {
        Self {
            pool: &pool.inner,
            selector,
            distance,
        }
    }
}

impl<'p, S> BulkExecutor<'p> for ThrottledKeyPoolExecutor<'p, S>
where
    S: KeyPoolStorage + 'static,
{
    type Error = S::Error;

    fn execute<R>(
        self,
        requests: impl IntoIterator<Item = R>,
    ) -> impl futures::Stream<Item = (R::Discriminant, Result<ApiResponse, Self::Error>)>
    where
        R: torn_api::request::IntoRequest,
    {
        StreamExt::map(
            futures::stream::iter(requests).throttle(self.distance),
            move |r| {
                let this = self.clone();
                async move {
                    let (d, request) = r.into_request();
                    let result = this.execute_request(request).await;
                    (d, result)
                }
            },
        )
        .buffer_unordered(25)
    }
}

#[cfg(test)]
#[cfg(feature = "postgres")]
mod test {
    use torn_api::executor::{BulkExecutorExt, ExecutorExt};

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

    #[sqlx::test]
    fn bulk(pool: sqlx::PgPool) {
        let (storage, _) = postgres::test::setup(pool).await;

        let pool = PoolBuilder::new(storage)
            .use_default_hooks()
            .comment("test_runner")
            .build();

        let responses = pool
            .torn_api(postgres::test::Domain::All)
            .faction_bulk()
            .basic_for_id(vec![19.into(), 89.into()], |b| b);
        let mut responses: Vec<_> = StreamExt::collect(responses).await;

        let (_id1, basic1) = responses.pop().unwrap();
        basic1.unwrap();

        let (_id2, basic2) = responses.pop().unwrap();
        basic2.unwrap();
    }

    #[sqlx::test]
    fn bulk_trottled(pool: sqlx::PgPool) {
        let (storage, _) = postgres::test::setup(pool).await;

        let pool = PoolBuilder::new(storage)
            .use_default_hooks()
            .comment("test_runner")
            .build();

        let responses = pool
            .throttled_torn_api(postgres::test::Domain::All, Duration::from_millis(500))
            .faction_bulk()
            .basic_for_id(vec![19.into(), 89.into()], |b| b);
        let mut responses: Vec<_> = StreamExt::collect(responses).await;

        let (_id1, basic1) = responses.pop().unwrap();
        basic1.unwrap();

        let (_id2, basic2) = responses.pop().unwrap();
        basic2.unwrap();
    }
}
