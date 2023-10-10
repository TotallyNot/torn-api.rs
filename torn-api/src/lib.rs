#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

pub mod into_owned;
pub mod local;
pub mod send;

#[cfg(feature = "user")]
pub mod user;

#[cfg(feature = "faction")]
pub mod faction;

#[cfg(feature = "torn")]
pub mod torn;

#[cfg(feature = "key")]
pub mod key;

#[cfg(feature = "awc")]
pub mod awc;

#[cfg(feature = "reqwest")]
pub mod reqwest;

#[cfg(feature = "__common")]
pub mod common;

mod de_util;

use std::fmt::Write;

use chrono::{DateTime, Utc};
use serde::{de::Error as DeError, Deserialize};
use thiserror::Error;

pub use into_owned::IntoOwned;

pub struct ApiResponse {
    pub value: serde_json::Value,
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

    #[allow(dead_code)]
    fn decode<'de, D>(&'de self) -> serde_json::Result<D>
    where
        D: Deserialize<'de>,
    {
        D::deserialize(&self.value)
    }

    #[allow(dead_code)]
    fn decode_field<'de, D>(&'de self, field: &'static str) -> serde_json::Result<D>
    where
        D: Deserialize<'de>,
    {
        self.value
            .get(field)
            .ok_or_else(|| serde_json::Error::missing_field(field))
            .and_then(D::deserialize)
    }

    #[allow(dead_code)]
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

pub trait ApiSelection: Send + Sync {
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
    A: ApiSelection,
{
    pub selections: Vec<&'static str>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub comment: Option<String>,
    phantom: std::marker::PhantomData<A>,
}

impl<A> std::default::Default for ApiRequest<A>
where
    A: ApiSelection,
{
    fn default() -> Self {
        Self {
            selections: Vec::default(),
            from: None,
            to: None,
            comment: None,
            phantom: Default::default(),
        }
    }
}

impl<A> ApiRequest<A>
where
    A: ApiSelection,
{
    pub fn url(&self, key: &str, id: Option<&str>) -> String {
        let mut url = format!("https://api.torn.com/{}/", A::category());

        if let Some(id) = id {
            write!(url, "{}", id).unwrap();
        }

        write!(url, "?selections={}&key={}", self.selections.join(","), key).unwrap();

        if let Some(from) = self.from {
            write!(url, "&from={}", from).unwrap();
        }

        if let Some(to) = self.to {
            write!(url, "&to={}", to).unwrap();
        }

        if let Some(comment) = &self.comment {
            write!(url, "&comment={}", comment).unwrap();
        }

        url
    }
}

pub struct ApiRequestBuilder<A>
where
    A: ApiSelection,
{
    request: ApiRequest<A>,
    id: Option<String>,
}

impl<A> Default for ApiRequestBuilder<A>
where
    A: ApiSelection,
{
    fn default() -> Self {
        Self {
            request: Default::default(),
            id: None,
        }
    }
}

impl<A> ApiRequestBuilder<A>
where
    A: ApiSelection,
{
    #[must_use]
    pub fn selections(mut self, selections: &[A]) -> Self {
        self.request
            .selections
            .append(&mut selections.iter().map(ApiSelection::raw_value).collect());
        self
    }

    #[must_use]
    pub fn from(mut self, from: DateTime<Utc>) -> Self {
        self.request.from = Some(from.timestamp());
        self
    }

    #[must_use]
    pub fn from_timestamp(mut self, from: i64) -> Self {
        self.request.from = Some(from);
        self
    }

    #[must_use]
    pub fn to(mut self, to: DateTime<Utc>) -> Self {
        self.request.to = Some(to.timestamp());
        self
    }

    #[must_use]
    pub fn to_timestamp(mut self, to: i64) -> Self {
        self.request.to = Some(to);
        self
    }

    #[must_use]
    pub fn comment(mut self, comment: String) -> Self {
        self.request.comment = Some(comment);
        self
    }

    #[must_use]
    pub fn id<I>(mut self, id: I) -> Self
    where
        I: ToString,
    {
        self.id = Some(id.to_string());
        self
    }
}

#[cfg(test)]
#[allow(unused)]
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

    #[cfg(feature = "user")]
    #[test]
    fn selection_raw_value() {
        assert_eq!(user::Selection::Basic.raw_value(), "basic");
    }

    #[cfg(all(feature = "reqwest", feature = "user"))]
    #[tokio::test]
    async fn reqwest() {
        let key = setup();

        Client::default().torn_api(key).user(|b| b).await.unwrap();
    }

    #[cfg(all(feature = "awc", feature = "user"))]
    #[actix_rt::test]
    async fn awc() {
        let key = setup();

        Client::default().torn_api(key).user(|b| b).await.unwrap();
    }
}
