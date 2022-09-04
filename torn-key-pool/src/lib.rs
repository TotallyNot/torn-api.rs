#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

use async_trait::async_trait;
use thiserror::Error;

use torn_api::prelude::*;

#[derive(Debug, Error)]
pub enum KeyPoolError<S>
where
    S: Sync + Send + std::error::Error,
{
    #[error("Key pool storage driver error: {0:?}")]
    Storage(#[source] S),

    #[error(transparent)]
    Client(#[from] torn_api::ClientError),
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
    type Err: Sync + Send + std::error::Error;

    async fn acquire_key(&self, domain: KeyDomain) -> Result<Self::Key, Self::Err>;

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Err>;
}

#[derive(Debug, Clone)]
pub struct KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage,
{
    client: &'client C,
    storage: &'client S,
    domain: KeyDomain,
}

impl<'client, C, S> KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage,
{
    pub fn new(client: &'client C, storage: &'client S, domain: KeyDomain) -> Self {
        Self {
            client,
            storage,
            domain,
        }
    }
}

#[cfg_attr(feature = "awc", async_trait(?Send))]
#[cfg_attr(not(feature = "awc"), async_trait)]
impl<'client, C, S> ApiRequestExecutor<'client> for KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    type Err = KeyPoolError<S::Err>;

    async fn excute<A>(&self, request: torn_api::ApiRequest<A>) -> Result<A, Self::Err>
    where
        A: torn_api::ApiCategoryResponse,
    {
        loop {
            let key = self
                .storage
                .acquire_key(self.domain)
                .await
                .map_err(KeyPoolError::Storage)?;
            let url = request.url(key.value());
            let res = self.client.request(url).await;

            match res {
                Err(torn_api::ClientError::Api { code, .. }) => {
                    if !self
                        .storage
                        .flag_key(key, code)
                        .await
                        .map_err(KeyPoolError::Storage)?
                    {
                        panic!();
                    }
                }
                _ => return res.map(A::from_response).map_err(KeyPoolError::Client),
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
    S: KeyPoolStorage,
{
    pub fn new(client: C, storage: S) -> Self {
        Self { client, storage }
    }

    pub fn torn_api(&self, domain: KeyDomain) -> KeyPoolExecutor<C, S> {
        KeyPoolExecutor::new(&self.client, &self.storage, domain)
    }
}

pub trait KeyPoolClient: ApiClient {
    fn with_pool<'a, S>(&'a self, domain: KeyDomain, storage: &'a S) -> KeyPoolExecutor<Self, S>
    where
        Self: Sized,
        S: KeyPoolStorage,
    {
        KeyPoolExecutor::new(self, storage, domain)
    }
}

#[cfg(feature = "reqwest")]
impl KeyPoolClient for reqwest::Client {}

#[cfg(feature = "awc")]
impl KeyPoolClient for awc::Client {}
