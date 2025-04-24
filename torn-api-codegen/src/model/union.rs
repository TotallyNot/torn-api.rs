use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::openapi::path::OpenApiResponseBody;

#[derive(Debug, Clone)]
pub struct Union {
    pub name: String,
    pub members: Vec<String>,
}

impl Union {
    pub fn from_schema(name: &str, schema: &OpenApiResponseBody) -> Option<Self> {
        let members = match schema {
            OpenApiResponseBody::Union { any_of } => {
                any_of.iter().map(|l| l.ref_path.to_owned()).collect()
            }
            _ => return None,
        };
        let name = name.to_owned();

        Some(Self { name, members })
    }

    pub fn codegen(&self) -> Option<TokenStream> {
        let name = format_ident!("{}", self.name);
        let mut variants = Vec::new();

        for member in &self.members {
            let variant_name = member.strip_prefix("#/components/schemas/")?;
            let accessor_name = format_ident!("{}", variant_name.to_snake_case());
            let ty_name = format_ident!("{}", variant_name);
            variants.push(quote! {
                pub fn #accessor_name(&self) -> Result<crate::models::#ty_name, serde_json::Error> {
                    <crate::models::#ty_name as serde::Deserialize>::deserialize(&self.0)
                }
            });
        }

        Some(quote! {
            #[derive(Debug, Clone, serde::Deserialize)]
            pub struct #name(serde_json::Value);

            impl #name {
                #(#variants)*
            }
        })
    }
}
