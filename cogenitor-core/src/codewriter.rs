use std::ops::Deref;

use anyhow::anyhow;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use rust_format::Formatter;
use thiserror::Error;

use crate::codemodel::{
    Attr, AttrListBuilder, Codemodel, EnumVariantData, FunctionListBuilder, Indirection, NamedItem,
    TraitRef, TypeRef, TypeRefOrTokenStream, function::Function, implementation::Implementation,
};

// useful read on working with proc_macro2, quote and syn:
// https://petanode.com/posts/rust-proc-macro/

pub(crate) fn write_to_token_stream(
    cm: &Codemodel,
    crate_name: &str,
) -> anyhow::Result<TokenStream> {
    let mod_ = cm
        .find_crate(crate_name)
        .ok_or(anyhow!(format!("crate {crate_name} not found")))?;

    let mut type_decls = Vec::new();
    for t in mod_.type_iter() {
        type_decls.push(write_type_decl(t)?);
    }

    let mut trait_decls = Vec::new();
    for t in mod_.trait_iter() {
        trait_decls.push(write_trait_decl(t)?);
    }

    let mut impl_decls = Vec::new();
    for impl_block in mod_.implementations_iter() {
        impl_decls.push(write_implementation(impl_block)?);
    }

    let mut ts = TokenStream::new();
    ts.extend(trait_decls);
    ts.extend(type_decls);
    ts.extend(impl_decls);
    Ok(ts)
}

fn tokenize_attrs<'a>(attr_iter: impl Iterator<Item = &'a Attr>) -> TokenStream {
    let mut ts = TokenStream::new();
    for attr in attr_iter {
        let attr_name = syn::parse_str::<syn::Path>(attr.path().as_str()).unwrap();
        let attr_input = attr.input();
        ts.extend(quote!(#[#attr_name #attr_input]));
    }
    ts
}

fn write_type_decl(type_ref: &TypeRef) -> anyhow::Result<TokenStream> {
    let ts = match type_ref {
        TypeRef::Struct(s) => {
            let struct_name = format_ident!("{}", s.name());
            let mut struct_fields = Vec::new();

            let attrs = tokenize_attrs(s.attr_iter());
            for f in s.field_iter() {
                let field_name = Ident::new(&f.name(), Span::call_site());
                let syn_type_ref = match f.type_() {
                    TypeRefOrTokenStream::TypeRef(type_ref) => syn_type_name_of(type_ref)?,
                    TypeRefOrTokenStream::TokenStream(token_stream) => token_stream.clone(),
                };
                let field_type: TokenStream = syn_type_ref.to_token_stream();
                struct_fields.push(quote!(pub #field_name: #field_type));
            }
            quote!(
                #attrs
                pub struct #struct_name {
                #(#struct_fields),*
            })
        }
        TypeRef::Enum(e) => {
            let enum_name = format_ident!("{}", e.name());
            let mut enum_variants = Vec::new();

            for variant in e.variant_iter() {
                let variant_name = format_ident!("{}", variant.name());
                match variant.data() {
                    EnumVariantData::Unit => {
                        enum_variants.push(quote!(#variant_name));
                    }
                    EnumVariantData::Tuple(types) => {
                        let mut variant_types = Vec::new();
                        for t_or_ts in types {
                            let syn_type_ref = match t_or_ts {
                                TypeRefOrTokenStream::TokenStream(token_stream) => {
                                    token_stream.clone()
                                }
                                TypeRefOrTokenStream::TypeRef(type_ref) => {
                                    syn_type_name_of(type_ref)?
                                }
                            };
                            variant_types.push(syn_type_ref);
                        }
                        enum_variants.push(quote!(#variant_name(#(#variant_types),*)));
                    }
                    EnumVariantData::Struct(fields) => {
                        let mut variant_fields = Vec::new();
                        for f in fields {
                            let field_name = Ident::new(&f.name(), Span::call_site());
                            let syn_type_ref = match f.type_() {
                                TypeRefOrTokenStream::TypeRef(type_ref) => {
                                    syn_type_name_of(type_ref)?
                                }
                                TypeRefOrTokenStream::TokenStream(token_stream) => {
                                    token_stream.clone()
                                }
                            };
                            variant_fields.push(quote!(#field_name: #syn_type_ref));
                        }
                        enum_variants.push(quote!(#variant_name { #(#variant_fields),* }));
                    }
                }
            }
            let attrs = tokenize_attrs(e.attr_iter());
            quote!(
                #attrs
                pub enum #enum_name {
                    #(#enum_variants),*
                }
            )
        }
        TypeRef::Alias(alias) => {
            let alias_name = Ident::new(&alias.name(), Span::call_site());
            let target_name = syn_type_name_of(alias.target())?;
            quote!(type #alias_name = #target_name;)
        }
        TypeRef::Indirection(ind) => match ind.borrow().deref() {
            Indirection::Stub(_) => todo!("unresolved stub {ind:?}"),
            Indirection::Resolved(type_ref) => write_type_decl(type_ref)?,
        },
        _ => return Err(anyhow!("unsupported type declaration {type_ref:?}")),
    };
    Ok(ts)
}

fn write_trait_decl(t: &TraitRef) -> anyhow::Result<TokenStream> {
    let trait_name = format_ident!("{}", t.name());
    let mut function_tokens = Vec::new();

    for func in t.function_iter() {
        function_tokens.push(write_function(func, false, None)?);
    }

    let attrs = tokenize_attrs(t.attr_iter());
    let ts = quote!(
        #attrs
        pub trait #trait_name {
            #(#function_tokens)*
        }
    );
    Ok(ts)
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct FormattingError(#[from] rust_format::Error);

pub fn fmt_code(ts: proc_macro2::TokenStream) -> Result<String, FormattingError> {
    let formatter = rust_format::RustFmt::default();
    Ok(formatter.format_tokens(ts)?)
}

fn syn_type_name_of(type_ref: &TypeRef) -> anyhow::Result<TokenStream> {
    let syn_type = syn::parse_str::<syn::Type>(&type_ref.name())?;
    let ts = syn_type.to_token_stream();
    Ok(ts)
}

fn write_implementation(impl_block: &Implementation) -> anyhow::Result<TokenStream> {
    let type_name = syn_type_name_of(&impl_block.implementing_type)?;
    let mut function_tokens = Vec::new();

    let is_pub = !impl_block.impl_trait.is_some();
    for func in &impl_block.associated_functions {
        function_tokens.push(write_function(func, is_pub, func.body().map(Clone::clone))?);
    }

    match &impl_block.impl_trait {
        Some(trait_ref) => {
            let trait_name = syn::parse_str::<syn::Type>(trait_ref.name().as_ref())?;
            Ok(quote! {
                impl #trait_name for #type_name {
                    #(#function_tokens)*
                }
            })
        }
        None => Ok(quote! {
            impl #type_name {
                #(#function_tokens)*
            }
        }),
    }
}

fn write_function(
    func: &Function,
    is_pub: bool,
    body: Option<TokenStream>,
) -> anyhow::Result<TokenStream> {
    let func_name = format_ident!("{}", func.name());
    let return_type = syn_type_name_of(func.return_type())?;

    let mut params = Vec::new();
    for param in func.function_params_iter() {
        let param_name = format_ident!("{}", param.name);
        let param_type = syn_type_name_of(&param.type_)?;
        params.push(quote!(#param_name: #param_type));
    }

    let access = if is_pub { Some(quote!(pub)) } else { None };

    let body = body.map(|body| quote!({ #body })).unwrap_or(quote!(;));
    Ok(quote! {
        #access fn #func_name(#(#params),*) -> #return_type #body
    })
}

#[test]
fn test_write_code() -> anyhow::Result<()> {
    use crate::codemodel::{Module, StructBuilder};
    use assert_tokenstreams_eq::assert_tokenstreams_eq;

    let mut cm = Codemodel::new();
    let mut m = Module::new("crate");

    // forward declare 'Bar'
    let bar_t = m.insert_type_stub("Bar")?;

    // insert an alias to forward-declared 'Bar'
    let bar_alias_t = m.insert_type_alias("BarAlias", bar_t.clone())?;

    // insert 'Foo' that also references 'Bar';
    let foo_struct = StructBuilder::new("Foo")
        .field("bar", bar_t)?
        .field("bar_alias", bar_alias_t)?
        .field("name", cm.type_string())?
        .field(
            "other_names",
            cm.type_instance(&cm.type_vec(), &vec![cm.type_string()]),
        )?
        .field("zab", cm.type_u8())?
        .build()?;
    m.insert_struct(foo_struct)?;

    let bar_struct = StructBuilder::new("Bar")
        .field("has_handles", cm.type_bool())?
        .build()?;
    m.insert_struct(bar_struct)?;

    cm.insert_crate(m)?;

    let ts = write_to_token_stream(&cm, "crate")?;
    println!("{ts}");

    let ts_reference = quote!(
        pub struct Bar {
            pub has_handles: bool,
        }
        type BarAlias = Bar;
        pub struct Foo {
            pub bar: Bar,
            pub bar_alias: BarAlias,
            pub name: String,
            pub other_names: Vec<String>,
            pub zab: u8,
        }
    );
    assert_tokenstreams_eq!(&ts, &ts_reference);
    Ok(())
}

#[test]
fn test_write_struct_with_serde() -> anyhow::Result<()> {
    use crate::codemodel::{Module, StructBuilder};
    use assert_tokenstreams_eq::assert_tokenstreams_eq;

    let mut cm = Codemodel::new();
    let mut m = Module::new("crate");

    // insert 'Foo' that also references 'Bar';
    let foo_struct = StructBuilder::new("Foo")
        .attr_with_input("derive", quote!((serde::Deserialize)))?
        .field("bar", cm.type_bool())?
        .build()?;

    m.insert_struct(foo_struct)?;

    cm.insert_crate(m)?;

    let ts = write_to_token_stream(&cm, "crate")?;
    println!("{ts}");

    assert_tokenstreams_eq!(
        &ts,
        &quote!(
            #[derive(serde::Deserialize)]
            pub struct Foo {
                pub bar: bool,
            }
        )
    );
    Ok(())
}

#[test]
fn test_write_enum_code() -> anyhow::Result<()> {
    use crate::codemodel::{EnumBuilder, Module};
    use assert_tokenstreams_eq::assert_tokenstreams_eq;

    let mut cm = Codemodel::new();
    let mut m = Module::new("crate");

    // Create a simple enum with unit variants
    let color_enum = EnumBuilder::new("Color")
        .unit_variant("Red")?
        .unit_variant("Green")?
        .unit_variant("Blue")?
        .build()?;
    m.insert_enum(color_enum)?;

    // Create an enum with tuple variants and both struct variant approaches
    let shape_enum = EnumBuilder::new("Shape")
        .attr_with_input("derive", quote!((Debug)))?
        .unit_variant("Circle")?
        .tuple_variant("Rectangle", vec![cm.type_f64(), cm.type_f64()])?
        // Closure-based field builder
        .struct_variant("Point", |builder| {
            builder.field("x", cm.type_f64())?.field("y", cm.type_f64())
        })?
        .struct_variant("Line", |builder| {
            builder
                .field("start", cm.type_f64())?
                .field("end", cm.type_f64())
        })?
        .build()?;
    m.insert_enum(shape_enum)?;

    cm.insert_crate(m)?;

    let ts = write_to_token_stream(&cm, "crate")?;
    println!("{ts}");

    let ts_reference = quote!(
        pub enum Color {
            Red,
            Green,
            Blue,
        }
        #[derive(Debug)]
        pub enum Shape {
            Circle,
            Rectangle(f64, f64),
            Point { x: f64, y: f64 },
            Line { start: f64, end: f64 },
        }
    );
    assert_tokenstreams_eq!(&ts, &ts_reference);
    Ok(())
}

#[test]
fn test_write_implementation() -> anyhow::Result<()> {
    use crate::codemodel::{
        Module, StructBuilder, function::FunctionBuilder, implementation::ImplementationBuilder,
    };
    use assert_tokenstreams_eq::assert_tokenstreams_eq;

    let mut cm = Codemodel::new();
    let mut m = Module::new("crate");

    // Create multiple structs
    let user_struct = StructBuilder::new("User")
        .field("id", cm.type_u32())?
        .field("name", cm.type_string())?
        .field("email", cm.type_string())?
        .build()?;
    let user_ref = m.insert_struct(user_struct)?;

    let post_struct = StructBuilder::new("Post")
        .field("id", cm.type_u32())?
        .field("title", cm.type_string())?
        .field("author_id", cm.type_u32())?
        .build()?;
    let post_ref = m.insert_struct(post_struct)?;

    // Create implementations for User
    let user_new_fn = FunctionBuilder::new("new".to_string(), user_ref.clone())
        .param("id".to_string(), cm.type_u32())
        .param("name".to_string(), cm.type_string())
        .param("email".to_string(), cm.type_string())
        .build();

    let user_get_id_fn = FunctionBuilder::new("get_id".to_string(), cm.type_u32()).build();

    let user_impl = ImplementationBuilder::new_inherent(user_ref.clone())
        .function(user_new_fn)
        .function(user_get_id_fn)
        .build();

    // Create implementations for Post
    let post_new_fn = FunctionBuilder::new("new".to_string(), post_ref.clone())
        .param("id".to_string(), cm.type_u32())
        .param("title".to_string(), cm.type_string())
        .param("author_id".to_string(), cm.type_u32())
        .build();

    let post_get_author_fn = FunctionBuilder::new("get_author".to_string(), user_ref.clone())
        .param("users".to_string(), cm.type_vec())
        .build();

    let post_impl = ImplementationBuilder::new_inherent(post_ref.clone())
        .function(post_new_fn)
        .function(post_get_author_fn)
        .build();

    // Insert implementations
    m.insert_implementation(user_impl)?;
    m.insert_implementation(post_impl)?;

    cm.insert_crate(m)?;

    let ts = write_to_token_stream(&cm, "crate")?;
    println!("{ts}");

    let ts_reference = quote!(
        pub struct User {
            pub id: u32,
            pub name: String,
            pub email: String,
        }
        pub struct Post {
            pub id: u32,
            pub title: String,
            pub author_id: u32,
        }
        impl User {
            pub fn new(id: u32, name: String, email: String) -> User {
                todo!()
            }
            pub fn get_id() -> u32 {
                todo!()
            }
        }
        impl Post {
            pub fn new(id: u32, title: String, author_id: u32) -> Post {
                todo!()
            }
            pub fn get_author(users: Vec) -> User {
                todo!()
            }
        }
    );
    assert_tokenstreams_eq!(&ts, &ts_reference);
    Ok(())
}

#[test]
fn test_write_trait() -> anyhow::Result<()> {
    use crate::codemodel::{Module, function::FunctionBuilder, trait_::TraitBuilder};
    use assert_tokenstreams_eq::assert_tokenstreams_eq;

    let mut cm = Codemodel::new();
    let mut m = Module::new("crate");

    // Create trait functions
    let get_name_fn = FunctionBuilder::new("get_name".to_string(), cm.type_string()).build();
    let set_name_fn = FunctionBuilder::new("set_name".to_string(), cm.type_unit())
        .param("name".to_string(), cm.type_string())
        .build();
    let get_id_fn = FunctionBuilder::new("get_id".to_string(), cm.type_u32()).build();

    // Create a trait with associated functions
    let identifiable_trait = TraitBuilder::new("Identifiable")
        .attr_with_input("derive", quote!((Debug)))?
        .function(get_name_fn)
        .function(set_name_fn)
        .function(get_id_fn)
        .build()?;

    m.insert_trait(identifiable_trait)?;

    // Create a simpler trait without attributes
    let simple_trait = TraitBuilder::new("Simple")
        .function(FunctionBuilder::new("process".to_string(), cm.type_bool()).build())
        .build()?;

    m.insert_trait(simple_trait)?;

    cm.insert_crate(m)?;

    let ts = write_to_token_stream(&cm, "crate")?;
    println!("{ts}");

    let ts_reference = quote!(
        #[derive(Debug)]
        pub trait Identifiable {
            fn get_name() -> String;
            fn set_name(name: String) -> ();
            fn get_id() -> u32;
        }
        pub trait Simple {
            fn process() -> bool;
        }
    );
    assert_tokenstreams_eq!(&ts, &ts_reference);
    Ok(())
}
