use bytes::Bytes;
use http::StatusCode;

#[cfg(feature = "requests")]
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

#[cfg(test)]
mod test {}
