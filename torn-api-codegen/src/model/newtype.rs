use quote::{format_ident, quote};

use crate::openapi::r#type::OpenApiType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewtypeInner {
    Str,
    I32,
    I64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Newtype {
    pub name: String,
    pub description: Option<String>,
    pub inner: NewtypeInner,
    pub copy: bool,
    pub ord: bool,
}

impl Newtype {
    pub fn from_schema(name: &str, schema: &OpenApiType) -> Option<Self> {
        let name = name.to_owned();
        let description = schema.description.as_deref().map(ToOwned::to_owned);

        match schema {
            OpenApiType {
                r#type: Some("string"),
                ..
            } => Some(Self {
                name,
                description,
                inner: NewtypeInner::Str,
                copy: false,
                ord: false,
            }),
            OpenApiType {
                r#type: Some("integer"),
                format: Some("int32"),
                ..
            } => Some(Self {
                name,
                description,
                inner: NewtypeInner::I32,
                copy: true,
                ord: true,
            }),
            OpenApiType {
                r#type: Some("integer"),
                format: Some("int64"),
                ..
            } => Some(Self {
                name,
                description,
                inner: NewtypeInner::I64,
                copy: true,
                ord: true,
            }),
            _ => None,
        }
    }

    pub fn codegen(&self) -> Option<proc_macro2::TokenStream> {
        let mut derives = vec![quote! { Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize }];

        if self.copy {
            derives.push(quote! { Copy });
        }

        if self.ord {
            derives.push(quote! { PartialOrd, Ord });
        }

        let name = format_ident!("{}", self.name);
        let inner = match self.inner {
            NewtypeInner::Str => format_ident!("String"),
            NewtypeInner::I32 => format_ident!("i32"),
            NewtypeInner::I64 => format_ident!("i64"),
        };

        let doc = self.description.as_ref().map(|d| {
            quote! {
                #[doc = #d]
            }
        });

        let body = quote! {
            #doc
            #[derive(#(#derives),*)]
            pub struct #name(pub #inner);

            impl #name {
                pub fn new(inner: #inner) -> Self {
                    Self(inner)
                }

                pub fn into_inner(self) -> #inner {
                    self.0
                }
            }

            impl From<#inner> for #name {
                fn from(inner: #inner) -> Self {
                    Self(inner)
                }
            }

            impl From<#name> for #inner {
                fn from(outer: #name) -> Self {
                    outer.0
                }
            }

            impl std::fmt::Display for #name {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(f, "{}", self.0)
                }
            }
        };

        Some(body)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::openapi::schema::OpenApiSchema;

    #[test]
    fn codegen() {
        let schema = OpenApiSchema::read().unwrap();

        let user_id = schema.components.schemas.get("UserId").unwrap();

        let mut newtype = Newtype::from_schema("UserId", user_id).unwrap();

        newtype.description = Some("Description goes here".to_owned());

        let code = newtype.codegen().unwrap().to_string();

        panic!("{code}");
    }
}
