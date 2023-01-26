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

pub trait ApiKey: Sync + Send {
    fn value(&self) -> &str;
}

pub trait KeyDomain: Clone + std::fmt::Debug + Send + Sync {
    fn fallback(&self) -> Option<Self> {
        None
    }
}

impl<T> KeyDomain for T where T: Clone + std::fmt::Debug + Send + Sync {}

#[async_trait]
pub trait KeyPoolStorage {
    type Key: ApiKey;
    type Domain: KeyDomain;
    type Error: std::error::Error + Sync + Send;

    async fn acquire_key(&self, domain: Self::Domain) -> Result<Self::Key, Self::Error>;

    async fn acquire_many_keys(
        &self,
        domain: Self::Domain,
        number: i64,
    ) -> Result<Vec<Self::Key>, Self::Error>;

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Error>;

    async fn store_key(
        &self,
        user_id: i32,
        key: String,
        domains: Vec<Self::Domain>,
    ) -> Result<Self::Key, Self::Error>;

    async fn read_key(&self, key: String) -> Result<Self::Key, Self::Error>;

    async fn read_user_keys(&self, user_id: i32) -> Result<Vec<Self::Key>, Self::Error>;

    async fn remove_key(&self, key: String) -> Result<Self::Key, Self::Error>;

    async fn add_domain_to_key(
        &self,
        key: String,
        domain: Self::Domain,
    ) -> Result<Self::Key, Self::Error>;

    async fn remove_domain_from_key(
        &self,
        key: String,
        domain: Self::Domain,
    ) -> Result<Self::Key, Self::Error>;

    async fn set_domains_for_key(
        &self,
        key: String,
        domains: Vec<Self::Domain>,
    ) -> Result<Self::Key, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    storage: &'a S,
    comment: Option<&'a str>,
    domain: S::Domain,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C, S> KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    pub fn new(storage: &'a S, domain: S::Domain, comment: Option<&'a str>) -> Self {
        Self {
            storage,
            domain,
            comment,
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(all(test, feature = "postgres"))]
mod test {}
