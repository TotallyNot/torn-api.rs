use std::{fmt::Write, ops::Deref};

use heck::{ToSnakeCase, ToUpperCamelCase};
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::openapi::{
    parameter::OpenApiParameter,
    path::{OpenApiPath, OpenApiPathParameter, OpenApiResponseBody},
};

use super::{
    parameter::{Parameter, ParameterLocation, ParameterType},
    union::Union,
};

#[derive(Debug, Clone)]
pub enum PathSegment {
    Constant(String),
    Parameter { name: String },
}

#[derive(Debug, Clone)]
pub enum PathParameter {
    Inline(Parameter),
    Component(Parameter),
}

#[derive(Debug, Clone)]
pub enum PathResponse {
    Component { name: String },
    // TODO: needs to be implemented
    ArbitraryUnion(Union),
}

#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub name: String,
    pub summary: Option<String>,
    pub description: String,
    pub parameters: Vec<PathParameter>,
    pub response: PathResponse,
}

impl Path {
    pub fn from_schema(
        path: &str,
        schema: &OpenApiPath,
        parameters: &IndexMap<&str, OpenApiParameter>,
    ) -> Option<Self> {
        let mut segments = Vec::new();
        for segment in path.strip_prefix('/')?.split('/') {
            if segment.starts_with('{') && segment.ends_with('}') {
                segments.push(PathSegment::Parameter {
                    name: segment[1..(segment.len() - 1)].to_owned(),
                });
            } else {
                segments.push(PathSegment::Constant(segment.to_owned()));
            }
        }

        let summary = schema.get.summary.as_deref().map(ToOwned::to_owned);
        let description = schema.get.description.deref().to_owned();

        let mut params = Vec::with_capacity(schema.get.parameters.len());
        for parameter in &schema.get.parameters {
            match &parameter {
                OpenApiPathParameter::Link { ref_path } => {
                    let name = ref_path
                        .strip_prefix("#/components/parameters/")?
                        .to_owned();
                    let param = parameters.get(&name.as_str())?;
                    params.push(PathParameter::Component(Parameter::from_schema(
                        &name, param,
                    )?));
                }
                OpenApiPathParameter::Inline(schema) => {
                    let name = schema.name.to_upper_camel_case();
                    let parameter = Parameter::from_schema(&name, schema)?;
                    params.push(PathParameter::Inline(parameter));
                }
            };
        }

        let mut suffixes = vec![];
        let mut name = String::new();

        for seg in &segments {
            match seg {
                PathSegment::Constant(val) => {
                    name.push_str(&val.to_upper_camel_case());
                }
                PathSegment::Parameter { name } => {
                    suffixes.push(format!("For{}", name.to_upper_camel_case()));
                }
            }
        }

        for suffix in suffixes {
            name.push_str(&suffix);
        }

        let response = match &schema.get.response_content {
            OpenApiResponseBody::Schema(link) => PathResponse::Component {
                name: link
                    .ref_path
                    .strip_prefix("#/components/schemas/")?
                    .to_owned(),
            },
            OpenApiResponseBody::Union { any_of: _ } => PathResponse::ArbitraryUnion(
                Union::from_schema("Response", &schema.get.response_content)?,
            ),
        };

        Some(Self {
            segments,
            name,
            summary,
            description,
            parameters: params,
            response,
        })
    }

    pub fn codegen_request(&self) -> Option<TokenStream> {
        let name = if self.segments.len() == 1 {
            let Some(PathSegment::Constant(first)) = self.segments.first() else {
                return None;
            };
            format_ident!("{}Request", first.to_upper_camel_case())
        } else {
            format_ident!("{}Request", self.name)
        };

        let mut ns = PathNamespace {
            path: self,
            ident: None,
            elements: Vec::new(),
        };

        let mut fields = Vec::with_capacity(self.parameters.len());
        let mut convert_field = Vec::with_capacity(self.parameters.len());
        let mut start_fields = Vec::new();
        let mut discriminant = Vec::new();
        let mut discriminant_val = Vec::new();
        let mut fmt_val = Vec::new();

        for param in &self.parameters {
            let (is_inline, param) = match &param {
                PathParameter::Inline(param) => (true, param),
                PathParameter::Component(param) => (false, param),
            };

            let (ty, builder_param) = match &param.r#type {
                ParameterType::I32 { .. } | ParameterType::Enum { .. } => {
                    let ty_name = format_ident!("{}", param.name);

                    if is_inline {
                        ns.push_element(param.codegen()?);
                        let path = ns.get_ident();

                        (
                            quote! {
                                crate::request::models::#path::#ty_name
                            },
                            Some(quote! { #[builder(into)] }),
                        )
                    } else {
                        (
                            quote! {
                                crate::parameters::#ty_name
                            },
                            Some(quote! { #[builder(into)]}),
                        )
                    }
                }
                ParameterType::String => (quote! { String }, None),
                ParameterType::Boolean => (quote! { bool }, None),
                ParameterType::Schema { type_name } => {
                    let ty_name = format_ident!("{}", type_name);

                    (
                        quote! {
                            crate::models::#ty_name
                        },
                        None,
                    )
                }
                ParameterType::Array { .. } => {
                    ns.push_element(param.codegen()?);
                    let ty_name = param.r#type.codegen_type_name(&param.name);
                    let path = ns.get_ident();
                    (
                        quote! {
                            crate::request::models::#path::#ty_name
                        },
                        Some(quote! { #[builder(into)] }),
                    )
                }
            };

            let name = format_ident!("{}", param.name.to_snake_case());
            let query_val = &param.value;

            if param.location == ParameterLocation::Path {
                discriminant.push(ty.clone());
                discriminant_val.push(quote! { self.#name });
                let path_name = format_ident!("{}", param.value);
                start_fields.push(quote! {
                    #[builder(start_fn)]
                    #builder_param
                    pub #name: #ty
                });
                fmt_val.push(quote! {
                    #path_name=self.#name
                });
            } else {
                let ty = if param.required {
                    convert_field.push(quote! {
                        .chain(std::iter::once(&self.#name).map(|v| (#query_val, v.to_string())))
                    });
                    ty
                } else {
                    convert_field.push(quote! {
                        .chain(self.#name.as_ref().into_iter().map(|v| (#query_val, v.to_string())))
                    });
                    quote! { Option<#ty>}
                };

                fields.push(quote! {
                    #builder_param
                    pub #name: #ty
                });
            }
        }

        let response_ty = match &self.response {
            PathResponse::Component { name } => {
                let name = format_ident!("{name}");
                quote! {
                    crate::models::#name
                }
            }
            PathResponse::ArbitraryUnion(union) => {
                let path = ns.get_ident();
                let ty_name = format_ident!("{}", union.name);

                quote! {
                    crate::request::models::#path::#ty_name
                }
            }
        };

        let mut path_fmt_str = String::new();
        for seg in &self.segments {
            match seg {
                PathSegment::Constant(val) => _ = write!(path_fmt_str, "/{}", val),
                PathSegment::Parameter { name } => _ = write!(path_fmt_str, "/{{{}}}", name),
            }
        }

        if let PathResponse::ArbitraryUnion(union) = &self.response {
            ns.push_element(union.codegen()?);
        }

        let ns = ns.codegen();

        start_fields.extend(fields);

        Some(quote! {
            #ns

            #[derive(Debug, Clone, bon::Builder)]
            #[builder(state_mod(vis = "pub(crate)"), on(String, into))]
            pub struct #name {
                #(#start_fields),*
            }

            impl crate::request::IntoRequest for #name {
                #[allow(unused_parens)]
                type Discriminant = (#(#discriminant),*);
                type Response = #response_ty;
                fn into_request(self) -> crate::request::ApiRequest<Self::Discriminant> {
                    #[allow(unused_parens)]
                    crate::request::ApiRequest {
                        path: format!(#path_fmt_str, #(#fmt_val),*),
                        parameters: std::iter::empty()
                            #(#convert_field)*
                            .collect(),
                        disriminant:  (#(#discriminant_val),*),
                    }
                }
            }
        })
    }

    pub fn codegen_scope_call(&self) -> Option<TokenStream> {
        let mut extra_args = Vec::new();
        let mut disc = Vec::new();

        let snake_name = self.name.to_snake_case();

        let request_name = format_ident!("{}Request", self.name);
        let builder_name = format_ident!("{}RequestBuilder", self.name);
        let builder_mod_name = format_ident!("{}_request_builder", snake_name);
        let request_mod_name = format_ident!("{snake_name}");

        let request_path = quote! { crate::request::models::#request_name };
        let builder_path = quote! { crate::request::models::#builder_name };
        let builder_mod_path = quote! { crate::request::models::#builder_mod_name };

        let tail = snake_name
            .split_once('_')
            .map_or_else(|| "for_selections".to_owned(), |(_, tail)| tail.to_owned());

        let fn_name = format_ident!("{tail}");

        for param in &self.parameters {
            let (param, is_inline) = match param {
                PathParameter::Inline(param) => (param, true),
                PathParameter::Component(param) => (param, false),
            };

            if param.location == ParameterLocation::Path {
                let ty = match &param.r#type {
                    ParameterType::I32 { .. } | ParameterType::Enum { .. } => {
                        let ty_name = format_ident!("{}", param.name);

                        if is_inline {
                            quote! {
                                crate::request::models::#request_mod_name::#ty_name
                            }
                        } else {
                            quote! {
                                crate::parameters::#ty_name
                            }
                        }
                    }
                    ParameterType::String => quote! { String },
                    ParameterType::Boolean => quote! { bool },
                    ParameterType::Schema { type_name } => {
                        let ty_name = format_ident!("{}", type_name);

                        quote! {
                            crate::models::#ty_name
                        }
                    }
                    ParameterType::Array { .. } => param.r#type.codegen_type_name(&param.name),
                };

                let arg_name = format_ident!("{}", param.value.to_snake_case());

                extra_args.push(quote! { #arg_name: #ty, });
                disc.push(arg_name);
            }
        }

        let response_ty = match &self.response {
            PathResponse::Component { name } => {
                let name = format_ident!("{name}");
                quote! {
                    crate::models::#name
                }
            }
            PathResponse::ArbitraryUnion(union) => {
                let name = format_ident!("{}", union.name);
                quote! {
                    crate::request::models::#request_mod_name::#name
                }
            }
        };

        Some(quote! {
            pub async fn #fn_name<S>(
                &self,
                #(#extra_args)*
                builder: impl FnOnce(
                    #builder_path<#builder_mod_path::Empty>
                ) -> #builder_path<S>,
            ) -> Result<#response_ty, E::Error>
            where
                S: #builder_mod_path::IsComplete,
            {
                let r = builder(#request_path::builder(#(#disc),*)).build();

                self.0.fetch(r).await
            }
        })
    }
}

pub struct PathNamespace<'r> {
    path: &'r Path,
    ident: Option<Ident>,
    elements: Vec<TokenStream>,
}

impl PathNamespace<'_> {
    pub fn get_ident(&mut self) -> Ident {
        self.ident
            .get_or_insert_with(|| {
                let name = self.path.name.to_snake_case();
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
    fn resolve_paths() {
        let schema = OpenApiSchema::read().unwrap();

        let mut paths = 0;
        let mut unresolved = vec![];

        for (name, desc) in &schema.paths {
            paths += 1;
            if Path::from_schema(name, desc, &schema.components.parameters).is_none() {
                unresolved.push(name);
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to resolve {}/{} paths. Could not resolve [{}]",
                unresolved.len(),
                paths,
                unresolved
                    .into_iter()
                    .map(|u| format!("`{u}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    #[test]
    fn codegen_paths() {
        let schema = OpenApiSchema::read().unwrap();

        let mut paths = 0;
        let mut unresolved = vec![];

        for (name, desc) in &schema.paths {
            paths += 1;
            let Some(path) = Path::from_schema(name, desc, &schema.components.parameters) else {
                unresolved.push(name);
                continue;
            };

            if path.codegen_scope_call().is_none() || path.codegen_request().is_none() {
                unresolved.push(name);
            }
        }

        if !unresolved.is_empty() {
            panic!(
                "Failed to codegen {}/{} paths. Could not resolve [{}]",
                unresolved.len(),
                paths,
                unresolved
                    .into_iter()
                    .map(|u| format!("`{u}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}
