use heck::ToUpperCamelCase;
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::path::{Path, PathSegment};

pub struct Scope {
    pub name: String,
    pub mod_name: String,
    pub members: Vec<Path>,
}

impl Scope {
    pub fn from_paths(paths: Vec<Path>) -> Vec<Scope> {
        let mut map = IndexMap::new();

        for path in paths {
            let Some(PathSegment::Constant(first_seg)) = path.segments.first() else {
                continue;
            };

            map.entry(first_seg.to_owned())
                .or_insert_with(|| Scope {
                    name: format!("{}Scope", first_seg.to_upper_camel_case()),
                    mod_name: first_seg.clone(),
                    members: Vec::new(),
                })
                .members
                .push(path);
        }

        map.into_values().collect()
    }

    pub fn codegen(&self) -> Option<TokenStream> {
        let name = format_ident!("{}", self.name);
        let bulk_name = format_ident!("Bulk{}", self.name);

        let mut functions = Vec::with_capacity(self.members.len());
        let mut bulk_functions = Vec::with_capacity(self.members.len());

        for member in &self.members {
            if let Some(code) = member.codegen_scope_call() {
                functions.push(code);
            }
            if let Some(code) = member.codegen_bulk_scope_call() {
                bulk_functions.push(code);
            }
        }

        Some(quote! {
            pub struct #name<E>(E)
            where
                E: crate::executor::Executor;

            impl<E> #name<E>
            where
                E: crate::executor::Executor
            {
                pub fn new(executor: E) -> Self {
                    Self(executor)
                }

                #(#functions)*
            }

            pub struct #bulk_name<E> where
                E: crate::executor::BulkExecutor,
            {
                executor: E,
            }

            impl<E> #bulk_name<E>
            where
                E: crate::executor::BulkExecutor
            {
                pub fn new(executor: E) -> Self {
                    Self {
                        executor,
                    }
                }

                #(#bulk_functions)*
            }
        })
    }
}
