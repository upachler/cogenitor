extern crate proc_macro;
use proc_macro::TokenStream;

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream}, punctuated::Punctuated, spanned::Spanned, token::{Colon, Comma}, Attribute, Expr, ExprLit, LitStr, Meta, MetaNameValue, Path, Token
};

// Structure to hold key-value pair arguments
#[derive(Default)]
pub(super) struct ApiConfig {
    path: Option<String>,
    traits: bool,
    types: bool,
    module_name: Option<String>,
}


trait ExprInto<T> {
    fn expr_into(&self) -> Option<T>;
}

impl ExprInto<String> for Expr {
    fn expr_into(&self) -> Option<String> {
        if let Expr::Lit(ExprLit { attrs: _, lit: syn::Lit::Str(lit_str) }) = self{
            Some(lit_str.value())
        }  else {
            None
        }
    }
}

impl ExprInto<bool> for Expr {
    fn expr_into(&self) -> Option<bool> {
        if let Expr::Lit(ExprLit { attrs: _, lit: syn::Lit::Bool(lit_bool)}) = self {
            Some(lit_bool.value())
        } else {
            None
        }
    }
}

impl Parse for ApiConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut config = Self::default();
        
        // Parse key-value pairs
        let mut kv_pairs = Punctuated::<MetaNameValue, Comma>::parse_terminated(input)?;
        
        for name_value in kv_pairs {
            let name = name_value.path.to_token_stream().to_string();
            
            match name.as_str() {
                "path" => {
                    config.path = Some(name_value.value.expr_into().ok_or(syn::Error::new(name_value.span(), "'path' expects a string literal as argument"))?);
                },
                "traits" => {
                    config.traits = name_value.value.expr_into().ok_or(syn::Error::new(name_value.span(), "'traits' expects a bool as argument"))?;
                },
                "types" => {
                    config.types = name_value.value.expr_into().ok_or(syn::Error::new(name_value.span(), "'types' expects a bool as argument"))?;
                },
                "module_name" => {
                    config.module_name = Some(name_value.value.clone().expr_into().ok_or(syn::Error::new(name_value.span(), "'module_name' expects a string literal as argument"))?);
                },
                _ => {
                    return Err(syn::Error::new(
                        name_value.span(),
                        format!("unknown parameter: {}", name),
                    ));
                }
            }
        }
        
        Ok(config)
    }
}

impl ApiConfig {
    pub fn new_from_path(path: String) -> Self {
        Self {
            path: Some(path),
            ..Self::default()
        }
    }
}

// Main macro implementation
pub(super) fn generate_code(config: ApiConfig) -> TokenStream {
    let module_name = config.module_name.unwrap_or_else(|| "generated_api".to_string());
    let module_ident = Ident::new(&module_name, proc_macro2::Span::call_site());
    
    quote! {
        pub mod #module_ident {
            #![allow(unused_imports)]
            
            use std::path::Path;
            

        }
    }.into()
}

