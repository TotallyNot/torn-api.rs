use r#enum::Enum;
use indexmap::IndexMap;
use newtype::Newtype;
use object::Object;
use proc_macro2::TokenStream;

use crate::openapi::r#type::OpenApiType;

pub mod r#enum;
pub mod newtype;
pub mod object;
pub mod parameter;
pub mod path;
pub mod scope;
pub mod union;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Model {
    Newtype(Newtype),
    Enum(Enum),
    Object(Object),
    Unresolved,
}

pub fn resolve(r#type: &OpenApiType, name: &str, schemas: &IndexMap<&str, OpenApiType>) -> Model {
    match r#type {
        OpenApiType {
            r#enum: Some(_), ..
        } => Enum::from_schema(name, r#type).map_or(Model::Unresolved, Model::Enum),
        OpenApiType {
            r#type: Some("object"),
            ..
        } => Object::from_schema_object(name, r#type, schemas)
            .map_or(Model::Unresolved, Model::Object),
        OpenApiType {
            r#type: Some(_), ..
        } => Newtype::from_schema(name, r#type).map_or(Model::Unresolved, Model::Newtype),
        OpenApiType {
            one_of: Some(types),
            ..
        } => Enum::from_one_of(name, types).map_or(Model::Unresolved, Model::Enum),
        OpenApiType {
            all_of: Some(types),
            ..
        } => Object::from_all_of(name, types, schemas).map_or(Model::Unresolved, Model::Object),
        _ => Model::Unresolved,
    }
}

impl Model {
    pub fn codegen(&self) -> Option<TokenStream> {
        match self {
            Self::Newtype(newtype) => newtype.codegen(),
            Self::Enum(r#enum) => r#enum.codegen(),
            Self::Object(object) => object.codegen(),
            Self::Unresolved => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        model::r#enum::{EnumRepr, EnumVariant},
        openapi::schema::OpenApiSchema,
    };

    #[test]
    fn resolve_newtypes() {
        let schema = OpenApiSchema::read().unwrap();

        let user_id_schema = schema.components.schemas.get("UserId").unwrap();

        let user_id = resolve(user_id_schema, "UserId", &schema.components.schemas);

        assert_eq!(
            user_id,
            Model::Newtype(Newtype {
                name: "UserId".to_owned(),
                description: None,
                inner: newtype::NewtypeInner::I32,
                copy: true,
                ord: true
            })
        );

        let attack_code_schema = schema.components.schemas.get("AttackCode").unwrap();

        let attack_code = resolve(attack_code_schema, "AttackCode", &schema.components.schemas);

        assert_eq!(
            attack_code,
            Model::Newtype(Newtype {
                name: "AttackCode".to_owned(),
                description: None,
                inner: newtype::NewtypeInner::Str,
                copy: false,
                ord: false
            })
        );
    }

    #[test]
    fn resolve_enums() {
        let schema = OpenApiSchema::read().unwrap();

        let forum_feed_type_schema = schema.components.schemas.get("ForumFeedTypeEnum").unwrap();

        let forum_feed_type = resolve(
            forum_feed_type_schema,
            "ForumFeedTypeEnum",
            &schema.components.schemas,
        );

        assert_eq!(forum_feed_type, Model::Enum(Enum {
            name: "ForumFeedType".to_owned(),
            description: Some("This represents the type of the activity. Values range from 1 to 8 where:\n *                    1 = 'X posted on a thread',\n *                    2 = 'X created a thread',\n *                    3 = 'X liked your thread',\n *                    4 = 'X disliked your thread',\n *                    5 = 'X liked your post',\n *                    6 = 'X disliked your post',\n *                    7 = 'X quoted your post'.".to_owned()),
            repr: Some(EnumRepr::U32),
            copy: true,
            untagged: false,
            display: true,
            variants: vec![
                EnumVariant {
                    name: "Variant1".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(1),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant2".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(2),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant3".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(3),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant4".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(4),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant5".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(5),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant6".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(6),
                    ..Default::default()
                },
                EnumVariant {
                    name: "Variant7".to_owned(),
                    value: r#enum::EnumVariantValue::Repr(7),
                    ..Default::default()
                },
            ]
        }))
    }

    #[test]
    fn resolve_all() {
        let schema = OpenApiSchema::read().unwrap();

        let mut unresolved = vec![];
        let total = schema.components.schemas.len();

        for (name, desc) in &schema.components.schemas {
            if resolve(desc, name, &schema.components.schemas) == Model::Unresolved {
                unresolved.push(name);
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to resolve {}/{} types. Could not resolve [{}]",
                unresolved.len(),
                total,
                unresolved
                    .into_iter()
                    .map(|u| format!("`{u}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}
