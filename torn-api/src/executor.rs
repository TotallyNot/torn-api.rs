use http::{HeaderMap, HeaderValue, header::AUTHORIZATION};
use serde::Deserialize;

use crate::{
    request::{ApiResponse, IntoRequest},
    scopes::{FactionScope, ForumScope, MarketScope, RacingScope, TornScope, UserScope},
};

pub trait Executor {
    type Error: From<serde_json::Error> + From<crate::ApiError> + Send;

    fn execute<R>(
        &self,
        request: R,
    ) -> impl Future<Output = Result<ApiResponse<R::Discriminant>, Self::Error>> + Send
    where
        R: IntoRequest;

    fn fetch<R>(&self, request: R) -> impl Future<Output = Result<R::Response, Self::Error>> + Send
    where
        R: IntoRequest,
    {
        // HACK: workaround for not using `async` in trait declaration.
        // The future is `Send` but `&self` might not be.
        let fut = self.execute(request);
        async {
            let resp = fut.await?;

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

pub trait ExecutorExt: Executor + Sized {
    fn user(&self) -> UserScope<'_, Self>;

    fn faction(&self) -> FactionScope<'_, Self>;

    fn torn(&self) -> TornScope<'_, Self>;

    fn market(&self) -> MarketScope<'_, Self>;

    fn racing(&self) -> RacingScope<'_, Self>;

    fn forum(&self) -> ForumScope<'_, Self>;
}

impl<T> ExecutorExt for T
where
    T: Executor + Sized,
{
    fn user(&self) -> UserScope<'_, Self> {
        UserScope::new(self)
    }

    fn faction(&self) -> FactionScope<'_, Self> {
        FactionScope::new(self)
    }

    fn torn(&self) -> TornScope<'_, Self> {
        TornScope::new(self)
    }

    fn market(&self) -> MarketScope<'_, Self> {
        MarketScope::new(self)
    }

    fn racing(&self) -> RacingScope<'_, Self> {
        RacingScope::new(self)
    }

    fn forum(&self) -> ForumScope<'_, Self> {
        ForumScope::new(self)
    }
}

impl Executor for ReqwestClient {
    type Error = crate::Error;

    async fn execute<R>(&self, request: R) -> Result<ApiResponse<R::Discriminant>, Self::Error>
    where
        R: IntoRequest,
    {
        let request = request.into_request();
        let url = request.url();

        let response = self.0.get(url).send().await?;
        let status = response.status();
        let body = response.bytes().await.ok();

        Ok(ApiResponse {
            discriminant: request.disriminant,
            status,
            body,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{ApiError, Error, scopes::test::test_client};

    use super::*;

    #[tokio::test]
    async fn api_error() {
        let client = test_client().await;

        let resp = client.faction().basic_for_id((-1).into(), |b| b).await;

        match resp {
            Err(Error::Api(ApiError::IncorrectIdEntityRelation)) => (),
            other => panic!("Expected incorrect id entity relation error, got {other:?}"),
        }
    }
}
