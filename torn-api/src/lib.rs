#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

pub mod faction;
pub mod user;

#[cfg(feature = "awc")]
pub mod awc;

#[cfg(feature = "reqwest")]
pub mod reqwest;

mod de_util;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::de::{DeserializeOwned, Error as DeError};
use thiserror::Error;

pub struct ApiResponse {
    value: serde_json::Value,
}

#[derive(Error, Debug)]
pub enum ResponseError {
    #[error("API: {reason}")]
    Api { code: u8, reason: String },

    #[error(transparent)]
    Parsing(#[from] serde_json::Error),
}

impl ApiResponse {
    pub fn from_value(mut value: serde_json::Value) -> Result<Self, ResponseError> {
        #[derive(serde::Deserialize)]
        struct ApiErrorDto {
            code: u8,
            #[serde(rename = "error")]
            reason: String,
        }
        match value.get_mut("error") {
            Some(error) => {
                let dto: ApiErrorDto = serde_json::from_value(error.take())?;
                Err(ResponseError::Api {
                    code: dto.code,
                    reason: dto.reason,
                })
            }
            None => Ok(Self { value }),
        }
    }

    fn decode<D>(&self) -> serde_json::Result<D>
    where
        D: DeserializeOwned,
    {
        serde_json::from_value(self.value.clone())
    }

    fn decode_field<D>(&self, field: &'static str) -> serde_json::Result<D>
    where
        D: DeserializeOwned,
    {
        let value = self
            .value
            .get(field)
            .ok_or_else(|| serde_json::Error::missing_field(field))?
            .clone();

        serde_json::from_value(value)
    }
}

pub trait ApiSelection {
    fn raw_value(&self) -> &'static str;

    fn category() -> &'static str;
}

pub trait ApiCategoryResponse: Send + Sync {
    type Selection: ApiSelection;

    fn from_response(response: ApiResponse) -> Self;
}

#[async_trait]
pub trait ThreadSafeApiClient: Send + Sync {
    type Error: std::error::Error + Sync + Send;

    async fn request(&self, url: String) -> Result<serde_json::Value, Self::Error>;

    fn torn_api<S>(&self, key: S) -> ThreadSafeApiProvider<Self, DirectExecutor<Self>>
    where
        Self: Sized,
        S: ToString,
    {
        ThreadSafeApiProvider::new(self, DirectExecutor::new(key.to_string()))
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

#[async_trait(?Send)]
pub trait RequestExecutor<C>
where
    C: ApiClient,
{
    type Error: std::error::Error;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse;
}

#[async_trait]
pub trait ThreadSafeRequestExecutor<C>
where
    C: ThreadSafeApiClient,
{
    type Error: std::error::Error + Send + Sync;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse;
}

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
        let mut builder = ApiRequestBuilder::<user::Response>::new();
        builder = build(builder);

        self.executor.execute(self.client, builder.request).await
    }

    pub async fn faction<F>(&self, build: F) -> Result<faction::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<faction::Response>) -> ApiRequestBuilder<faction::Response>,
    {
        let mut builder = ApiRequestBuilder::<faction::Response>::new();
        builder = build(builder);

        self.executor.execute(self.client, builder.request).await
    }
}

pub struct ThreadSafeApiProvider<'a, C, E>
where
    C: ThreadSafeApiClient,
    E: ThreadSafeRequestExecutor<C>,
{
    client: &'a C,
    executor: E,
}

impl<'a, C, E> ThreadSafeApiProvider<'a, C, E>
where
    C: ThreadSafeApiClient,
    E: ThreadSafeRequestExecutor<C>,
{
    pub fn new(client: &'a C, executor: E) -> ThreadSafeApiProvider<'a, C, E> {
        Self { client, executor }
    }

    pub async fn user<F>(&self, build: F) -> Result<user::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<user::Response>) -> ApiRequestBuilder<user::Response>,
    {
        let mut builder = ApiRequestBuilder::<user::Response>::new();
        builder = build(builder);

        self.executor.execute(self.client, builder.request).await
    }

    pub async fn faction<F>(&self, build: F) -> Result<faction::Response, E::Error>
    where
        F: FnOnce(ApiRequestBuilder<faction::Response>) -> ApiRequestBuilder<faction::Response>,
    {
        let mut builder = ApiRequestBuilder::<faction::Response>::new();
        builder = build(builder);

        self.executor.execute(self.client, builder.request).await
    }
}

pub struct DirectExecutor<C> {
    key: String,
    _marker: std::marker::PhantomData<C>,
}

impl<C> DirectExecutor<C> {
    fn new(key: String) -> Self {
        Self {
            key,
            _marker: std::marker::PhantomData,
        }
    }
}

#[derive(Error, Debug)]
pub enum ApiClientError<C>
where
    C: std::error::Error,
{
    #[error(transparent)]
    Client(C),

    #[error(transparent)]
    Response(#[from] ResponseError),
}

#[async_trait(?Send)]
impl<C> RequestExecutor<C> for DirectExecutor<C>
where
    C: ApiClient,
{
    type Error = ApiClientError<C::Error>;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        let url = request.url(&self.key);

        let value = client.request(url).await.map_err(ApiClientError::Client)?;

        Ok(A::from_response(ApiResponse::from_value(value)?))
    }
}

#[async_trait]
impl<C> ThreadSafeRequestExecutor<C> for DirectExecutor<C>
where
    C: ThreadSafeApiClient,
{
    type Error = ApiClientError<C::Error>;

    async fn execute<A>(&self, client: &C, request: ApiRequest<A>) -> Result<A, Self::Error>
    where
        A: ApiCategoryResponse,
    {
        let url = request.url(&self.key);

        let value = client.request(url).await.map_err(ApiClientError::Client)?;

        Ok(A::from_response(ApiResponse::from_value(value)?))
    }
}

#[derive(Debug)]
pub struct ApiRequest<A>
where
    A: ApiCategoryResponse,
{
    selections: Vec<&'static str>,
    id: Option<u64>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    comment: Option<String>,
    phantom: std::marker::PhantomData<A>,
}

impl<A> std::default::Default for ApiRequest<A>
where
    A: ApiCategoryResponse,
{
    fn default() -> Self {
        Self {
            selections: Vec::default(),
            id: None,
            from: None,
            to: None,
            comment: None,
            phantom: std::marker::PhantomData::default(),
        }
    }
}

impl<A> ApiRequest<A>
where
    A: ApiCategoryResponse,
{
    pub fn url(&self, key: &str) -> String {
        let mut query_fragments = vec![
            format!("selections={}", self.selections.join(",")),
            format!("key={}", key),
        ];

        if let Some(from) = self.from {
            query_fragments.push(format!("from={}", from.timestamp()));
        }

        if let Some(to) = self.to {
            query_fragments.push(format!("to={}", to.timestamp()));
        }

        if let Some(comment) = &self.comment {
            query_fragments.push(format!("comment={}", comment));
        }

        let query = query_fragments.join("&");

        let id_fragment = match self.id {
            Some(id) => id.to_string(),
            None => "".to_owned(),
        };

        format!(
            "https://api.torn.com/{}/{}?{}",
            A::Selection::category(),
            id_fragment,
            query
        )
    }
}

pub struct ApiRequestBuilder<A>
where
    A: ApiCategoryResponse,
{
    request: ApiRequest<A>,
}

impl<A> ApiRequestBuilder<A>
where
    A: ApiCategoryResponse,
{
    pub(crate) fn new() -> Self {
        Self {
            request: ApiRequest::default(),
        }
    }

    #[must_use]
    pub fn id(mut self, id: u64) -> Self {
        self.request.id = Some(id);
        self
    }

    #[must_use]
    pub fn selections(mut self, selections: &[A::Selection]) -> Self {
        self.request
            .selections
            .append(&mut selections.iter().map(ApiSelection::raw_value).collect());
        self
    }

    #[must_use]
    pub fn from(mut self, from: DateTime<Utc>) -> Self {
        self.request.from = Some(from);
        self
    }

    #[must_use]
    pub fn to(mut self, to: DateTime<Utc>) -> Self {
        self.request.to = Some(to);
        self
    }

    #[must_use]
    pub fn comment(mut self, comment: String) -> Self {
        self.request.comment = Some(comment);
        self
    }
}

pub mod prelude {}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Once;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use ::awc::Client;
    #[cfg(feature = "reqwest")]
    pub use ::reqwest::Client;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use crate::ApiClient as ClientTrait;
    #[cfg(feature = "reqwest")]
    pub use crate::ThreadSafeApiClient as ClientTrait;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use actix_rt::test as async_test;
    #[cfg(feature = "reqwest")]
    pub use tokio::test as async_test;

    use super::*;

    static INIT: Once = Once::new();

    pub(crate) fn setup() -> String {
        INIT.call_once(|| {
            dotenv::dotenv().ok();
        });
        std::env::var("APIKEY").expect("api key")
    }

    #[test]
    fn selection_raw_value() {
        assert_eq!(user::Selection::Basic.raw_value(), "basic");
    }

    #[cfg(feature = "reqwest")]
    #[tokio::test]
    async fn reqwest() {
        let key = setup();

        Client::default().torn_api(key).user(|b| b).await.unwrap();
    }

    #[cfg(feature = "awc")]
    #[actix_rt::test]
    async fn awc() {
        let key = setup();

        Client::default().torn_api(key).user(|b| b).await.unwrap();
    }
}
