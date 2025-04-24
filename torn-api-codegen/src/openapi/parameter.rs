use std::borrow::Cow;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    Query,
    Path,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OpenApiParameterDefault<'a> {
    Int(i32),
    Str(&'a str),
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiParameterSchema<'a> {
    #[serde(rename = "$ref")]
    pub ref_path: Option<&'a str>,
    pub r#type: Option<&'a str>,
    pub r#enum: Option<Vec<&'a str>>,
    pub format: Option<&'a str>,
    pub default: Option<OpenApiParameterDefault<'a>>,
    pub maximum: Option<i32>,
    pub minimum: Option<i32>,
    pub items: Option<Box<OpenApiParameterSchema<'a>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiParameter<'a> {
    pub name: &'a str,
    pub description: Option<Cow<'a, str>>,
    pub r#in: ParameterLocation,
    pub required: bool,
    #[serde(borrow)]
    pub schema: OpenApiParameterSchema<'a>,
}
