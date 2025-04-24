use bon::Builder;
use bytes::Bytes;
use http::StatusCode;

use crate::{
    executor::Executor,
    models::{FactionChainsResponse, FactionId},
};

pub mod models;

#[derive(Default)]
pub struct ApiRequest<D = ()> {
    pub disriminant: D,
    pub path: String,
    pub parameters: Vec<(&'static str, String)>,
}

impl<D> ApiRequest<D> {
    pub fn url(&self) -> String {
        let mut url = format!("https://api.torn.com/v2{}?", self.path);

        for (name, value) in &self.parameters {
            url.push_str(&format!("{name}={value}"));
        }

        url
    }
}

pub struct ApiResponse<D = ()> {
    pub discriminant: D,
    pub body: Option<Bytes>,
    pub status: StatusCode,
}

pub trait IntoRequest: Send {
    type Discriminant: Send;
    type Response: for<'de> serde::Deserialize<'de> + Send;
    fn into_request(self) -> ApiRequest<Self::Discriminant>;
}

pub struct FactionScope<'e, E>(&'e E)
where
    E: Executor;

impl<E> FactionScope<'_, E>
where
    E: Executor,
{
    pub async fn chains_for_id<S>(
        &self,
        id: FactionId,
        builder: impl FnOnce(
            FactionChainsRequestBuilder<faction_chains_request_builder::Empty>,
        ) -> FactionChainsRequestBuilder<S>,
    ) -> Result<FactionChainsResponse, E::Error>
    where
        S: faction_chains_request_builder::IsComplete,
    {
        let r = builder(FactionChainsRequest::with_id(id)).build();

        self.0.fetch(r).await
    }
}

#[derive(Builder)]
#[builder(start_fn = with_id)]
pub struct FactionChainsRequest {
    #[builder(start_fn)]
    pub id: FactionId,
    pub limit: Option<usize>,
}

impl IntoRequest for FactionChainsRequest {
    type Discriminant = FactionId;
    type Response = FactionChainsResponse;
    fn into_request(self) -> ApiRequest<Self::Discriminant> {
        ApiRequest {
            disriminant: self.id,
            path: format!("/faction/{}/chains", self.id),
            parameters: self
                .limit
                .into_iter()
                .map(|l| ("limit", l.to_string()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::executor::ReqwestClient;

    use super::*;

    #[tokio::test]
    async fn test_request() {
        let client = ReqwestClient::new("nAYRXaoqzBAGalWt");

        let r = models::TornItemsForIdsRequest::builder("1".to_owned()).build();
        client.fetch(r).await.unwrap();
    }
}
