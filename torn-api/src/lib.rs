#![warn(clippy::all, clippy::perf, clippy::style, clippy::suspicious)]

pub mod into_owned;
pub mod local;
pub mod send;

#[cfg(feature = "user")]
pub mod user;

#[cfg(feature = "faction")]
pub mod faction;

#[cfg(feature = "market")]
pub mod market;

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
    MalformedResponse(#[from] serde_json::Error),
}

impl ResponseError {
    pub fn api_code(&self) -> Option<u8> {
        match self {
            Self::Api { code, .. } => Some(*code),
            _ => None,
        }
    }
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
    fn raw_value(self) -> &'static str;

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

impl<C> ApiClientError<C>
where
    C: std::error::Error,
{
    pub fn api_code(&self) -> Option<u8> {
        match self {
            Self::Response(err) => err.api_code(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ApiRequest<A>
where
    A: ApiSelection,
{
    pub selections: Vec<&'static str>,
    pub query_items: Vec<(&'static str, String)>,
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
            query_items: Vec::default(),
            comment: None,
            phantom: Default::default(),
        }
    }
}

impl<A> ApiRequest<A>
where
    A: ApiSelection,
{
    fn add_query_item(&mut self, name: &'static str, value: impl ToString) {
        if let Some((_, old)) = self.query_items.iter_mut().find(|(n, _)| *n == name) {
            *old = value.to_string();
        } else {
            self.query_items.push((name, value.to_string()));
        }
    }

    pub fn url(&self, key: &str, id: Option<&str>) -> String {
        let mut url = format!("https://api.torn.com/{}/", A::category());

        if let Some(id) = id {
            write!(url, "{}", id).unwrap();
        }

        write!(url, "?selections={}&key={}", self.selections.join(","), key).unwrap();

        for (name, value) in &self.query_items {
            write!(url, "&{name}={value}").unwrap();
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
    pub fn selections(mut self, selections: impl IntoIterator<Item = A>) -> Self {
        self.request.selections.append(
            &mut selections
                .into_iter()
                .map(ApiSelection::raw_value)
                .collect(),
        );
        self
    }

    #[must_use]
    pub fn from(mut self, from: DateTime<Utc>) -> Self {
        self.request.add_query_item("from", from.timestamp());
        self
    }

    #[must_use]
    pub fn from_timestamp(mut self, from: i64) -> Self {
        self.request.add_query_item("from", from);
        self
    }

    #[must_use]
    pub fn to(mut self, to: DateTime<Utc>) -> Self {
        self.request.add_query_item("to", to.timestamp());
        self
    }

    #[must_use]
    pub fn to_timestamp(mut self, to: i64) -> Self {
        self.request.add_query_item("to", to);
        self
    }

    #[must_use]
    pub fn stats_timestamp(mut self, ts: i64) -> Self {
        self.request.add_query_item("timestamp", ts);
        self
    }

    #[must_use]
    pub fn stats_datetime(mut self, dt: DateTime<Utc>) -> Self {
        self.request.add_query_item("timestamp", dt.timestamp());
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

    #[test]
    fn url_builder_from_dt() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .from(DateTime::default())
            .request
            .url("", None);

        assert_eq!("https://api.torn.com/user/?selections=&key=&from=0", url);
    }

    #[test]
    fn url_builder_from_ts() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .from_timestamp(12345)
            .request
            .url("", None);

        assert_eq!(
            "https://api.torn.com/user/?selections=&key=&from=12345",
            url
        );
    }

    #[test]
    fn url_builder_to_dt() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .to(DateTime::default())
            .request
            .url("", None);

        assert_eq!("https://api.torn.com/user/?selections=&key=&to=0", url);
    }

    #[test]
    fn url_builder_to_ts() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .to_timestamp(12345)
            .request
            .url("", None);

        assert_eq!("https://api.torn.com/user/?selections=&key=&to=12345", url);
    }

    #[test]
    fn url_builder_timestamp_dt() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .stats_datetime(DateTime::default())
            .request
            .url("", None);

        assert_eq!(
            "https://api.torn.com/user/?selections=&key=&timestamp=0",
            url
        );
    }

    #[test]
    fn url_builder_timestamp_ts() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .stats_timestamp(12345)
            .request
            .url("", None);

        assert_eq!(
            "https://api.torn.com/user/?selections=&key=&timestamp=12345",
            url
        );
    }

    #[test]
    fn url_builder_duplicate() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .from(DateTime::default())
            .from_timestamp(12345)
            .request
            .url("", None);

        assert_eq!(
            "https://api.torn.com/user/?selections=&key=&from=12345",
            url
        );
    }

    #[test]
    fn url_builder_many_options() {
        let url = ApiRequestBuilder::<user::Selection>::default()
            .from(DateTime::default())
            .to_timestamp(60)
            .stats_timestamp(12345)
            .selections([user::Selection::PersonalStats])
            .request
            .url("KEY", Some("1"));

        assert_eq!(
            "https://api.torn.com/user/1?selections=personalstats&key=KEY&from=0&to=60&timestamp=12345",
            url
        );
    }
}
