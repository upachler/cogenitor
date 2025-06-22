extern crate proc_macro;

use proc_macro::TokenStream;

mod procmacro;

#[proc_macro]
pub fn generate_api(input: TokenStream) -> TokenStream {
    let config = match procmacro::parse_config(proc_macro2::TokenStream::from(input)) {
        Ok(config) => config,
        Err(e) => return e.to_compile_error().into(),
    };
    procmacro::generate_code(config).into()
}
