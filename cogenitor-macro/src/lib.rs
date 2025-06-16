use proc_macro::TokenStream;
use syn::LitStr;

use crate::procmacro::ApiConfig;

mod procmacro;


#[proc_macro]
pub fn generate_api(input: TokenStream) -> TokenStream {
    // Handle single argument case
    if let Ok(path) = syn::parse::<LitStr>(input.clone()) {
        let config = ApiConfig::new_from_path(path.value());
        procmacro::generate_code(config)
    } else {
        // Handle key-value pairs case
        match syn::parse(input) {
            Ok(config) => procmacro::generate_code(config),
            Err(e) => {
                return e.to_compile_error().into()
            }
        }
    }
}