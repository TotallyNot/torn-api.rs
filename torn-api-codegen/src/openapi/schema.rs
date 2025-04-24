use indexmap::IndexMap;
use serde::Deserialize;

use super::{parameter::OpenApiParameter, path::OpenApiPath, r#type::OpenApiType};

#[derive(Debug, Clone, Deserialize)]
pub struct Components<'a> {
    #[serde(borrow)]
    pub schemas: IndexMap<&'a str, OpenApiType<'a>>,
    #[serde(borrow)]
    pub parameters: IndexMap<&'a str, OpenApiParameter<'a>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiSchema<'a> {
    #[serde(borrow)]
    pub paths: IndexMap<&'a str, OpenApiPath<'a>>,
    #[serde(borrow)]
    pub components: Components<'a>,
}

impl OpenApiSchema<'_> {
    pub fn read() -> Result<Self, serde_json::Error> {
        let s = include_str!("../../openapi.json");

        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn read() {
        OpenApiSchema::read().unwrap();
    }
}
