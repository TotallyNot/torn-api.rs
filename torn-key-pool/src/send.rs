use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use torn_api::{
    send::{ApiClient, ApiProvider, RequestExecutor},
    ApiRequest, ApiResponse, ApiSelection, ResponseError,
};

use crate::{
    ApiKey, IntoSelector, KeyAction, KeyDomain, KeyPoolError, KeyPoolExecutor, KeyPoolStorage,
    KeySelector, PoolOptions,
};

#[async_trait]
impl<'client, C, S> RequestExecutor<C> for KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    type Error = KeyPoolError<S::Error, C::Error>;

    async fn execute<A>(
        &self,
        client: &C,
        mut request: ApiRequest<A>,
        id: Option<String>,
    ) -> Result<A::Response, Self::Error>
    where
        A: ApiSelection,
    {
        request.comment = self.options.comment.clone();
        if let Some(hook) = self.options.hooks_before.get(&std::any::TypeId::of::<A>()) {
            let concrete = hook
                .downcast_ref::<BeforeHook<A, S::Key, S::Domain>>()
                .unwrap();

            (concrete.body)(&mut request, &self.selector);
        }
        loop {
            let key = self
                .storage
                .acquire_key(self.selector.clone())
                .await
                .map_err(KeyPoolError::Storage)?;
            let url = request.url(key.value(), id.as_deref());
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
                Ok(res) => {
                    let res = res.into();
                    if let Some(hook) = self.options.hooks_after.get(&std::any::TypeId::of::<A>()) {
                        let concrete = hook
                            .downcast_ref::<AfterHook<A, S::Key, S::Domain>>()
                            .unwrap();

                        match (concrete.body)(&res, &self.selector) {
                            Err(KeyAction::Delete) => {
                                self.storage
                                    .remove_key(key.selector())
                                    .await
                                    .map_err(KeyPoolError::Storage)?;
                                continue;
                            }
                            Err(KeyAction::RemoveDomain(domain)) => {
                                self.storage
                                    .remove_domain_from_key(key.selector(), domain)
                                    .await
                                    .map_err(KeyPoolError::Storage)?;
                                continue;
                            }
                            _ => (),
                        };
                    }
                    return Ok(res);
                }
            };
        }
    }

    async fn execute_many<A, I>(
        &self,
        client: &C,
        mut request: ApiRequest<A>,
        ids: Vec<I>,
    ) -> HashMap<I, Result<A::Response, Self::Error>>
    where
        A: ApiSelection,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
    {
        let keys = match self
            .storage
            .acquire_many_keys(self.selector.clone(), ids.len() as i64)
            .await
        {
            Ok(keys) => keys,
            Err(why) => {
                return ids
                    .into_iter()
                    .map(|i| (i, Err(Self::Error::Storage(why.clone()))))
                    .collect();
            }
        };

        request.comment = self.options.comment.clone();
        let request_ref = &request;

        let tuples =
            futures::future::join_all(std::iter::zip(ids, keys).map(|(id, mut key)| async move {
                let id_string = id.to_string();
                loop {
                    let url = request_ref.url(key.value(), Some(&id_string));
                    let value = match client.request(url).await {
                        Ok(v) => v,
                        Err(why) => return (id, Err(Self::Error::Client(why))),
                    };

                    match ApiResponse::from_value(value) {
                        Err(ResponseError::Api { code, reason }) => {
                            match self.storage.flag_key(key, code).await {
                                Ok(false) => {
                                    return (
                                        id,
                                        Err(KeyPoolError::Response(ResponseError::Api {
                                            code,
                                            reason,
                                        })),
                                    )
                                }
                                Ok(true) => (),
                                Err(why) => return (id, Err(KeyPoolError::Storage(why))),
                            }
                        }
                        Err(parsing_error) => {
                            return (id, Err(KeyPoolError::Response(parsing_error)))
                        }
                        Ok(res) => return (id, Ok(res.into())),
                    };

                    key = match self.storage.acquire_key(self.selector.clone()).await {
                        Ok(k) => k,
                        Err(why) => return (id, Err(Self::Error::Storage(why))),
                    };
                }
            }))
            .await;

        HashMap::from_iter(tuples)
    }
}

#[allow(clippy::type_complexity)]
pub struct BeforeHook<A, K, D>
where
    A: ApiSelection,
    K: ApiKey,
    D: KeyDomain,
{
    body: Box<dyn Fn(&mut ApiRequest<A>, &KeySelector<K, D>) + Send + Sync + 'static>,
}

#[allow(clippy::type_complexity)]
pub struct AfterHook<A, K, D>
where
    A: ApiSelection,
    K: ApiKey,
    D: KeyDomain,
{
    body: Box<
        dyn Fn(&A::Response, &KeySelector<K, D>) -> Result<(), crate::KeyAction<D>>
            + Send
            + Sync
            + 'static,
    >,
}

pub struct PoolBuilder<C, S>
where
    C: ApiClient,
    S: KeyPoolStorage,
{
    client: C,
    storage: S,
    options: crate::PoolOptions,
}

impl<C, S> PoolBuilder<C, S>
where
    C: ApiClient,
    S: KeyPoolStorage,
{
    pub fn new(client: C, storage: S) -> Self {
        Self {
            client,
            storage,
            options: Default::default(),
        }
    }

    pub fn comment(mut self, c: impl ToString) -> Self {
        self.options.comment = Some(c.to_string());
        self
    }

    pub fn hook_before<A>(
        mut self,
        hook: impl Fn(&mut ApiRequest<A>, &KeySelector<S::Key, S::Domain>) + Send + Sync + 'static,
    ) -> Self
    where
        A: ApiSelection + 'static,
    {
        self.options.hooks_before.insert(
            std::any::TypeId::of::<A>(),
            Box::new(BeforeHook {
                body: Box::new(hook),
            }),
        );
        self
    }

    pub fn hook_after<A>(
        mut self,
        hook: impl Fn(&A::Response, &KeySelector<S::Key, S::Domain>) -> Result<(), KeyAction<S::Domain>>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        A: ApiSelection + 'static,
    {
        self.options.hooks_after.insert(
            std::any::TypeId::of::<A>(),
            Box::new(AfterHook::<A, S::Key, S::Domain> {
                body: Box::new(hook),
            }),
        );
        self
    }

    pub fn build(self) -> KeyPool<C, S> {
        KeyPool {
            client: self.client,
            storage: self.storage,
            options: Arc::new(self.options),
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
    pub storage: S,
    options: Arc<PoolOptions>,
}

impl<C, S> KeyPool<C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + Send + Sync + 'static,
{
    pub fn torn_api<I>(&self, selector: I) -> ApiProvider<C, KeyPoolExecutor<C, S>>
    where
        I: IntoSelector<S::Key, S::Domain>,
    {
        ApiProvider::new(
            &self.client,
            KeyPoolExecutor::new(
                &self.storage,
                selector.into_selector(),
                self.options.clone(),
            ),
        )
    }
}

pub trait WithStorage {
    fn with_storage<'a, S, I>(
        &'a self,
        storage: &'a S,
        selector: I,
    ) -> ApiProvider<Self, KeyPoolExecutor<Self, S>>
    where
        Self: ApiClient + Sized,
        S: KeyPoolStorage + Send + Sync + 'static,
        I: IntoSelector<S::Key, S::Domain>,
    {
        ApiProvider::new(
            self,
            KeyPoolExecutor::new(storage, selector.into_selector(), Default::default()),
        )
    }
}

#[cfg(feature = "reqwest")]
impl WithStorage for reqwest::Client {}

#[cfg(all(test, feature = "postgres", feature = "reqwest"))]
mod test {
    use sqlx::PgPool;

    use super::*;
    use crate::{
        postgres::test::{setup, Domain},
        KeySelector,
    };

    #[sqlx::test]
    async fn test_pool_request(pool: PgPool) {
        let (storage, _) = setup(pool).await;
        let pool = PoolBuilder::new(reqwest::Client::default(), storage)
            .comment("api.rs")
            .build();

        let response = pool.torn_api(Domain::All).user(|b| b).await.unwrap();
        _ = response.profile().unwrap();
    }

    #[sqlx::test]
    async fn test_with_storage_request(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let response = reqwest::Client::new()
            .with_storage(&storage, Domain::All)
            .user(|b| b)
            .await
            .unwrap();
        _ = response.profile().unwrap();
    }

    #[sqlx::test]
    async fn before_hook(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let pool = PoolBuilder::new(reqwest::Client::default(), storage)
            .hook_before::<torn_api::user::UserSelection>(|req, _s| {
                req.selections.push("crimes");
            })
            .build();

        let response = pool.torn_api(Domain::All).user(|b| b).await.unwrap();
        _ = response.crimes().unwrap();
    }

    #[sqlx::test]
    async fn after_hook(pool: PgPool) {
        let (storage, _) = setup(pool).await;

        let pool = PoolBuilder::new(reqwest::Client::default(), storage)
            .hook_after::<torn_api::user::UserSelection>(|_res, _s| Err(KeyAction::Delete))
            .build();

        let key = pool.storage.read_key(KeySelector::Id(1)).await.unwrap();
        assert!(key.is_some());

        let response = pool.torn_api(Domain::All).user(|b| b).await;
        assert!(matches!(response, Err(KeyPoolError::Storage(_))));

        let key = pool.storage.read_key(KeySelector::Id(1)).await.unwrap();
        assert!(key.is_none());
    }
}
