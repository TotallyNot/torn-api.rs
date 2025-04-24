use std::borrow::Cow;

use serde::{Deserialize, Deserializer};

use super::parameter::OpenApiParameter;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OpenApiPathParameter<'a> {
    Link {
        #[serde(rename = "$ref")]
        ref_path: &'a str,
    },
    Inline(OpenApiParameter<'a>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaLink<'a> {
    #[serde(rename = "$ref")]
    pub ref_path: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OpenApiResponseBody<'a> {
    Schema(SchemaLink<'a>),
    Union {
        #[serde(borrow, rename = "anyOf")]
        any_of: Vec<SchemaLink<'a>>,
    },
}

fn deserialize_response_body<'de, D>(deserializer: D) -> Result<OpenApiResponseBody<'de>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Json<'a> {
        #[serde(borrow)]
        schema: OpenApiResponseBody<'a>,
    }
    #[derive(Deserialize)]
    struct Content<'a> {
        #[serde(borrow, rename = "application/json")]
        json: Json<'a>,
    }
    #[derive(Deserialize)]
    struct StatusOk<'a> {
        #[serde(borrow)]
        content: Content<'a>,
    }
    #[derive(Deserialize)]
    struct Responses<'a> {
        #[serde(borrow, rename = "200")]
        ok: StatusOk<'a>,
    }

    let responses = Responses::deserialize(deserializer)?;

    Ok(responses.ok.content.json.schema)
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiPathBody<'a> {
    pub summary: Option<Cow<'a, str>>,
    pub description: Cow<'a, str>,
    #[serde(borrow, default)]
    pub parameters: Vec<OpenApiPathParameter<'a>>,
    #[serde(
        borrow,
        rename = "responses",
        deserialize_with = "deserialize_response_body"
    )]
    pub response_content: OpenApiResponseBody<'a>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiPath<'a> {
    #[serde(borrow)]
    pub get: OpenApiPathBody<'a>,
}
