#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

pub mod faction;
pub mod local;
pub mod send;
pub mod torn;
pub mod user;

#[cfg(feature = "awc")]
mod awc;

#[cfg(feature = "reqwest")]
mod reqwest;

mod de_util;

use std::fmt::Write;

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
        D::deserialize(&self.value)
    }

    fn decode_field<D>(&self, field: &'static str) -> serde_json::Result<D>
    where
        D: DeserializeOwned,
    {
        self.value
            .get(field)
            .ok_or_else(|| serde_json::Error::missing_field(field))
            .and_then(D::deserialize)
    }

    fn decode_field_with<'de, V, F>(&'de self, field: &'static str, fun: F) -> serde_json::Result<V>
    where
        F: FnOnce(&'de serde_json::Value) -> serde_json::Result<V>,
    {
        self.value
            .get(field)
            .ok_or_else(|| serde_json::Error::missing_field(field))
            .and_then(fun)
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

pub struct DirectExecutor<C> {
    key: String,
    _marker: std::marker::PhantomData<C>,
}

impl<C> DirectExecutor<C> {
    fn new(key: String) -> Self {
        Self {
            key,
            _marker: Default::default(),
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

#[derive(Debug)]
pub struct ApiRequest<A>
where
    A: ApiCategoryResponse,
{
    selections: Vec<&'static str>,
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
    pub fn url(&self, key: &str, id: Option<i64>) -> String {
        let mut url = format!("https://api.torn.com/{}/", A::Selection::category());

        if let Some(id) = id {
            write!(url, "{}", id).unwrap();
        }

        write!(url, "?selections={}&key={}", self.selections.join(","), key).unwrap();

        if let Some(from) = self.from {
            write!(url, "&from={}", from.timestamp()).unwrap();
        }

        if let Some(to) = self.to {
            write!(url, "&to={}", to.timestamp()).unwrap();
        }

        if let Some(comment) = &self.comment {
            write!(url, "&comment={}", comment).unwrap();
        }

        url
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

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Once;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use ::awc::Client;
    #[cfg(feature = "reqwest")]
    pub use ::reqwest::Client;

    #[cfg(all(not(feature = "reqwest"), feature = "awc"))]
    pub use crate::local::ApiClient as ClientTrait;
    #[cfg(feature = "reqwest")]
    pub use crate::send::ApiClient as ClientTrait;

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

        Client::default()
            .torn_api(key)
            .user(None, |b| b)
            .await
            .unwrap();
    }

    #[cfg(feature = "awc")]
    #[actix_rt::test]
    async fn awc() {
        let key = setup();

        Client::default()
            .torn_api(key)
            .user(None, |b| b)
            .await
            .unwrap();
    }
}
