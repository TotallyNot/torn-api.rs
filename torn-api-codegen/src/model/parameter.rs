use std::fmt::Write;

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

use crate::openapi::parameter::{
    OpenApiParameter, OpenApiParameterDefault, OpenApiParameterSchema,
    ParameterLocation as SchemaLocation,
};

use super::r#enum::Enum;

#[derive(Debug, Clone)]
pub struct ParameterOptions<P> {
    pub default: Option<P>,
    pub minimum: Option<P>,
    pub maximum: Option<P>,
}

#[derive(Debug, Clone)]
pub enum ParameterType {
    I32 {
        options: ParameterOptions<i32>,
    },
    String,
    Boolean,
    Enum {
        options: ParameterOptions<String>,
        r#type: Enum,
    },
    Schema {
        type_name: String,
    },
    Array {
        items: Box<ParameterType>,
    },
}

impl ParameterType {
    pub fn from_schema(name: &str, schema: &OpenApiParameterSchema) -> Option<Self> {
        match schema {
            OpenApiParameterSchema {
                r#type: Some("integer"),
                // BUG: missing for some types in the spec

                // format: Some("int32"),
                ..
            } => {
                let default = match schema.default {
                    Some(OpenApiParameterDefault::Int(d)) => Some(d),
                    None => None,
                    _ => return None,
                };

                Some(Self::I32 {
                    options: ParameterOptions {
                        default,
                        minimum: schema.minimum,
                        maximum: schema.maximum,
                    },
                })
            }
            OpenApiParameterSchema {
                r#type: Some("string"),
                r#enum: Some(variants),
                ..
            } if variants.as_slice() == ["true", "false"]
                || variants.as_slice() == ["false", "true "] =>
            {
                Some(ParameterType::Boolean)
            }
            OpenApiParameterSchema {
                r#type: Some("string"),
                r#enum: Some(_),
                ..
            } => {
                let default = match schema.default {
                    Some(OpenApiParameterDefault::Str(d)) => Some(d.to_owned()),
                    None => None,
                    _ => return None,
                };

                Some(ParameterType::Enum {
                    options: ParameterOptions {
                        default,
                        minimum: None,
                        maximum: None,
                    },
                    r#type: Enum::from_parameter_schema(name, schema)?,
                })
            }
            OpenApiParameterSchema {
                r#type: Some("string"),
                ..
            } => Some(ParameterType::String),
            OpenApiParameterSchema {
                ref_path: Some(path),
                ..
            } => {
                let type_name = path.strip_prefix("#/components/schemas/")?.to_owned();

                Some(ParameterType::Schema { type_name })
            }
            OpenApiParameterSchema {
                r#type: Some("array"),
                items: Some(items),
                ..
            } => Some(Self::Array {
                items: Box::new(Self::from_schema(name, items)?),
            }),
            _ => None,
        }
    }

    pub fn codegen_type_name(&self, name: &str) -> TokenStream {
        match self {
            Self::I32 { .. } | Self::String | Self::Enum { .. } | Self::Array { .. } => {
                format_ident!("{name}").into_token_stream()
            }
            Self::Boolean => quote! { bool },
            Self::Schema { type_name } => {
                let type_name = format_ident!("{type_name}",);
                quote! { crate::models::#type_name }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterLocation {
    Query,
    Path,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub value: String,
    pub description: Option<String>,
    pub r#type: ParameterType,
    pub required: bool,
    pub location: ParameterLocation,
}

impl Parameter {
    pub fn from_schema(name: &str, schema: &OpenApiParameter) -> Option<Self> {
        let name = match name {
            "From" => "FromTimestamp".to_owned(),
            "To" => "ToTimestamp".to_owned(),
            name => name.to_owned(),
        };
        let value = schema.name.to_owned();
        let description = schema.description.as_deref().map(ToOwned::to_owned);

        let location = match &schema.r#in {
            SchemaLocation::Query => ParameterLocation::Query,
            SchemaLocation::Path => ParameterLocation::Path,
        };

        let r#type = ParameterType::from_schema(&name, &schema.schema)?;

        Some(Self {
            name,
            value,
            description,
            r#type,
            required: schema.required,
            location,
        })
    }

    pub fn codegen(&self) -> Option<TokenStream> {
        match &self.r#type {
            ParameterType::I32 { options } => {
                let name = format_ident!("{}", self.name);

                let mut desc = self.description.as_deref().unwrap_or_default().to_owned();

                if options.default.is_some()
                    || options.minimum.is_some()
                    || options.maximum.is_some()
                {
                    _ = writeln!(desc, "\n # Notes");
                }

                let constructor = if let (Some(min), Some(max)) = (options.minimum, options.maximum)
                {
                    _ = write!(desc, "Values have to lie between {min} and {max}. ");
                    let name_raw = &self.name;
                    quote! {
                        impl #name {
                            pub fn new(inner: i32) -> Result<Self, crate::ParameterError> {
                                if inner > #max || inner < #min {
                                    Err(crate::ParameterError::OutOfRange { value: inner, name: #name_raw })
                                } else {
                                    Ok(Self(inner))
                                }
                            }
                        }

                        impl TryFrom<i32> for #name {
                            type Error = crate::ParameterError;
                            fn try_from(inner: i32) -> Result<Self, Self::Error> {
                                if inner > #max || inner < #min {
                                    Err(crate::ParameterError::OutOfRange { value: inner, name: #name_raw })
                                } else {
                                    Ok(Self(inner))
                                }
                            }
                        }
                    }
                } else {
                    quote! {
                        impl #name {
                            pub fn new(inner: i32) -> Self {
                                Self(inner)
                            }
                        }
                    }
                };

                if let Some(default) = options.default {
                    _ = write!(desc, "The default value is {default}.");
                }

                let doc = quote! {
                    #[doc = #desc]
                };

                Some(quote! {
                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    #doc
                    pub struct #name(i32);

                    #constructor

                    impl From<#name> for i32 {
                        fn from(value: #name) -> Self {
                            value.0
                        }
                    }

                    impl #name {
                        pub fn into_inner(self) -> i32 {
                            self.0
                        }
                    }

                    impl std::fmt::Display for #name {
                        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                            write!(f, "{}", self.0)
                        }
                    }
                })
            }
            ParameterType::Enum { options, r#type } => {
                let mut desc = self.description.as_deref().unwrap_or_default().to_owned();
                if let Some(default) = &options.default {
                    let default = default.to_upper_camel_case();
                    _ = write!(
                        desc,
                        r#"
# Notes
The default value [Self::{}](self::{}#variant.{})"#,
                        default, self.name, default
                    );
                }

                let doc = quote! { #[doc = #desc]};
                let inner = r#type.codegen()?;

                Some(quote! {
                    #doc
                    #inner
                })
            }
            ParameterType::Array { items } => {
                let (inner_name, outer_name) = match items.as_ref() {
                    ParameterType::I32 { .. }
                    | ParameterType::String
                    | ParameterType::Array { .. }
                    | ParameterType::Enum { .. } => self.name.strip_suffix('s').map_or_else(
                        || (self.name.to_owned(), format!("{}s", self.name)),
                        |s| (s.to_owned(), self.name.to_owned()),
                    ),
                    ParameterType::Boolean => ("bool".to_owned(), self.name.clone()),
                    ParameterType::Schema { type_name } => (type_name.clone(), self.name.clone()),
                };

                let inner = Self {
                    r#type: *items.clone(),
                    name: inner_name.clone(),
                    ..self.clone()
                };

                let mut code = inner.codegen().unwrap_or_default();

                let name = format_ident!("{}", outer_name);
                let inner_ty = items.codegen_type_name(&inner_name);

                code.extend(quote! {
                    #[derive(Debug, Clone)]
                    pub struct #name(pub Vec<#inner_ty>);

                    impl std::fmt::Display for #name {
                        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                            let mut first = true;
                            for el in &self.0 {
                                if first {
                                    first = false;
                                    write!(f, "{el}")?;
                                } else {
                                    write!(f, ",{el}")?;
                                }
                            }
                            Ok(())
                        }
                    }
                });

                Some(code)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::openapi::{path::OpenApiPathParameter, schema::OpenApiSchema};

    use super::*;

    #[test]
    fn resolve_components() {
        let schema = OpenApiSchema::read().unwrap();

        let mut parameters = 0;
        let mut unresolved = vec![];

        for (name, desc) in &schema.components.parameters {
            parameters += 1;
            if Parameter::from_schema(name, desc).is_none() {
                unresolved.push(name);
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to resolve {}/{} params. Could not resolve [{}]",
                unresolved.len(),
                parameters,
                unresolved
                    .into_iter()
                    .map(|u| format!("`{u}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    #[test]
    fn resolve_inline() {
        let schema = OpenApiSchema::read().unwrap();

        let mut params = 0;
        let mut unresolved = Vec::new();

        for (path, body) in &schema.paths {
            for param in &body.get.parameters {
                if let OpenApiPathParameter::Inline(inline) = param {
                    params += 1;
                    if Parameter::from_schema(inline.name, inline).is_none() {
                        unresolved.push(format!("`{}.{}`", path, inline.name));
                    }
                }
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to resolve {}/{} inline params. Could not resolve [{}]",
                unresolved.len(),
                params,
                unresolved.join(", ")
            )
        }
    }

    #[test]
    fn codegen_inline() {
        let schema = OpenApiSchema::read().unwrap();

        let mut params = 0;
        let mut unresolved = Vec::new();

        for (path, body) in &schema.paths {
            for param in &body.get.parameters {
                if let OpenApiPathParameter::Inline(inline) = param {
                    if inline.r#in == SchemaLocation::Query {
                        let Some(param) = Parameter::from_schema(inline.name, inline) else {
                            continue;
                        };
                        if matches!(
                            param.r#type,
                            ParameterType::Schema { .. }
                                | ParameterType::Boolean
                                | ParameterType::String
                        ) {
                            continue;
                        }
                        params += 1;
                        if param.codegen().is_none() {
                            unresolved.push(format!("`{}.{}`", path, inline.name));
                        }
                    }
                }
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to codegen {}/{} inline params. Could not codegen [{}]",
                unresolved.len(),
                params,
                unresolved.join(", ")
            )
        }
    }
}
