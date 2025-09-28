use cogenitor_core::{ApiConfig, adapters::oas30::OAS30Spec};
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

use syn::{
    Expr, ExprLit, LitStr, MetaNameValue,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
};

// wrapper struct holding ApiConfig, but is parseable
#[derive(Default, Debug, PartialEq)]
pub struct MacroConfig {
    pub inner: ApiConfig,
}

trait ExprInto<T> {
    fn expr_into(&self) -> Option<T>;
}

impl ExprInto<String> for Expr {
    fn expr_into(&self) -> Option<String> {
        if let Expr::Lit(ExprLit {
            attrs: _,
            lit: syn::Lit::Str(lit_str),
        }) = self
        {
            Some(lit_str.value())
        } else {
            None
        }
    }
}

impl ExprInto<bool> for Expr {
    fn expr_into(&self) -> Option<bool> {
        if let Expr::Lit(ExprLit {
            attrs: _,
            lit: syn::Lit::Bool(lit_bool),
        }) = self
        {
            Some(lit_bool.value())
        } else {
            None
        }
    }
}

impl Parse for MacroConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut macro_config = Self::default();
        let config = &mut macro_config.inner;

        // Parse key-value pairs
        let kv_pairs = Punctuated::<MetaNameValue, Comma>::parse_terminated(input)?;

        for name_value in kv_pairs {
            let name = name_value.path.to_token_stream().to_string();

            match name.as_str() {
                "path" => {
                    config.path = Some(name_value.value.expr_into().ok_or(syn::Error::new(
                        name_value.span(),
                        "'path' expects a string literal as argument",
                    ))?);
                }
                "traits" => {
                    config.traits = name_value.value.expr_into().ok_or(syn::Error::new(
                        name_value.span(),
                        "'traits' expects a bool as argument",
                    ))?;
                }
                "types" => {
                    config.types = name_value.value.expr_into().ok_or(syn::Error::new(
                        name_value.span(),
                        "'types' expects a bool as argument",
                    ))?;
                }
                "module_name" => {
                    config.module_name =
                        Some(name_value.value.clone().expr_into().ok_or(syn::Error::new(
                            name_value.span(),
                            "'module_name' expects a string literal as argument",
                        ))?);
                }
                _ => {
                    return Err(syn::Error::new(
                        name_value.span(),
                        format!("unknown parameter: {}", name),
                    ));
                }
            }
        }

        Ok(macro_config)
    }
}

// Main macro implementation
pub(super) fn generate_macro_code(config: ApiConfig) -> TokenStream {
    match cogenitor_core::generate_mod(config) {
        Ok(ts) => ts,
        Err(e) => match e.downcast_ref::<syn::Error>() {
            Some(e) => e.to_compile_error(),
            None => {
                let message = e.to_string();
                syn::Error::new(Span::call_site(), message).to_compile_error()
            }
        },
    }
}

pub(crate) fn parse_config(input: TokenStream) -> syn::Result<ApiConfig> {
    // Handle single argument case
    let config;
    if let Ok(path) = syn::parse2::<LitStr>(input.clone()) {
        config = ApiConfig::new_from_path(path.value());
    } else {
        // Handle key-value pairs case
        let macro_config: MacroConfig = syn::parse2(input)?;
        config = macro_config.inner;
    }
    Ok(config)
}

#[test]
pub fn test_parse_config() {
    let lit_str: LitStr = syn::parse_quote!("Hello\nWorld");
    assert_eq!(lit_str.value(), "Hello\nWorld");

    let macro_args = quote::quote!("/path/to/openapi.yaml");
    let config = parse_config(macro_args).unwrap();
    assert_eq!(
        ApiConfig::new_from_path("/path/to/openapi.yaml".to_string()),
        config
    );

    let macro_args = quote::quote!(path = "/path/to/openapi.yaml", traits = true);
    let config = parse_config(macro_args).unwrap();
    assert_eq!(
        ApiConfig {
            path: Some("/path/to/openapi.yaml".to_string()),
            traits: true,
            ..Default::default()
        },
        config
    );

    // error on unknown params
    let macro_args = quote::quote!(xxx = "/path/to/openapi.yaml");
    parse_config(macro_args).unwrap_err();
}
