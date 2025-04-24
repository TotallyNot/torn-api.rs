use std::{env, fs, path::Path};

use proc_macro2::TokenStream;
use torn_api_codegen::{
    model::{parameter::Parameter, path::Path as ApiPath, resolve, scope::Scope},
    openapi::schema::OpenApiSchema,
};

const DENY_LIST: &[&str] = &[];

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let model_dest = Path::new(&out_dir).join("models.rs");
    let params_dest = Path::new(&out_dir).join("parameters.rs");
    let requests_dest = Path::new(&out_dir).join("requests.rs");
    let scopes_dest = Path::new(&out_dir).join("scopes.rs");

    let schema = OpenApiSchema::read().unwrap();

    let mut models_code = TokenStream::new();

    for (name, model) in &schema.components.schemas {
        if DENY_LIST.contains(name) {
            continue;
        }
        let model = resolve(model, name, &schema.components.schemas);
        if let Some(new_code) = model.codegen() {
            models_code.extend(new_code);
        }
    }

    let models_file = syn::parse2(models_code).unwrap();
    let models_pretty = prettyplease::unparse(&models_file);
    fs::write(&model_dest, models_pretty).unwrap();

    let mut params_code = TokenStream::new();

    for (name, param) in &schema.components.parameters {
        if let Some(code) = Parameter::from_schema(name, param).unwrap().codegen() {
            params_code.extend(code);
        }
    }

    let params_file = syn::parse2(params_code).unwrap();
    let params_pretty = prettyplease::unparse(&params_file);
    fs::write(&params_dest, params_pretty).unwrap();

    let mut requests_code = TokenStream::new();
    let mut paths = Vec::new();
    for (name, path) in &schema.paths {
        let Some(path) = ApiPath::from_schema(name, path, &schema.components.parameters) else {
            continue;
        };
        if let Some(code) = path.codegen_request() {
            requests_code.extend(code);
        }
        paths.push(path);
    }

    let requests_file = syn::parse2(requests_code).unwrap();
    let requests_pretty = prettyplease::unparse(&requests_file);
    fs::write(&requests_dest, requests_pretty).unwrap();

    let mut scope_code = TokenStream::new();
    let scopes = Scope::from_paths(paths);
    for scope in scopes {
        if let Some(code) = scope.codegen() {
            scope_code.extend(code);
        }
    }

    let scopes_file = syn::parse2(scope_code).unwrap();
    let scopes_pretty = prettyplease::unparse(&scopes_file);
    fs::write(&scopes_dest, scopes_pretty).unwrap();
}
