use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};

#[proc_macro_derive(ApiCategory, attributes(api))]
pub fn derive_api_category(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_api_category(&ast)
}

#[derive(Debug)]
enum ApiField {
    Property(syn::Ident),
    Flattened,
}

#[derive(Debug)]
struct ApiAttribute {
    field: ApiField,
    name: syn::Ident,
    raw_value: String,
    variant: syn::Ident,
    type_name: proc_macro2::TokenStream,
    with: Option<syn::Ident>,
}

fn get_lit_string(lit: syn::Lit) -> String {
    match lit {
        syn::Lit::Str(lit) => lit.value(),
        _ => panic!("Expected api attribute to be a string"),
    }
}

fn impl_api_category(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let enum_ = match &ast.data {
        syn::Data::Enum(data) => data,
        _ => panic!("ApiCategory can only be derived for enums"),
    };

    let mut category: Option<String> = None;
    ast.attrs
        .iter()
        .filter(|a| a.path.is_ident("api"))
        .for_each(|a| {
            if let Ok(syn::Meta::List(l)) = a.parse_meta() {
                for nested in l.nested {
                    match nested {
                        syn::NestedMeta::Meta(syn::Meta::NameValue(m))
                            if m.path.is_ident("category") =>
                        {
                            category = Some(get_lit_string(m.lit))
                        }
                        _ => panic!("unknown api attribute"),
                    }
                }
            }
        });

    let category = category.expect("`category`");

    let fields: Vec<_> = enum_
        .variants
        .iter()
        .filter_map(|variant| {
            for attr in &variant.attrs {
                if attr.path.is_ident("api") {
                    let meta = attr.parse_meta();
                    match meta {
                        Ok(syn::Meta::List(l)) => {
                            let mut type_: Option<String> = None;
                            let mut field: Option<ApiField> = None;
                            let mut with: Option<String> = None;
                            for nested in l.nested.into_iter() {
                                match nested {
                                    syn::NestedMeta::Meta(syn::Meta::NameValue(m))
                                        if m.path.is_ident("type") =>
                                    {
                                        if type_.is_none() {
                                            type_ = Some(get_lit_string(m.lit));
                                        } else {
                                            panic!("type can only be specified once");
                                        }
                                    }
                                    syn::NestedMeta::Meta(syn::Meta::NameValue(m))
                                        if m.path.is_ident("with") =>
                                    {
                                        if with.is_none() {
                                            with = Some(get_lit_string(m.lit));
                                        } else {
                                            panic!("with can only be specified once");
                                        }
                                    }
                                    syn::NestedMeta::Meta(syn::Meta::NameValue(m))
                                        if m.path.is_ident("field") =>
                                    {
                                        if field.is_none() {
                                            field = Some(ApiField::Property(quote::format_ident!(
                                                "{}",
                                                get_lit_string(m.lit)
                                            )));
                                        } else {
                                            panic!("field/flatten can only be specified once");
                                        }
                                    }
                                    syn::NestedMeta::Meta(syn::Meta::Path(m))
                                        if m.is_ident("flatten") =>
                                    {
                                        if field.is_none() {
                                            field = Some(ApiField::Flattened);
                                        } else {
                                            panic!("field/flatten can only be specified once");
                                        }
                                    }
                                    _ => panic!("Couldn't parse api attribute"),
                                }
                            }
                            let name =
                                format_ident!("{}", variant.ident.to_string().to_case(Case::Snake));
                            let raw_value = variant.ident.to_string().to_lowercase();

                            return Some(ApiAttribute {
                                field: field.expect("one of field/flatten"),
                                name,
                                raw_value,
                                variant: variant.ident.clone(),
                                type_name: type_
                                    .expect("Need to specify type name")
                                    .parse()
                                    .expect("failed to parse type name"),
                                with: with.map(|w| format_ident!("{}", w)),
                            });
                        }
                        _ => panic!("Couldn't parse api attribute"),
                    }
                }
            }
            None
        })
        .collect();

    let accessors = fields.iter().map(
        |ApiAttribute {
             field,
             name,
             type_name,
             with,
             ..
         }| match (field, with) {
            (ApiField::Property(prop), None) => {
                let prop_str = prop.to_string();
                quote! {
                    pub fn #name(&self) -> serde_json::Result<#type_name> {
                        self.0.decode_field(#prop_str)
                    }
                }
            }
            (ApiField::Property(prop), Some(f)) => {
                let prop_str = prop.to_string();
                quote! {
                    pub fn #name(&self) -> serde_json::Result<#type_name> {
                        self.0.decode_field_with(#prop_str, #f)
                    }
                }
            }
            (ApiField::Flattened, None) => quote! {
                pub fn #name(&self) -> serde_json::Result<#type_name> {
                    self.0.decode()
                }
            },
            (ApiField::Flattened, Some(_)) => todo!(),
        },
    );

    let raw_values = fields.iter().map(
        |ApiAttribute {
             variant, raw_value, ..
         }| {
            quote! {
                #name::#variant => #raw_value
            }
        },
    );

    let gen = quote! {
        pub struct Response(crate::ApiResponse);

        impl Response {
            #(#accessors)*
        }

        impl crate::ApiCategoryResponse for Response {
            type Selection = #name;

            fn from_response(response: crate::ApiResponse) -> Self {
                Self(response)
            }
        }

        impl crate::ApiSelection for #name {
            fn raw_value(&self) -> &'static str {
                match self {
                    #(#raw_values,)*
                }
            }

            fn category() -> &'static str {
                #category
            }
        }
    };

    gen.into()
}
