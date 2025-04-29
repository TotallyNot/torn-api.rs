use bytes::Bytes;
use http::StatusCode;

#[cfg(feature = "requests")]
pub mod models;

#[derive(Default)]
pub struct ApiRequest {
    pub path: String,
    pub parameters: Vec<(&'static str, String)>,
}

impl ApiRequest {
    pub fn url(&self) -> String {
        let mut url = format!("https://api.torn.com/v2{}?", self.path);

        for (name, value) in &self.parameters {
            url.push_str(&format!("{name}={value}"));
        }

        url
    }
}

pub struct ApiResponse {
    pub body: Option<Bytes>,
    pub status: StatusCode,
}

pub trait IntoRequest: Send {
    type Discriminant: Send;
    type Response: for<'de> serde::Deserialize<'de> + Send;
    fn into_request(self) -> (Self::Discriminant, ApiRequest);
}

pub(crate) struct WrappedApiRequest<R>
where
    R: IntoRequest,
{
    discriminant: R::Discriminant,
    request: ApiRequest,
}

impl<R> IntoRequest for WrappedApiRequest<R>
where
    R: IntoRequest,
{
    type Discriminant = R::Discriminant;
    type Response = R::Response;
    fn into_request(self) -> (Self::Discriminant, ApiRequest) {
        (self.discriminant, self.request)
    }
}

#[cfg(test)]
mod test {}
