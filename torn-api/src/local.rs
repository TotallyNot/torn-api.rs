use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    faction, torn, user, ApiCategoryResponse, ApiClientError, ApiRequest, ApiRequestBuilder,
    ApiResponse, DirectExecutor,
};

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

    pub async fn user<F>(&self, build: F) -> Result<user::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<user::Response>) -> ApiRequestBuilder<user::Response>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    pub async fn users<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<user::Response, E::Error>>
    where
        F: FnOnce(ApiRequestBuilder<user::Response>) -> ApiRequestBuilder<user::Response>,
        I: num_traits::AsPrimitive<i64> + std::hash::Hash + std::cmp::Eq,
        i64: num_traits::AsPrimitive<I>,
        L: IntoIterator<Item = I>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(
                self.client,
                builder.request,
                ids.into_iter().map(|i| i.as_()).collect(),
            )
            .await
            .into_iter()
            .map(|(i, r)| (num_traits::AsPrimitive::as_(i), r))
            .collect()
    }

    pub async fn faction<F>(&self, build: F) -> Result<faction::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<faction::Response>) -> ApiRequestBuilder<faction::Response>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    pub async fn factions<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<faction::Response, E::Error>>
    where
        F: FnOnce(ApiRequestBuilder<faction::Response>) -> ApiRequestBuilder<faction::Response>,
        I: num_traits::AsPrimitive<i64> + std::hash::Hash + std::cmp::Eq,
        i64: num_traits::AsPrimitive<I>,
        L: IntoIterator<Item = I>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(
                self.client,
                builder.request,
                ids.into_iter().map(|i| i.as_()).collect(),
            )
            .await
            .into_iter()
            .map(|(i, r)| (num_traits::AsPrimitive::as_(i), r))
            .collect()
    }

    pub async fn torn<F>(&self, build: F) -> Result<torn::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<torn::Response>) -> ApiRequestBuilder<torn::Response>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute(self.client, builder.request, builder.id)
            .await
    }

    pub async fn torns<F, L, I>(
        &self,
        ids: L,
        build: F,
    ) -> HashMap<I, Result<torn::Response, E::Error>>
    where
        F: FnOnce(ApiRequestBuilder<torn::Response>) -> ApiRequestBuilder<torn::Response>,
        I: num_traits::AsPrimitive<i64> + std::hash::Hash + std::cmp::Eq,
        i64: num_traits::AsPrimitive<I>,
        L: IntoIterator<Item = I>,
    {
        let mut builder = ApiRequestBuilder::default();
        builder = build(builder);

        self.executor
            .execute_many(
                self.client,
                builder.request,
                ids.into_iter().map(|i| i.as_()).collect(),
            )
            .await
            .into_iter()
            .map(|(i, r)| (num_traits::AsPrimitive::as_(i), r))
            .collect()
    }
}

#[async_trait(?Send)]
pub trait RequestExecutor<C>
where
    C: ApiClient,
{
    type Error: std::error::Error;

    async fn execute<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        id: Option<i64>,
    ) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse;

    async fn execute_many<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        ids: Vec<i64>,
    ) -> HashMap<i64, Result<A, Self::Error>>
    where
        A: ApiCategoryResponse;
}

#[async_trait(?Send)]
impl<C> RequestExecutor<C> for DirectExecutor<C>
where
    C: ApiClient,
{
    type Error = ApiClientError<C::Error>;

    async fn execute<A>(
        &self,
        client: &C,
        request: ApiRequest<A>,
        id: Option<i64>,
    ) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        let url = request.url(&self.key, id);

        let value = client.request(url).await.map_err(ApiClientError::Client)?;

        Ok(A::from_response(ApiResponse::from_value(value)?))
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
        let request_ref = &request;
        futures::future::join_all(ids.into_iter().map(|i| async move {
            let url = request_ref.url(&self.key, Some(i));

            let value = client.request(url).await.map_err(ApiClientError::Client);

            (
                i,
                value
                    .and_then(|v| ApiResponse::from_value(v).map_err(Into::into))
                    .map(A::from_response),
            )
        }))
        .await
        .into_iter()
        .collect()
    }
}

#[async_trait(?Send)]
pub trait ApiClient {
    type Error: std::error::Error;

    async fn request(&self, url: String) -> Result<serde_json::Value, Self::Error>;

    fn torn_api<S>(&self, key: S) -> ApiProvider<Self, DirectExecutor<Self>>
    where
        Self: Sized,
        S: ToString,
    {
        ApiProvider::new(self, DirectExecutor::new(key.to_string()))
    }
}
