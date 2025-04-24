use std::borrow::Cow;

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum OpenApiVariants<'a> {
    Int(Vec<i32>),
    #[serde(borrow)]
    Str(Vec<&'a str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiType<'a> {
    #[serde(default)]
    pub deprecated: bool,
    pub description: Option<Cow<'a, str>>,

    pub r#type: Option<&'a str>,
    pub format: Option<&'a str>,

    #[serde(rename = "$ref")]
    pub ref_path: Option<&'a str>,

    pub one_of: Option<Vec<OpenApiType<'a>>>,
    pub all_of: Option<Vec<OpenApiType<'a>>>,

    pub required: Option<Vec<&'a str>>,
    #[serde(borrow)]
    pub properties: Option<IndexMap<&'a str, OpenApiType<'a>>>,

    pub items: Option<Box<OpenApiType<'a>>>,
    pub r#enum: Option<OpenApiVariants<'a>>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn object() {
        let json = r##"
            {
                "required": [
                    "name",
                    "branches"
                ],
                "properties": {
                    "name": {
                        "type": "string"
                    },
                    "branches": {
                        "type": "array",
                        "items": {
                            "$ref": "#/components/schemas/TornFactionTreeBranch"
                        }
                    }
                },
                "type": "object"
            }
        "##;

        let obj: OpenApiType = serde_json::from_str(json).unwrap();

        assert_eq!(obj.r#type, Some("object"));

        let props = obj.properties.unwrap();

        assert!(props.contains_key("name"));

        let branches = props.get("branches").unwrap();
        assert_eq!(branches.r#type, Some("array"));

        let items = branches.items.as_ref().unwrap();
        assert!(items.ref_path.is_some());
    }

    #[test]
    fn enum_variants() {
        let int_json = r#"
            [1, 2, 3, 4]
        "#;

        let de: OpenApiVariants = serde_json::from_str(int_json).unwrap();

        assert_eq!(de, OpenApiVariants::Int(vec![1, 2, 3, 4]));

        let str_json = r#"
            ["foo", "bar", "baz"]
        "#;

        let de: OpenApiVariants = serde_json::from_str(str_json).unwrap();

        assert_eq!(de, OpenApiVariants::Str(vec!["foo", "bar", "baz"]));
    }
}
