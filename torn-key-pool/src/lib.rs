#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

pub mod local;
pub mod send;

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use torn_api::ResponseError;

#[derive(Debug, Error)]
pub enum KeyPoolError<S, C>
where
    S: std::error::Error,
    C: std::error::Error,
{
    #[error("Key pool storage driver error: {0:?}")]
    Storage(#[source] Arc<S>),

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

    async fn acquire_many_keys(
        &self,
        domain: KeyDomain,
        number: i64,
    ) -> Result<Vec<Self::Key>, Self::Error>;

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
