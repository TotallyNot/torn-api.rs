use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::openapi::{
    parameter::OpenApiParameterSchema,
    r#type::{OpenApiType, OpenApiVariants},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumRepr {
    U8,
    U32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumVariantTupleValue {
    Ref(String),
}

impl EnumVariantTupleValue {
    pub fn from_schema(schema: &OpenApiType) -> Option<Self> {
        if let OpenApiType {
            ref_path: Some(path),
            ..
        } = schema
        {
            Some(Self::Ref((*path).to_owned()))
        } else {
            None
        }
    }

    pub fn name(&self) -> Option<&str> {
        let Self::Ref(path) = self;

        path.strip_prefix("#/components/schemas/")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumVariantValue {
    Repr(u32),
    String { rename: Option<String> },
    Tuple(Vec<EnumVariantTupleValue>),
}

impl Default for EnumVariantValue {
    fn default() -> Self {
        Self::String { rename: None }
    }
}

impl EnumVariantValue {
    pub fn codegen_display(&self, name: &str) -> Option<TokenStream> {
        match self {
            Self::Repr(i) => Some(quote! { write!(f, "{}", #i) }),
            Self::String { rename } => {
                let name = rename.as_deref().unwrap_or(name);
                Some(quote! { write!(f, #name) })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnumVariant {
    pub name: String,
    pub description: Option<String>,
    pub value: EnumVariantValue,
}

impl EnumVariant {
    pub fn codegen(&self) -> Option<TokenStream> {
        let doc = self.description.as_ref().map(|d| {
            quote! {
                #[doc = #d]
            }
        });

        let name = format_ident!("{}", self.name);

        match &self.value {
            EnumVariantValue::Repr(repr) => Some(quote! {
                #doc
                #name = #repr
            }),
            EnumVariantValue::String { rename } => {
                let serde_attr = rename.as_ref().map(|r| {
                    quote! {
                        #[serde(rename = #r)]
                    }
                });

                Some(quote! {
                    #doc
                    #serde_attr
                    #name
                })
            }
            EnumVariantValue::Tuple(values) => {
                let mut val_tys = Vec::with_capacity(values.len());

                for value in values {
                    let ty_name = value.name()?;
                    let ty_name = format_ident!("{ty_name}");

                    val_tys.push(quote! {
                        crate::models::#ty_name
                    });
                }

                Some(quote! {
                    #name(#(#val_tys),*)
                })
            }
        }
    }

    pub fn codegen_display(&self) -> Option<TokenStream> {
        let rhs = self.value.codegen_display(&self.name)?;
        let name = format_ident!("{}", self.name);

        Some(quote! {
            Self::#name => #rhs
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Enum {
    pub name: String,
    pub description: Option<String>,
    pub repr: Option<EnumRepr>,
    pub copy: bool,
    pub display: bool,
    pub untagged: bool,
    pub variants: Vec<EnumVariant>,
}

impl Enum {
    pub fn from_schema(name: &str, schema: &OpenApiType) -> Option<Self> {
        let mut result = Enum {
            name: name.to_owned(),
            description: schema.description.as_deref().map(ToOwned::to_owned),
            copy: true,
            ..Default::default()
        };

        match &schema.r#enum {
            Some(OpenApiVariants::Int(int_variants)) => {
                result.repr = Some(EnumRepr::U32);
                result.display = true;
                result.variants = int_variants
                    .iter()
                    .copied()
                    .map(|i| EnumVariant {
                        name: format!("Variant{i}"),
                        value: EnumVariantValue::Repr(i as u32),
                        ..Default::default()
                    })
                    .collect();
            }
            Some(OpenApiVariants::Str(str_variants)) => {
                result.display = true;
                result.variants = str_variants
                    .iter()
                    .copied()
                    .map(|s| {
                        let transformed = s.replace('&', "And").to_upper_camel_case();
                        EnumVariant {
                            value: EnumVariantValue::String {
                                rename: (transformed != s).then(|| s.to_owned()),
                            },
                            name: transformed,
                            ..Default::default()
                        }
                    })
                    .collect();
            }
            None => return None,
        }

        Some(result)
    }

    pub fn from_parameter_schema(name: &str, schema: &OpenApiParameterSchema) -> Option<Self> {
        let mut result = Self {
            name: name.to_owned(),
            copy: true,
            display: true,
            ..Default::default()
        };

        for var in schema.r#enum.as_ref()? {
            let transformed = var.to_upper_camel_case();
            result.variants.push(EnumVariant {
                value: EnumVariantValue::String {
                    rename: (transformed != *var).then(|| transformed.clone()),
                },
                name: transformed,
                ..Default::default()
            });
        }

        Some(result)
    }

    pub fn from_one_of(name: &str, schemas: &[OpenApiType]) -> Option<Self> {
        let mut result = Self {
            name: name.to_owned(),
            untagged: true,
            ..Default::default()
        };

        for schema in schemas {
            let value = EnumVariantTupleValue::from_schema(schema)?;
            let name = value.name()?.to_owned();

            result.variants.push(EnumVariant {
                name,
                value: EnumVariantValue::Tuple(vec![value]),
                ..Default::default()
            });
        }

        Some(result)
    }

    pub fn codegen(&self) -> Option<TokenStream> {
        let repr = self.repr.map(|r| match r {
            EnumRepr::U8 => quote! { #[repr(u8)]},
            EnumRepr::U32 => quote! { #[repr(u32)]},
        });
        let name = format_ident!("{}", self.name);
        let desc = self.description.as_ref().map(|d| {
            quote! {
                #repr
                #[doc = #d]
            }
        });

        let mut display = Vec::with_capacity(self.variants.len());
        let mut variants = Vec::with_capacity(self.variants.len());
        for variant in &self.variants {
            variants.push(variant.codegen()?);

            if self.display {
                display.push(variant.codegen_display()?);
            }
        }

        let mut derives = vec![];

        if self.copy {
            derives.extend_from_slice(&["Copy", "Hash"]);
        }

        let derives = derives.into_iter().map(|d| format_ident!("{d}"));

        let serde_attr = self.untagged.then(|| {
            quote! {
                #[serde(untagged)]
            }
        });

        let display = self.display.then(|| {
            quote! {
                impl std::fmt::Display for #name {
                    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        match self {
                            #(#display),*
                        }
                    }
                }
            }
        });

        Some(quote! {
            #desc
            #[derive(Debug, Clone, PartialEq, serde::Deserialize, #(#derives),*)]
            #serde_attr
            pub enum #name {
                #(#variants),*
            }
            #display
        })
    }
}

#[cfg(test)]
mod test {
    use crate::openapi::schema::OpenApiSchema;

    use super::*;

    #[test]
    fn codegen() {
        let schema = OpenApiSchema::read().unwrap();

        let revive_setting = schema.components.schemas.get("ReviveSetting").unwrap();

        let r#enum = Enum::from_schema("ReviveSetting", revive_setting).unwrap();

        let code = r#enum.codegen().unwrap();

        panic!("{code}");
    }
}
