use heck::{ToSnakeCase, ToUpperCamelCase};
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::Ident;

use crate::openapi::r#type::OpenApiType;

use super::r#enum::Enum;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    I32,
    I64,
    String,
    Float,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyType {
    Primitive(PrimitiveType),
    Ref(String),
    Enum(Enum),
    Nested(Box<Object>),
    Array(Box<PropertyType>),
}

impl PropertyType {
    pub fn codegen(&self, namespace: &mut ObjectNamespace) -> Option<TokenStream> {
        match self {
            Self::Primitive(PrimitiveType::Bool) => Some(format_ident!("bool").into_token_stream()),
            Self::Primitive(PrimitiveType::I32) => Some(format_ident!("i32").into_token_stream()),
            Self::Primitive(PrimitiveType::I64) => Some(format_ident!("i64").into_token_stream()),
            Self::Primitive(PrimitiveType::String) => {
                Some(format_ident!("String").into_token_stream())
            }
            Self::Primitive(PrimitiveType::Float) => Some(format_ident!("f64").into_token_stream()),
            Self::Ref(path) => {
                let name = path.strip_prefix("#/components/schemas/")?;
                let name = format_ident!("{name}");

                Some(quote! { crate::models::#name })
            }
            Self::Enum(r#enum) => {
                let code = r#enum.codegen()?;
                namespace.push_element(code);

                let ns = namespace.get_ident();
                let name = format_ident!("{}", r#enum.name);

                Some(quote! {
                    #ns::#name
                })
            }
            Self::Array(array) => {
                let inner_ty = array.codegen(namespace)?;

                Some(quote! {
                    Vec<#inner_ty>
                })
            }
            Self::Nested(nested) => {
                let code = nested.codegen()?;
                namespace.push_element(code);

                let ns = namespace.get_ident();
                let name = format_ident!("{}", nested.name);

                Some(quote! {
                    #ns::#name
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub nullable: bool,
    pub r#type: PropertyType,
}

impl Property {
    pub fn from_schema(
        name: &str,
        required: bool,
        schema: &OpenApiType,
        schemas: &IndexMap<&str, OpenApiType>,
    ) -> Option<Self> {
        let name = name.to_owned();
        let description = schema.description.as_deref().map(ToOwned::to_owned);

        match schema {
            OpenApiType {
                r#enum: Some(_), ..
            } => Some(Self {
                r#type: PropertyType::Enum(Enum::from_schema(
                    &name.clone().to_upper_camel_case(),
                    schema,
                )?),
                name,
                description,
                required,
                nullable: false,
            }),
            OpenApiType {
                one_of: Some(types),
                ..
            } => match types.as_slice() {
                [
                    left,
                    OpenApiType {
                        r#type: Some("null"),
                        ..
                    },
                ] => {
                    let mut inner = Self::from_schema(&name, required, left, schemas)?;
                    inner.nullable = true;
                    Some(inner)
                }
                [
                    left @ ..,
                    OpenApiType {
                        r#type: Some("null"),
                        ..
                    },
                ] => {
                    let rest = OpenApiType {
                        one_of: Some(left.to_owned()),
                        ..schema.clone()
                    };
                    let mut inner = Self::from_schema(&name, required, &rest, schemas)?;
                    inner.nullable = true;
                    Some(inner)
                }
                cases => {
                    let r#enum = Enum::from_one_of(&name.to_upper_camel_case(), cases)?;
                    Some(Self {
                        name,
                        description: None,
                        required,
                        nullable: false,
                        r#type: PropertyType::Enum(r#enum),
                    })
                }
            },
            OpenApiType {
                all_of: Some(types),
                ..
            } => {
                let composite = Object::from_all_of(&name.to_upper_camel_case(), types, schemas)?;
                Some(Self {
                    name,
                    description: None,
                    required,
                    nullable: false,
                    r#type: PropertyType::Nested(Box::new(composite)),
                })
            }
            OpenApiType {
                r#type: Some("object"),
                ..
            } => Some(Self {
                r#type: PropertyType::Nested(Box::new(Object::from_schema_object(
                    &name.clone().to_upper_camel_case(),
                    schema,
                    schemas,
                )?)),
                name,
                description,
                required,
                nullable: false,
            }),
            OpenApiType {
                ref_path: Some(path),
                ..
            } => Some(Self {
                name,
                description,
                r#type: PropertyType::Ref((*path).to_owned()),
                required,
                nullable: false,
            }),
            OpenApiType {
                r#type: Some("array"),
                items: Some(items),
                ..
            } => {
                let inner = Self::from_schema(&name, required, items, schemas)?;

                Some(Self {
                    name,
                    description,
                    required,
                    nullable: false,
                    r#type: PropertyType::Array(Box::new(inner.r#type)),
                })
            }
            OpenApiType {
                r#type: Some(_), ..
            } => {
                let prim = match (schema.r#type, schema.format) {
                    (Some("integer"), Some("int32")) => PrimitiveType::I32,
                    (Some("integer"), Some("int64")) => PrimitiveType::I64,
                    (Some("number"), Some("float")) => PrimitiveType::Float,
                    (Some("string"), None) => PrimitiveType::String,
                    (Some("boolean"), None) => PrimitiveType::Bool,
                    _ => return None,
                };

                Some(Self {
                    name,
                    description,
                    required,
                    nullable: false,
                    r#type: PropertyType::Primitive(prim),
                })
            }
            _ => None,
        }
    }

    pub fn codegen(&self, namespace: &mut ObjectNamespace) -> Option<TokenStream> {
        let desc = self.description.as_ref().map(|d| quote! { #[doc = #d]});

        let name = &self.name;
        let (name, serde_attr) = match name.as_str() {
            "type" => (format_ident!("r#type"), None),
            name if name != name.to_snake_case() => (
                format_ident!("{}", name.to_snake_case()),
                Some(quote! { #[serde(rename = #name)]}),
            ),
            _ => (format_ident!("{name}"), None),
        };

        let ty_inner = self.r#type.codegen(namespace)?;

        let ty = if !self.required || self.nullable {
            quote! { Option<#ty_inner> }
        } else {
            ty_inner
        };

        Some(quote! {
            #desc
            #serde_attr
            pub #name: #ty
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Object {
    pub name: String,
    pub description: Option<String>,
    pub properties: Vec<Property>,
}

impl Object {
    pub fn from_schema_object(
        name: &str,
        schema: &OpenApiType,
        schemas: &IndexMap<&str, OpenApiType>,
    ) -> Option<Self> {
        let mut result = Object {
            name: name.to_owned(),
            description: schema.description.as_deref().map(ToOwned::to_owned),
            ..Default::default()
        };

        let Some(props) = &schema.properties else {
            return None;
        };

        let required = schema.required.clone().unwrap_or_default();

        for (prop_name, prop) in props {
            // HACK: This will cause a duplicate key otherwise
            if *prop_name == "itemDetails" {
                continue;
            }

            // TODO: implement custom enum for this (depends on overrides being added)
            if *prop_name == "value" && name == "TornHof" {
                continue;
            }

            result.properties.push(Property::from_schema(
                prop_name,
                required.contains(prop_name),
                prop,
                schemas,
            )?);
        }

        Some(result)
    }

    pub fn from_all_of(
        name: &str,
        types: &[OpenApiType],
        schemas: &IndexMap<&str, OpenApiType>,
    ) -> Option<Self> {
        let mut result = Self {
            name: name.to_owned(),
            ..Default::default()
        };

        for r#type in types {
            let r#type = if let OpenApiType {
                ref_path: Some(path),
                ..
            } = r#type
            {
                let name = path.strip_prefix("#/components/schemas/")?;
                schemas.get(name)?
            } else {
                r#type
            };
            let obj = Self::from_schema_object(name, r#type, schemas)?;

            result.description = result.description.or(obj.description);
            result.properties.extend(obj.properties);
        }

        Some(result)
    }

    pub fn codegen(&self) -> Option<TokenStream> {
        let doc = self.description.as_ref().map(|d| {
            quote! {
                #[doc = #d]
            }
        });

        let mut namespace = ObjectNamespace {
            object: self,
            ident: None,
            elements: Vec::default(),
        };

        let mut props = Vec::with_capacity(self.properties.len());
        for prop in &self.properties {
            props.push(prop.codegen(&mut namespace)?);
        }

        let name = format_ident!("{}", self.name);
        let ns = namespace.codegen();

        Some(quote! {
            #ns

            #doc
            #[derive(Debug, Clone, PartialEq, serde::Deserialize)]
            pub struct #name {
                #(#props),*
            }
        })
    }
}

pub struct ObjectNamespace<'o> {
    object: &'o Object,
    ident: Option<Ident>,
    elements: Vec<TokenStream>,
}

impl ObjectNamespace<'_> {
    pub fn get_ident(&mut self) -> Ident {
        self.ident
            .get_or_insert_with(|| {
                let name = self.object.name.to_snake_case();
                format_ident!("{name}")
            })
            .clone()
    }

    pub fn push_element(&mut self, el: TokenStream) {
        self.elements.push(el);
    }

    pub fn codegen(mut self) -> Option<TokenStream> {
        if self.elements.is_empty() {
            None
        } else {
            let ident = self.get_ident();
            let elements = self.elements;
            Some(quote! {
                pub mod #ident {
                    #(#elements)*
                }
            })
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::openapi::schema::OpenApiSchema;

    #[test]
    fn resolve_object() {
        let schema = OpenApiSchema::read().unwrap();

        let attack = schema.components.schemas.get("FactionUpgrades").unwrap();

        let resolved =
            Object::from_schema_object("FactionUpgrades", attack, &schema.components.schemas)
                .unwrap();
        let _code = resolved.codegen().unwrap();
    }

    #[test]
    fn resolve_objects() {
        let schema = OpenApiSchema::read().unwrap();

        let mut objects = 0;
        let mut unresolved = vec![];

        for (name, desc) in &schema.components.schemas {
            if desc.r#type == Some("object") {
                objects += 1;
                if Object::from_schema_object(name, desc, &schema.components.schemas).is_none() {
                    unresolved.push(name);
                }
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to resolve {}/{} objects. Could not resolve [{}]",
                unresolved.len(),
                objects,
                unresolved
                    .into_iter()
                    .map(|u| format!("`{u}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}
