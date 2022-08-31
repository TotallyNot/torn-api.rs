#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

use async_trait::async_trait;
use thiserror::Error;

use torn_api::prelude::*;

#[derive(Debug, Error)]
pub enum KeyPoolError<S>
where
    S: std::error::Error + std::fmt::Debug,
{
    #[error("Key pool storage driver error: {0:?}")]
    Storage(#[source] S),

    #[error(transparent)]
    Client(#[from] torn_api::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum KeyDomain {
    Public,
    User(i32),
    Faction(i32),
}

pub trait ApiKey {
    fn value(&self) -> &str;
}

#[async_trait(?Send)]
pub trait KeyPoolStorage {
    type Key: ApiKey;
    type Err: std::error::Error;

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

#[async_trait(?Send)]
impl<'client, C, S> ApiRequestExecutor<'client> for KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + 'static,
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
                Err(torn_api::Error::Api { code, .. }) => {
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
