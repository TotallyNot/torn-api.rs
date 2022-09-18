use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use torn_api::{
    local::{ApiClient, ApiProvider, RequestExecutor},
    ApiCategoryResponse, ApiRequest, ApiResponse, ResponseError,
};

use crate::{ApiKey, KeyDomain, KeyPoolError, KeyPoolExecutor, KeyPoolStorage};

#[async_trait(?Send)]
impl<'client, C, S> RequestExecutor<C> for KeyPoolExecutor<'client, C, S>
where
    C: ApiClient,
    S: KeyPoolStorage + 'static,
{
    type Error = KeyPoolError<S::Error, C::Error>;

    async fn execute<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        id: Option<i64>,
    ) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        loop {
            let key = self
                .storage
                .acquire_key(self.domain)
                .await
                .map_err(|e| KeyPoolError::Storage(Arc::new(e)))?;
            let url = request.url(key.value(), id);
            let value = client.request(url).await?;

            match ApiResponse::from_value(value) {
                Err(ResponseError::Api { code, reason }) => {
                    if !self
                        .storage
                        .flag_key(key, code)
                        .await
                        .map_err(Arc::new)
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

    async fn execute_many<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        ids: Vec<i64>,
    ) -> HashMap<i64, Result<A, Self::Error>>
    where
        A: ApiCategoryResponse,
    {
        let keys = match self
            .storage
            .acquire_many_keys(self.domain, ids.len() as i64)
            .await
        {
            Ok(keys) => keys,
            Err(why) => {
                let shared = Arc::new(why);
                return ids
                    .into_iter()
                    .map(|i| (i, Err(Self::Error::Storage(shared.clone()))))
                    .collect();
            }
        };

        let request_ref = &request;

        futures::future::join_all(std::iter::zip(ids, keys).map(|(id, mut key)| async move {
            loop {
                let url = request_ref.url(key.value(), Some(id));
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
                            Err(why) => return (id, Err(KeyPoolError::Storage(Arc::new(why)))),
                        }
                    }
                    Err(parsing_error) => return (id, Err(KeyPoolError::Response(parsing_error))),
                    Ok(res) => return (id, Ok(A::from_response(res))),
                };

                key = match self.storage.acquire_key(self.domain).await {
                    Ok(k) => k,
                    Err(why) => return (id, Err(Self::Error::Storage(Arc::new(why)))),
                };
            }
        }))
        .await
        .into_iter()
        .collect()
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
}

#[cfg(feature = "awc")]
impl WithStorage for awc::Client {}
