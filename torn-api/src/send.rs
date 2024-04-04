use std::collections::HashMap;

use async_trait::async_trait;

use crate::{ApiClientError, ApiRequest, ApiResponse, ApiSelection, DirectExecutor};

pub struct ApiProvider<'a, C, E>
where
    C: ApiClient,
    E: RequestExecutor<C>,
{
    client: &'a C,
    executor: E,
}

impl<'a, C, E> ApiProvider<'a, C, E>
where
    C: ApiClient,
    E: RequestExecutor<C>,
{
    pub fn new(client: &'a C, executor: E) -> ApiProvider<'a, C, E> {
        Self { client, executor }
    }

    #[cfg(feature = "user")]
    pub async fn user<F>(&self, build: F) -> Result<crate::user::Response, E::Error>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::user::Selection>,
        ) -> crate::ApiRequestBuilder<crate::user::Selection>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    #[cfg(feature = "user")]
    pub async fn users<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<crate::user::Response, E::Error>>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::user::Selection>,
        ) -> crate::ApiRequestBuilder<crate::user::Selection>,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
        L: IntoIterator<Item = I>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(self.client, builder.request, Vec::from_iter(ids))
            .await
    }

    #[cfg(feature = "faction")]
    pub async fn faction<F>(&self, build: F) -> Result<crate::faction::Response, E::Error>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::faction::Selection>,
        ) -> crate::ApiRequestBuilder<crate::faction::Selection>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    #[cfg(feature = "faction")]
    pub async fn factions<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<crate::faction::Response, E::Error>>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::faction::Selection>,
        ) -> crate::ApiRequestBuilder<crate::faction::Selection>,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
        L: IntoIterator<Item = I>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(self.client, builder.request, Vec::from_iter(ids))
            .await
    }

    #[cfg(feature = "market")]
    pub async fn market<F>(&self, build: F) -> Result<crate::market::Response, E::Error>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::market::MarketSelection>,
        ) -> crate::ApiRequestBuilder<crate::market::MarketSelection>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    #[cfg(feature = "market")]
    pub async fn markets<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<crate::market::Response, E::Error>>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::market::MarketSelection>,
        ) -> crate::ApiRequestBuilder<crate::market::MarketSelection>,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
        L: IntoIterator<Item = I>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(self.client, builder.request, Vec::from_iter(ids))
            .await
    }

    #[cfg(feature = "torn")]
    pub async fn torn<F>(&self, build: F) -> Result<crate::torn::Response, E::Error>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::torn::Selection>,
        ) -> crate::ApiRequestBuilder<crate::torn::Selection>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    #[cfg(feature = "torn")]
    pub async fn torns<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<crate::torn::Response, E::Error>>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::torn::Selection>,
        ) -> crate::ApiRequestBuilder<crate::torn::Selection>,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
        L: IntoIterator<Item = I>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(self.client, builder.request, Vec::from_iter(ids))
            .await
    }

    #[cfg(feature = "key")]
    pub async fn key<F>(&self, build: F) -> Result<crate::key::Response, E::Error>
    where
        F: FnOnce(
            crate::ApiRequestBuilder<crate::key::Selection>,
        ) -> crate::ApiRequestBuilder<crate::key::Selection>,
    {
        let mut builder = crate::ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }
}

#[async_trait]
pub trait RequestExecutor<C>
where
    C: ApiClient,
{
    type Error: std::error::Error + Send + Sync;

    async fn execute<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        id: Option<String>,
    ) -> Result<A::Response, Self::Error>
    where
        A: ApiSelection;

    async fn execute_many<A, I>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        ids: Vec<I>,
    ) -> HashMap<I, Result<A::Response, Self::Error>>
    where
        A: ApiSelection,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync;
}

#[async_trait]
impl<C> RequestExecutor<C> for DirectExecutor<C>
where
    C: ApiClient,
{
    type Error = ApiClientError<C::Error>;

    async fn execute<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        id: Option<String>,
    ) -> Result<A::Response, Self::Error>
    where
        A: ApiSelection,
    {
        let url = request.url(&self.key, id.as_deref());

        let value = client.request(url).await.map_err(ApiClientError::Client)?;

        Ok(ApiResponse::from_value(value)?.into())
    }

    async fn execute_many<A, I>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        ids: Vec<I>,
    ) -> HashMap<I, Result<A::Response, Self::Error>>
    where
        A: ApiSelection,
        I: ToString + std::hash::Hash + std::cmp::Eq + Send + Sync,
    {
        let request_ref = &request;
        let tuples = futures::future::join_all(ids.into_iter().map(|i| async move {
            let id_string = i.to_string();
            let url = request_ref.url(&self.key, Some(&id_string));

            let value = client.request(url).await.map_err(ApiClientError::Client);

            (
                i,
                value.and_then(|v| {
                    ApiResponse::from_value(v)
                        .map(Into::into)
                        .map_err(Into::into)
                }),
            )
        }))
        .await;

        HashMap::from_iter(tuples)
    }
}

#[async_trait]
pub trait ApiClient: Send + Sync {
    type Error: std::error::Error + Sync + Send;

    async fn request(&self, url: String) -> Result<serde_json::Value, Self::Error>;

    fn torn_api<S>(&self, key: S) -> ApiProvider<Self, DirectExecutor<Self>>
    where
        Self: Sized,
        S: ToString,
    {
        ApiProvider::new(self, DirectExecutor::new(key.to_string()))
    }
}
