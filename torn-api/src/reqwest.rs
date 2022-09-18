use async_trait::async_trait;

use crate::send::ApiClient;

#[async_trait]
impl ApiClient for reqwest::Client {
    type Error = reqwest::Error;

    async fn request(&self, url: String) -> Result<serde_json::Value, Self::Error> {
        self.get(url).send().await?.json().await
    }
}
