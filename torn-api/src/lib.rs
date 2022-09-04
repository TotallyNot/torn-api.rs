#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

pub mod faction;
pub mod user;

mod de_util;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::de::{DeserializeOwned, Error as DeError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("api returned error '{reason}', code = '{code}'")]
    Api { code: u8, reason: String },

    #[cfg(feature = "reqwest")]
    #[error("api request failed with network error")]
    Reqwest(#[from] reqwest::Error),

    #[cfg(feature = "awc")]
    #[error("api request failed with network error")]
    AwcSend(#[from] awc::error::SendRequestError),

    #[cfg(feature = "awc")]
    #[error("api request failed to read payload")]
    AwcPayload(#[from] awc::error::JsonPayloadError),

    #[error("api response couldn't be deserialized")]
    Deserialize(#[from] serde_json::Error),
}

pub struct ApiResponse {
    value: serde_json::Value,
}

impl ApiResponse {
    fn from_value(mut value: serde_json::Value) -> Result<Self, ClientError> {
        #[derive(serde::Deserialize)]
        struct ApiErrorDto {
            code: u8,
            #[serde(rename = "error")]
            reason: String,
        }
        match value.get_mut("error") {
            Some(error) => {
                let dto: ApiErrorDto = serde_json::from_value(error.take())?;
                Err(ClientError::Api {
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

#[cfg(feature = "awc")]
#[async_trait(?Send)]
pub trait ApiClient {
    async fn request(&self, url: String) -> Result<ApiResponse, ClientError>;
}

#[cfg(not(feature = "awc"))]
#[async_trait]
pub trait ApiClient: Send + Sync {
    async fn request(&self, url: String) -> Result<ApiResponse, ClientError>;
}

pub trait DirectApiClient: ApiClient {
    fn torn_api(&self, key: String) -> DirectExecutor<Self>
    where
        Self: Sized,
    {
        DirectExecutor::from_client(self, key)
    }
}

pub trait BackedApiClient: ApiClient {}

#[cfg(feature = "reqwest")]
#[cfg_attr(feature = "awc", async_trait(?Send))]
#[cfg_attr(not(feature = "awc"), async_trait)]
impl crate::ApiClient for reqwest::Client {
    async fn request(&self, url: String) -> Result<ApiResponse, crate::ClientError> {
        let value: serde_json::Value = self.get(url).send().await?.json().await?;
        Ok(ApiResponse::from_value(value)?)
    }
}

#[cfg(feature = "reqwest")]
impl crate::DirectApiClient for reqwest::Client {}

#[cfg(feature = "awc")]
#[async_trait(?Send)]
impl crate::ApiClient for awc::Client {
    async fn request(&self, url: String) -> Result<ApiResponse, crate::ClientError> {
        let value: serde_json::Value = self.get(url).send().await?.json().await?;
        Ok(ApiResponse::from_value(value)?)
    }
}

#[cfg(feature = "awc")]
impl crate::DirectApiClient for awc::Client {}

#[cfg_attr(feature = "awc", async_trait(?Send))]
#[cfg_attr(not(feature = "awc"), async_trait)]
pub trait ApiRequestExecutor<'client> {
    type Err: std::error::Error;

    async fn excute<A>(&self, request: ApiRequest<A>) -> Result<A, Self::Err>
    where
        A: ApiCategoryResponse;

    #[must_use]
    fn user<'executor>(
        &'executor self,
    ) -> ApiRequestBuilder<'client, 'executor, Self, user::Response> {
        ApiRequestBuilder::new(self)
    }

    #[must_use]
    fn faction<'executor>(
        &'executor self,
    ) -> ApiRequestBuilder<'client, 'executor, Self, faction::Response> {
        ApiRequestBuilder::new(self)
    }
}

pub struct DirectExecutor<'client, C>
where
    C: ApiClient,
{
    client: &'client C,
    key: String,
}

impl<'client, C> DirectExecutor<'client, C>
where
    C: ApiClient,
{
    #[allow(dead_code)]
    pub(crate) fn from_client(client: &'client C, key: String) -> Self {
        Self { client, key }
    }
}

#[cfg_attr(feature = "awc", async_trait(?Send))]
#[cfg_attr(not(feature = "awc"), async_trait)]
impl<'client, C> ApiRequestExecutor<'client> for DirectExecutor<'client, C>
where
    C: ApiClient,
{
    type Err = ClientError;

    async fn excute<A>(&self, request: ApiRequest<A>) -> Result<A, Self::Err>
    where
        A: ApiCategoryResponse,
    {
        let url = request.url(&self.key);

        self.client.request(url).await.map(A::from_response)
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

pub struct ApiRequestBuilder<'client, 'executor, E, A>
where
    E: ApiRequestExecutor<'client> + ?Sized,
    A: ApiCategoryResponse,
{
    executor: &'executor E,
    request: ApiRequest<A>,
    _phantom: std::marker::PhantomData<&'client E>,
}

impl<'client, 'executor, E, A> ApiRequestBuilder<'client, 'executor, E, A>
where
    E: ApiRequestExecutor<'client> + ?Sized,
    A: ApiCategoryResponse,
{
    pub(crate) fn new(executor: &'executor E) -> Self {
        Self {
            executor,
            request: ApiRequest::default(),
            _phantom: std::marker::PhantomData::default(),
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

    /// Executes the api request.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use torn_api::{prelude::*, ClientError};
    /// use reqwest::Client;
    /// # async {
    ///
    /// let key = "XXXXXXXXX".to_owned();
    /// let response = Client::new()
    ///     .torn_api(key)
    ///     .user()
    ///     .send()
    ///     .await;
    ///
    /// // invalid key
    /// assert!(matches!(response, Err(ClientError::Api { code: 2, .. })));
    /// # };
    /// ```
    ///
    /// # Errors
    ///
    /// Will return an `Err` if the API returns an API error, the request fails due to a network
    /// error, or if the response body doesn't contain valid json.
    pub async fn send(self) -> Result<A, <E as ApiRequestExecutor<'client>>::Err> {
        self.executor.excute(self.request).await
    }
}

pub mod prelude {
    pub use super::{ApiClient, ApiRequestExecutor, DirectApiClient};
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Once;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use awc::Client;
    #[cfg(feature = "reqwest")]
    pub use reqwest::Client;

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

        reqwest::Client::default()
            .torn_api(key)
            .user()
            .send()
            .await
            .unwrap();
    }

    #[cfg(feature = "awc")]
    #[actix_rt::test]
    async fn awc() {
        let key = setup();

        awc::Client::default()
            .torn_api(key)
            .user()
            .send()
            .await
            .unwrap();
    }
}
