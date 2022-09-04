#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

use async_trait::async_trait;
use thiserror::Error;

use torn_api::{
    ApiCategoryResponse, ApiClient, ApiProvider, ApiRequest, ApiResponse, RequestExecutor,
    ResponseError, ThreadSafeApiClient, ThreadSafeApiProvider, ThreadSafeRequestExecutor,
};

#[derive(Debug, Error)]
pub enum KeyPoolError<S, C>
where
    S: std::error::Error,
    C: std::error::Error,
{
    #[error("Key pool storage driver error: {0:?}")]
    Storage(#[source] S),

    #[error(transparent)]
    Client(#[from] C),

    #[error(transparent)]
    Response(ResponseError),
}

#[derive(Debug, Clone, Copy)]
pub enum KeyDomain {
    Public,
    User(i32),
    Faction(i32),
}

pub trait ApiKey: Sync + Send {
    fn value(&self) -> &str;
}

#[async_trait]
pub trait KeyPoolStorage {
    type Key: ApiKey;
    type Error: std::error::Error + Sync + Send;

    async fn acquire_key(&self, domain: KeyDomain) -> Result<Self::Key, Self::Error>;

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    storage: &'a S,
    domain: KeyDomain,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C, S> KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    pub fn new(storage: &'a S, domain: KeyDomain) -> Self {
        Self {
            storage,
            domain,
            _marker: std::marker::PhantomData,
        }
    }
}

#[async_trait(?Send)]
impl<'client, C, S> RequestExecutor<C> for KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + 'static,
{
    type Error = KeyPoolError<S::Error, C::Error>;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        loop {
            let key = self
                .storage
                .acquire_key(self.domain)
                .await
                .map_err(KeyPoolError::Storage)?;
            let url = request.url(key.value());
            let value = client.request(url).await?;

            match ApiResponse::from_value(value) {
                Err(ResponseError::Api { code, reason }) => {
                    if !self
                        .storage
                        .flag_key(key, code)
                        .await
                        .map_err(KeyPoolError::Storage)?
                    {
                        return Err(KeyPoolError::Response(ResponseError::Api { code, reason }));
                    }
                }
                Err(parsing_error) => return Err(KeyPoolError::Response(parsing_error)),
                Ok(res) => return Ok(A::from_response(res)),
            };
        }
    }
}

#[async_trait]
impl<'client, C, S> ThreadSafeRequestExecutor<C> for KeyPoolExecutor<'client, C, S>
where
    C: ThreadSafeApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    type Error = KeyPoolError<S::Error, C::Error>;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        loop {
            let key = self
                .storage
                .acquire_key(self.domain)
                .await
                .map_err(KeyPoolError::Storage)?;
            let url = request.url(key.value());
            let value = client.request(url).await?;

            match ApiResponse::from_value(value) {
                Err(ResponseError::Api { code, reason }) => {
                    if !self
                        .storage
                        .flag_key(key, code)
                        .await
                        .map_err(KeyPoolError::Storage)?
                    {
                        return Err(KeyPoolError::Response(ResponseError::Api { code, reason }));
                    }
                }
                Err(parsing_error) => return Err(KeyPoolError::Response(parsing_error)),
                Ok(res) => return Ok(A::from_response(res)),
            };
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyPool<C, S>
where
    C: ApiClient,
    S: KeyPoolStorage,
{
    client: C,
    storage: S,
}

impl<C, S> KeyPool<C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + 'static,
{
    pub fn new(client: C, storage: S) -> Self {
        Self { client, storage }
    }

    pub fn torn_api(&self, domain: KeyDomain) -> ApiProvider<C, KeyPoolExecutor<C, S>> {
        ApiProvider::new(&self.client, KeyPoolExecutor::new(&self.storage, domain))
    }
}

#[derive(Clone, Debug)]
pub struct ThreadSafeKeyPool<C, S>
where
    C: ThreadSafeApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    client: C,
    storage: S,
}

impl<C, S> ThreadSafeKeyPool<C, S>
where
    C: ThreadSafeApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    pub fn new(client: C, storage: S) -> Self {
        Self { client, storage }
    }

    pub fn torn_api(&self, domain: KeyDomain) -> ThreadSafeApiProvider<C, KeyPoolExecutor<C, S>> {
        ThreadSafeApiProvider::new(&self.client, KeyPoolExecutor::new(&self.storage, domain))
    }
}

pub trait WithStorage {
    fn with_storage<'a, S>(
        &'a self,
        storage: &'a S,
        domain: KeyDomain,
    ) -> ApiProvider<Self, KeyPoolExecutor<Self, S>>
    where
        Self: ApiClient + Sized,
        S: KeyPoolStorage + 'static,
    {
        ApiProvider::new(self, KeyPoolExecutor::new(storage, domain))
    }

    fn with_storage_sync<'a, S>(
        &'a self,
        storage: &'a S,
        domain: KeyDomain,
    ) -> ThreadSafeApiProvider<Self, KeyPoolExecutor<Self, S>>
    where
        Self: ThreadSafeApiClient + Sized,
        S: KeyPoolStorage + Send + Sync + 'static,
    {
        ThreadSafeApiProvider::new(self, KeyPoolExecutor::new(storage, domain))
    }
}

#[cfg(feature = "reqwest")]
impl WithStorage for reqwest::Client {}

#[cfg(feature = "awc")]
impl WithStorage for awc::Client {}
