use std::future::Future;

use futures::{Stream, StreamExt};
use http::{header::AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;

use crate::request::{ApiRequest, ApiResponse, IntoRequest};
#[cfg(feature = "scopes")]
use crate::scopes::{
    BulkFactionScope, BulkForumScope, BulkMarketScope, BulkRacingScope, BulkTornScope,
    BulkUserScope, FactionScope, ForumScope, MarketScope, RacingScope, TornScope, UserScope,
};

pub trait Executor: Sized {
    type Error: From<serde_json::Error> + From<crate::ApiError> + Send;

    fn execute<R>(
        self,
        request: R,
    ) -> impl Future<Output = (R::Discriminant, Result<ApiResponse, Self::Error>)> + Send
    where
        R: IntoRequest;

    fn fetch<R>(self, request: R) -> impl Future<Output = Result<R::Response, Self::Error>> + Send
    where
        R: IntoRequest,
    {
        // HACK: workaround for not using `async` in trait declaration.
        // The future is `Send` but `&self` might not be.
        let fut = self.execute(request);
        async {
            let resp = fut.await.1?;

            let bytes = resp.body.unwrap();

            if bytes.starts_with(br#"{"error":{"#) {
                #[derive(Deserialize)]
                struct ErrorBody<'a> {
                    code: u16,
                    error: &'a str,
                }
                #[derive(Deserialize)]
                struct ErrorContainer<'a> {
                    #[serde(borrow)]
                    error: ErrorBody<'a>,
                }

                let error: ErrorContainer = serde_json::from_slice(&bytes)?;
                return Err(crate::ApiError::new(error.error.code, error.error.error).into());
            }

            let resp = serde_json::from_slice(&bytes)?;

            Ok(resp)
        }
    }
}

pub trait BulkExecutor<'e>: 'e + Sized {
    type Error: From<serde_json::Error> + From<crate::ApiError> + Send;

    fn execute<R>(
        self,
        requests: impl IntoIterator<Item = R>,
    ) -> impl Stream<Item = (R::Discriminant, Result<ApiResponse, Self::Error>)>
    where
        R: IntoRequest;

    fn fetch_many<R>(
        self,
        requests: impl IntoIterator<Item = R>,
    ) -> impl Stream<Item = (R::Discriminant, Result<R::Response, Self::Error>)>
    where
        R: IntoRequest,
    {
        self.execute(requests).map(|(d, r)| {
            let r = match r {
                Ok(r) => r,
                Err(why) => return (d, Err(why)),
            };
            let bytes = r.body.unwrap();

            if bytes.starts_with(br#"{"error":{"#) {
                #[derive(Deserialize)]
                struct ErrorBody<'a> {
                    code: u16,
                    error: &'a str,
                }
                #[derive(Deserialize)]
                struct ErrorContainer<'a> {
                    #[serde(borrow)]
                    error: ErrorBody<'a>,
                }

                let error: ErrorContainer = match serde_json::from_slice(&bytes) {
                    Ok(error) => error,
                    Err(why) => return (d, Err(why.into())),
                };
                return (
                    d,
                    Err(crate::ApiError::new(error.error.code, error.error.error).into()),
                );
            }

            let resp = match serde_json::from_slice(&bytes) {
                Ok(resp) => resp,
                Err(why) => return (d, Err(why.into())),
            };

            (d, Ok(resp))
        })
    }
}

#[cfg(feature = "scopes")]
pub trait ExecutorExt: Executor + Sized {
    fn user(self) -> UserScope<Self>;

    fn faction(self) -> FactionScope<Self>;

    fn torn(self) -> TornScope<Self>;

    fn market(self) -> MarketScope<Self>;

    fn racing(self) -> RacingScope<Self>;

    fn forum(self) -> ForumScope<Self>;
}

#[cfg(feature = "scopes")]
impl<T> ExecutorExt for T
where
    T: Executor + Sized,
{
    fn user(self) -> UserScope<Self> {
        UserScope::new(self)
    }

    fn faction(self) -> FactionScope<Self> {
        FactionScope::new(self)
    }

    fn torn(self) -> TornScope<Self> {
        TornScope::new(self)
    }

    fn market(self) -> MarketScope<Self> {
        MarketScope::new(self)
    }

    fn racing(self) -> RacingScope<Self> {
        RacingScope::new(self)
    }

    fn forum(self) -> ForumScope<Self> {
        ForumScope::new(self)
    }
}

#[cfg(feature = "scopes")]
pub trait BulkExecutorExt<'e>: BulkExecutor<'e> + Sized {
    fn user_bulk(self) -> BulkUserScope<'e, Self>;

    fn faction_bulk(self) -> BulkFactionScope<'e, Self>;

    fn torn_bulk(self) -> BulkTornScope<'e, Self>;

    fn market_bulk(self) -> BulkMarketScope<'e, Self>;

    fn racing_bulk(self) -> BulkRacingScope<'e, Self>;

    fn forum_bulk(self) -> BulkForumScope<'e, Self>;
}

#[cfg(feature = "scopes")]
impl<'e, T> BulkExecutorExt<'e> for T
where
    T: BulkExecutor<'e> + Sized,
{
    fn user_bulk(self) -> BulkUserScope<'e, Self> {
        BulkUserScope::new(self)
    }

    fn faction_bulk(self) -> BulkFactionScope<'e, Self> {
        BulkFactionScope::new(self)
    }

    fn torn_bulk(self) -> BulkTornScope<'e, Self> {
        BulkTornScope::new(self)
    }

    fn market_bulk(self) -> BulkMarketScope<'e, Self> {
        BulkMarketScope::new(self)
    }

    fn racing_bulk(self) -> BulkRacingScope<'e, Self> {
        BulkRacingScope::new(self)
    }

    fn forum_bulk(self) -> BulkForumScope<'e, Self> {
        BulkForumScope::new(self)
    }
}

pub struct ReqwestClient(reqwest::Client);

impl ReqwestClient {
    pub fn new(api_key: &str) -> Self {
        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("ApiKey {api_key}")).unwrap(),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .brotli(true)
            .build()
            .unwrap();

        Self(client)
    }
}

impl ReqwestClient {
    async fn execute_api_request(&self, request: ApiRequest) -> Result<ApiResponse, crate::Error> {
        let url = request.url();

        let response = self.0.get(url).send().await?;
        let status = response.status();
        let body = response.bytes().await.ok();

        Ok(ApiResponse { status, body })
    }
}

impl Executor for &ReqwestClient {
    type Error = crate::Error;

    async fn execute<R>(self, request: R) -> (R::Discriminant, Result<ApiResponse, Self::Error>)
    where
        R: IntoRequest,
    {
        let (d, request) = request.into_request();
        (d, self.execute_api_request(request).await)
    }
}

impl<'e> BulkExecutor<'e> for &'e ReqwestClient {
    type Error = crate::Error;

    fn execute<R>(
        self,
        requests: impl IntoIterator<Item = R>,
    ) -> impl Stream<Item = (R::Discriminant, Result<ApiResponse, Self::Error>)>
    where
        R: IntoRequest,
    {
        futures::stream::iter(requests)
            .map(move |r| <Self as Executor>::execute(self, r))
            .buffer_unordered(25)
    }
}

#[cfg(test)]
mod test {
    use crate::{scopes::test::test_client, ApiError, Error};

    use super::*;

    #[cfg(feature = "scopes")]
    #[tokio::test]
    async fn api_error() {
        let client = test_client().await;

        let resp = client.faction().basic_for_id((-1).into(), |b| b).await;

        match resp {
            Err(Error::Api(ApiError::IncorrectIdEntityRelation)) => (),
            other => panic!("Expected incorrect id entity relation error, got {other:?}"),
        }
    }

    #[cfg(feature = "scopes")]
    #[tokio::test]
    async fn bulk_request() {
        let client = test_client().await;

        let stream = client
            .faction_bulk()
            .basic_for_id(vec![19.into(), 89.into()], |b| b);

        let mut responses: Vec<_> = stream.collect().await;

        let (_id1, basic1) = responses.pop().unwrap();
        basic1.unwrap();

        let (_id2, basic2) = responses.pop().unwrap();
        basic2.unwrap();
    }
}
