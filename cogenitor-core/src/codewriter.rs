use std::ops::Deref;

use anyhow::anyhow;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};

use crate::codemodel::{Codemodel, Indirection, NamedItem, TypeRef};

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

    let mut ts = TokenStream::new();
    ts.extend(type_decls);
    Ok(ts)
}

fn write_type_decl(type_ref: &TypeRef) -> anyhow::Result<TokenStream> {
    let ts = match type_ref {
        TypeRef::Struct(s) => {
            let struct_name = format_ident!("{}", s.name());
            let mut struct_fields = Vec::new();

            for f in s.field_iter() {
                let field_name = Ident::new(&f.name(), Span::call_site());
                let syn_type_ref = syn_type_name_of(f.type_())?;
                let field_type: TokenStream = syn_type_ref.to_token_stream();
                struct_fields.push(quote!(pub #field_name: #field_type));
            }
            quote!(pub struct #struct_name {
                #(#struct_fields),*
            })
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

fn syn_type_name_of(type_ref: &TypeRef) -> anyhow::Result<TokenStream> {
    let syn_type = syn::parse_str::<syn::Type>(&type_ref.name())?;
    let ts = syn_type.to_token_stream();
    Ok(ts)
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
