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

fn impl_api_category(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let enum_ = match &ast.data {
        syn::Data::Enum(data) => data,
        _ => panic!("ApiCategory can only be derived for enums"),
    };

    let mut category: Option<String> = None;
    for attr in &ast.attrs {
        if attr.path().is_ident("api") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("category") {
                    let c: syn::LitStr = meta.value()?.parse()?;
                    category = Some(c.value());
                    Ok(())
                } else {
                    Err(meta.error("unknown attribute"))
                }
            })
            .unwrap();
        }
    }

    let category = category.expect("`category`");

    let fields: Vec<_> = enum_
        .variants
        .iter()
        .filter_map(|variant| {
            let mut r#type: Option<String> = None;
            let mut field: Option<ApiField> = None;
            let mut with: Option<proc_macro2::Ident> = None;
            for attr in &variant.attrs {
                if attr.path().is_ident("api") {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("type") {
                            let t: syn::LitStr = meta.value()?.parse()?;
                            r#type = Some(t.value());
                            Ok(())
                        } else if meta.path.is_ident("with") {
                            let w: syn::LitStr = meta.value()?.parse()?;
                            with = Some(quote::format_ident!("{}", w.value()));
                            Ok(())
                        } else if meta.path.is_ident("field") {
                            let f: syn::LitStr = meta.value()?.parse()?;
                            field = Some(ApiField::Property(quote::format_ident!("{}", f.value())));
                            Ok(())
                        } else if meta.path.is_ident("flatten") {
                            field = Some(ApiField::Flattened);
                            Ok(())
                        } else {
                            Err(meta.error("unsupported attribute"))
                        }
                    })
                    .unwrap();
                    let name = format_ident!("{}", variant.ident.to_string().to_case(Case::Snake));
                    let raw_value = variant.ident.to_string().to_lowercase();
                    return Some(ApiAttribute {
                        field: field.expect("field or flatten attribute must be specified"),
                        raw_value,
                        variant: variant.ident.clone(),
                        type_name: r#type.expect("type must be specified").parse().unwrap(),
                        name,
                        with,
                    });
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

        impl From<crate::ApiResponse> for Response {
            fn from(value: crate::ApiResponse) -> Self {
                Self(value)
            }
        }

        impl crate::ApiSelection for #name {
            type Response = Response;

            fn raw_value(self) -> &'static str {
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

#[proc_macro_derive(IntoOwned, attributes(into_owned))]
pub fn derive_into_owned(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_into_owned(&ast)
}

fn to_static_lt(ty: &mut syn::Type) -> bool {
    let mut res = false;
    match ty {
        syn::Type::Path(path) => {
            if let Some(syn::PathArguments::AngleBracketed(ab)) = path
                .path
                .segments
                .last_mut()
                .map(|s| &mut s.arguments)
                .as_mut()
            {
                for mut arg in &mut ab.args {
                    match &mut arg {
                        syn::GenericArgument::Type(ty) => {
                            if to_static_lt(ty) {
                                res = true;
                            }
                        }
                        syn::GenericArgument::Lifetime(lt) => {
                            lt.ident = syn::Ident::new("static", proc_macro2::Span::call_site());
                            res = true;
                        }
                        _ => (),
                    }
                }
            }
        }
        syn::Type::Reference(r) => {
            if let Some(lt) = r.lifetime.as_mut() {
                lt.ident = syn::Ident::new("static", proc_macro2::Span::call_site());
                res = true;
            }
            to_static_lt(&mut r.elem);
        }
        _ => (),
    };
    res
}

fn impl_into_owned(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let mut identity = false;
    for attr in &ast.attrs {
        if attr.path().is_ident("into_owned") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("identity") {
                    identity = true;
                    Ok(())
                } else {
                    Err(meta.error("unknown attribute"))
                }
            })
            .unwrap();
        }
    }

    if identity {
        return quote! {
            impl #impl_generics crate::into_owned::IntoOwned for #name #ty_generics #where_clause {
                type Owned = Self;
                fn into_owned(self) -> Self::Owned {
                    self
                }
            }
        }
        .into();
    }

    let syn::Data::Struct(r#struct) = &ast.data else {
        panic!("Only stucts are supported");
    };

    let syn::Fields::Named(named_fields) = &r#struct.fields else {
        panic!("Only named fields are supported");
    };

    let vis = &ast.vis;

    for attr in &ast.attrs {
        if attr.path().is_ident("identity") {
            //
        }
    }

    let mut owned_fields = Vec::with_capacity(named_fields.named.len());
    let mut fields = Vec::with_capacity(named_fields.named.len());

    for field in &named_fields.named {
        let field_name = &field.ident.as_ref().unwrap();
        let mut ty = field.ty.clone();
        let vis = &field.vis;

        if to_static_lt(&mut ty) {
            owned_fields
                .push(quote! { #vis #field_name: <#ty as crate::into_owned::IntoOwned>::Owned });
            fields.push(
                quote! { #field_name: crate::into_owned::IntoOwned::into_owned(self.#field_name) },
            );
        } else {
            owned_fields.push(quote! { #vis #field_name: #ty });
            fields.push(quote! { #field_name: self.#field_name });
        };
    }

    let owned_name = syn::Ident::new(
        &format!("{}Owned", ast.ident),
        proc_macro2::Span::call_site(),
    );

    let gen = quote! {
        #[derive(Debug, Clone)]
        #vis struct #owned_name {
            #(#owned_fields,)*
        }
        impl #impl_generics crate::into_owned::IntoOwned for #name #ty_generics #where_clause {
            type Owned = #owned_name;
            fn into_owned(self) -> Self::Owned {
                #owned_name {
                    #(#fields,)*
                }
            }
        }
    };

    gen.into()
}
