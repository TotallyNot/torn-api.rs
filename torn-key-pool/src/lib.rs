#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

#[cfg(feature = "postgres")]
pub mod postgres;

// pub mod local;
pub mod send;

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use torn_api::ResponseError;

#[derive(Debug, Error)]
pub enum KeyPoolError<S, C>
where
    S: std::error::Error + Clone,
    C: std::error::Error,
{
    #[error("Key pool storage driver error: {0:?}")]
    Storage(#[source] S),

    #[error(transparent)]
    Client(#[from] C),

    #[error(transparent)]
    Response(ResponseError),
}

impl<S, C> KeyPoolError<S, C>
where
    S: std::error::Error + Clone,
    C: std::error::Error,
{
    #[inline(always)]
    pub fn api_code(&self) -> Option<u8> {
        match self {
            Self::Response(why) => why.api_code(),
            _ => None,
        }
    }
}

pub trait ApiKey: Sync + Send + std::fmt::Debug + Clone + 'static {
    type IdType: PartialEq + Eq + std::hash::Hash + Send + Sync + std::fmt::Debug + Clone;

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

pub enum KeyAction<D>
where
    D: KeyDomain,
{
    Delete,
    RemoveDomain(D),
    Timeout(chrono::Duration),
}

#[async_trait]
pub trait KeyPoolStorage {
    type Key: ApiKey;
    type Domain: KeyDomain;
    type Error: std::error::Error + Sync + Send + Clone;

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

#[derive(Debug, Default)]
struct PoolOptions {
    comment: Option<String>,
    hooks_before: std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
    hooks_after: std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub struct KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    storage: &'a S,
    options: Arc<PoolOptions>,
    selector: KeySelector<S::Key, S::Domain>,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C, S> KeyPoolExecutor<'a, C, S>
where
    S: KeyPoolStorage,
{
    fn new(
        storage: &'a S,
        selector: KeySelector<S::Key, S::Domain>,
        options: Arc<PoolOptions>,
    ) -> Self {
        Self {
            storage,
            selector,
            options,
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(all(test, feature = "postgres"))]
mod test {}
