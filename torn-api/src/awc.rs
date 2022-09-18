use async_trait::async_trait;
use thiserror::Error;

use crate::local::ApiClient;

#[derive(Error, Debug)]
pub enum AwcApiClientError {
    #[error(transparent)]
    Client(#[from] awc::error::SendRequestError),

    #[error(transparent)]
    Payload(#[from] awc::error::JsonPayloadError),
}

#[async_trait(?Send)]
impl ApiClient for awc::Client {
    type Error = AwcApiClientError;

    async fn request(&self, url: String) -> Result<serde_json::Value, Self::Error> {
        self.get(url).send().await?.json().await.map_err(Into::into)
    }
}
