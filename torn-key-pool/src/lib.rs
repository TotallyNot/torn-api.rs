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
    type IdType: PartialEq + Eq + std::hash::Hash + Send + Sync;

    fn value(&self) -> &str;

    fn id(&self) -> Self::IdType;
}

pub trait KeyDomain: Clone + std::fmt::Debug + Send + Sync {
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
    Has(D),
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
            Self::Has(domain) => domain.fallback().map(Self::Has),
            Self::OneOf(domains) => {
                let fallbacks: Vec<_> = domains.into_iter().filter_map(|d| d.fallback()).collect();
                if fallbacks.is_empty() {
                    None
                } else {
                    Some(Self::OneOf(fallbacks))
                }
            }
        }
    }
}

pub trait IntoSelector<K, D>: Send + Sync
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
        KeySelector::Has(self)
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

#[async_trait]
pub trait KeyPoolStorage {
    type Key: ApiKey;
    type Domain: KeyDomain;
    type Error: std::error::Error + Sync + Send;

    async fn acquire_key<S>(&self, selector: S) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn acquire_many_keys<S>(
        &self,
        selector: S,
        number: i64,
    ) -> Result<Vec<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn flag_key(&self, key: Self::Key, code: u8) -> Result<bool, Self::Error>;

    async fn store_key(
        &self,
        user_id: i32,
        key: String,
        domains: Vec<Self::Domain>,
    ) -> Result<Self::Key, Self::Error>;

    async fn read_key<S>(&self, selector: S) -> Result<Option<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn read_keys<S>(&self, selector: S) -> Result<Vec<Self::Key>, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn remove_key<S>(&self, selector: S) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn add_domain_to_key<S>(
        &self,
        selector: S,
        domain: Self::Domain,
    ) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn remove_domain_from_key<S>(
        &self,
        selector: S,
        domain: Self::Domain,
    ) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;

    async fn set_domains_for_key<S>(
        &self,
        selector: S,
        domains: Vec<Self::Domain>,
    ) -> Result<Self::Key, Self::Error>
    where
        S: IntoSelector<Self::Key, Self::Domain>;
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
