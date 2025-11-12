use anyhow::anyhow;
use codewriter::fmt_code;
use proc_macro2::TokenStream;
use quote::quote;
use rust_format::Formatter;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Cursor, Read, Seek, Write},
};
use syn::Ident;

use codemodel::{AttrListBuilder, Codemodel, Module, StructBuilder, TypeRef};
use types::{BooleanOrSchema, Schema, Spec};

use crate::{
    adapters::oas30::OAS30Spec,
    codemodel::{
        EnumBuilder, FunctionListBuilder, function::FunctionBuilder, trait_::TraitBuilder,
    },
    types::{MediaType, Operation, Parameter, PathItem, RefOr, RequestBody, Response, StatusSpec},
};

pub mod codemodel;
mod codewriter;
mod oasprobe;
mod translate;
mod types;

#[cfg(test)]
mod test;

pub mod adapters;

/// Configuration settings for OpenAPI code generation.
#[derive(Default, Debug, PartialEq)]
pub struct ApiConfig {
    /// Path to the input OpenAPI spec from which we want to generate code from
    pub path: Option<String>,
    /// Name of the module into which the generated code should be placed
    pub module_name: Option<String>,
}

impl ApiConfig {
    pub fn new_from_path(path: String) -> Self {
        Self {
            path: Some(path),
            ..Self::default()
        }
    }
}

pub fn generate_mod(config: &ApiConfig) -> anyhow::Result<TokenStream> {
    let module_name = config
        .module_name
        .as_ref()
        .map(|s| s.clone())
        .unwrap_or_else(|| "generated_api".to_string());
    let module_ident = Ident::new(&module_name, proc_macro2::Span::call_site());

    let ts = generate_token_stream(&config)?;

    let ts = quote! {
        pub mod #module_ident {
            #![allow(unused_imports)]
            #![allow(dead_code)]
            #![allow(unused_variables)]
            #![allow(non_snake_case)]
            #![allow(non_camel_case_types)]

            use std::path::Path;

            #ts
        }
    }
    .into();

    Ok(ts)
}

pub fn generate_token_stream(config: &ApiConfig) -> anyhow::Result<TokenStream> {
    let path = config
        .path
        .as_ref()
        .ok_or(anyhow!("no path to OpenAPI file specified"))?;
    let path = std::path::Path::new(&path);
    let mut file = std::fs::File::open(path)?;

    generate_from_reader(&mut file)
}

pub fn generate_file(config: &ApiConfig, output_path: &std::path::Path) -> anyhow::Result<()> {
    let ts = generate_mod(config)?;
    let formatter = rust_format::RustFmt::default();
    let code_string = formatter.format_tokens(ts)?;

    let mut file = File::create(output_path)?;
    file.write(code_string.as_bytes())?;
    Ok(())
}

#[allow(unused)]
fn generate_from_str<S: Spec>(s: &str) -> anyhow::Result<TokenStream> {
    generate_from_reader(Cursor::new(s.as_bytes()))
}

fn generate_from_reader(input: impl Read + Seek) -> anyhow::Result<TokenStream> {
    let mut input = BufReader::with_capacity(8192, input);
    let version = oasprobe::probe_yaml_oas_version(&mut input).map_err(|e| anyhow!(e))?;
    input.rewind()?;
    match version {
        #[cfg(feature = "oas30")]
        adapters::OASMajorVersion::OAS30 => read_and_gererate::<OAS30Spec>(input),
        #[cfg(feature = "oas31")]
        adapters::OASMajorVersion::OAS31 => read_and_gererate::<OAS31Spec>(input),
    }
}

fn read_and_gererate<S: Spec>(input: impl Read) -> anyhow::Result<TokenStream> {
    let spec = S::from_reader(input)?;
    generate_code(&spec)
}

struct Context<S: Spec> {
    cm: Codemodel,
    m: Module,
    mapping: TypeMapping<S>,
}

fn build_codemodel<S: Spec>(spec: &S) -> anyhow::Result<(Codemodel, TypeMapping<S>)> {
    let mut ctx = Context {
        cm: Codemodel::new(),

        m: Module::new("crate"),

        mapping: TypeMapping::new(),
    };

    populate_types(&mut ctx, spec)?;

    let mut cm = ctx.cm;
    let m = ctx.m;
    cm.insert_crate(m)?;

    Ok((cm, ctx.mapping))
}

fn generate_code<S: Spec>(spec: &S) -> anyhow::Result<TokenStream> {
    let (codemodel, _) = build_codemodel(spec)?;

    let ts = codewriter::write_to_token_stream(&codemodel, "crate")?;

    log::trace!("token stream: \n{}", fmt_code(ts.clone()).unwrap());
    Ok(ts)
}

/** Maps OpenAPI type names to actual Codemodel [TypeRef]s instances */
struct TypeMapping<S: Spec> {
    schema_mapping: HashMap<RefOr<S::Schema>, TypeRef>,
}

impl<S: Spec> TypeMapping<S> {
    fn new() -> Self {
        Self {
            schema_mapping: HashMap::new(),
        }
    }
}

impl<S: Spec> std::fmt::Debug for TypeMapping<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeMapping")
            .field("schema_mapping", &self.schema_mapping)
            .finish()
    }
}

fn populate_types<S: Spec>(ctx: &mut Context<S>, spec: &S) -> anyhow::Result<()> {
    // in order to properly deal with cyclic data structures, we create
    // type stubs for all named schemata. This way, while constructing
    // a type from a schema, we can refer to another type that we
    // didn't construct yet.
    for (name, schema) in spec.schemata_iter() {
        let rust_name = translate::schema_to_rust_typename(&name);
        let type_ref = ctx.m.insert_type_stub(&rust_name)?;
        ctx.mapping.schema_mapping.insert(schema, type_ref);
    }

    log::trace!(
        "types stubs from schemas section constructed: {:?}",
        ctx.mapping
    );

    // we now construct all types properly. When inserting them into
    // the module, stubs are replaced by proper types.
    for (name, ro_schema) in spec.schemata_iter() {
        log::debug!("creating type for schema '{name}");
        match &ro_schema {
            RefOr::Reference(_) => {
                let alias_name = translate::schema_to_rust_typename(&name);
                let target = ctx
                    .mapping
                    .schema_mapping
                    .get(&ro_schema.resolve())
                    .expect("type not found for schema");
                ctx.m.insert_type_alias(&alias_name, target.clone())?;
            }
            RefOr::Object(schema) => {
                let type_ref = parse_schema(ctx, schema, Some(name.clone()))?;
                ctx.mapping.schema_mapping.insert(ro_schema, type_ref);
            }
        }
    }

    log::trace!("types from schemas section constructed: {:?}", ctx.mapping);

    let client_struct = StructBuilder::new("ClientImpl")
        .attr_with_input("derive", quote::quote!((Debug)))?
        .build()?;
    //let client_struct = ctx.m.insert_struct(client_struct)?;

    let mut client_trait = TraitBuilder::new("Client");
    //    let mut client_impl = ImplementationBuilder::new_trait(client_struct, client_trait);
    for (path, path_item) in spec.paths() {
        for (method, path_op) in path_item.operations_iter() {
            log::debug!("creating method for {method} {path}");
            client_trait = parse_path_into_impl_fn(
                ctx,
                client_trait,
                &path,
                &path_item,
                method.clone(),
                &path_op,
            )?;
        }
    }
    //    ctx.m.insert_implementation(client_impl.build())?;
    ctx.m.insert_trait(client_trait.build()?)?;

    Ok(())
}

/** The rust type we're converting a JSON schema item into */
#[derive(Debug)]
enum TypeKind {
    Enum, // a rust enum generated from the strings in the 'enum' keyword
    Struct,
    Builtin,
    String,
    Json,
    HashMap(Box<TypeKind>),
}

fn type_kind_of(schema: &impl Schema) -> anyhow::Result<TypeKind> {
    let kind: TypeKind;

    if let Some(types) = schema.type_() {
        if types.len() != 1 {
            kind = TypeKind::Json;
        } else {
            match types.get(0).unwrap() {
                types::Type::Object => {
                    if !schema.properties().is_empty() {
                        // if there are properties, it'll always be a struct
                        kind = TypeKind::Struct;
                    } else {
                        // without defined properties, we check the 'additionalProperties' and
                        // 'patternProperties' - if we have those, we'll create a HashMap
                        if let BooleanOrSchema::Boolean(true) = schema.addtional_properties() {
                            kind = TypeKind::HashMap(Box::new(TypeKind::Json))
                        } else {
                            // empty struct - because there are no properties
                            kind = TypeKind::Struct
                        }
                        // FIXME: we're ignoring patternProperties for now...
                    }
                }
                types::Type::String => {
                    if let Some(_e) = schema.enum_() {
                        kind = TypeKind::Enum;
                    } else {
                        kind = TypeKind::String
                    }
                }

                _ => {
                    todo!("type unimplemented");
                }
            }
        }
    } else {
        kind = TypeKind::Json
    }

    Ok(kind)
}

fn parse_schema<S: Spec>(
    ctx: &mut Context<S>,
    schema: &S::Schema,
    name: Option<String>,
) -> anyhow::Result<TypeRef> {
    let kind = type_kind_of(schema)?;

    match &kind {
        TypeKind::Struct => {
            let struct_name = name.as_ref().unwrap();
            let mut b = StructBuilder::new(struct_name);
            b = b
                .attr_with_input(
                    "derive",
                    quote::quote!((
                        ::std::fmt::Debug,
                        ::serde::Serialize,
                        ::serde::Deserialize,
                        ::core::cmp::PartialEq
                    )),
                )
                .unwrap();
            let required: HashSet<&str> = schema
                .required()
                .unwrap_or_default()
                .iter()
                .map(|e| *e)
                .collect();
            for (name, schema) in schema.properties() {
                let rust_name = translate::property_to_rust_fieldname(&name);
                let schema = schema.resolve();
                let candidate_name =
                    translate::schema_to_rust_typename((struct_name.to_string() + &name).as_str());
                let property_type_ref = type_ref_of(ctx, &schema, &candidate_name)?;
                let actual_type_ref;
                // if property is required, use the type directly, otherwise wrap it in option
                if required.contains(name.as_str()) {
                    actual_type_ref = property_type_ref;
                } else {
                    actual_type_ref = ctx
                        .cm
                        .type_instance(&ctx.cm.type_option(), &[property_type_ref]);
                }
                b = b.field(&rust_name, actual_type_ref)?;
            }
            let s = b.build()?;
            Ok(ctx.m.insert_struct(s)?)
        }
        /*TypeKind::String => {
            let string_type = cm.type_string(&self);
            if let Some(name) = name {
                return cm.build_type_alias(name, string_type)?;
            } else {
                return string_type;
            }
        }*/
        _ => {
            unimplemented!("parsing {schema:?} for kind {kind:?}")
        }
    }
}

fn parse_path_into_impl_fn<S: Spec, B: FunctionListBuilder>(
    ctx: &mut Context<S>,
    impl_builder: B,
    path_name: &str,
    path_item: &S::PathItem,
    method: http::Method,
    path_op: &S::Operation,
) -> anyhow::Result<B> {
    let candidate_name = translate::path_method_to_rust_fn_name(&method, path_name)?;

    let fn_name = candidate_name; // FIXME: handle collisions

    let return_type = parse_into_fn_result(ctx, path_name, path_item, method.clone(), path_op)?;
    let mut function = FunctionBuilder::new(fn_name, return_type)
        .param("self".to_string(), ctx.cm.type_ref_self());

    // Parameters in path_op can override those in path_item, so
    // we apply the non-shadowed of path_item first
    let outer_params = path_item
        .parameters()
        .map(|param| param.resolve_fully())
        .filter(|param| {
            let shadowed = path_op
                .parameters()
                .map(|p| p.resolve_fully())
                .any(|p| p.name() == param.name() && p.in_() == param.in_());
            !shadowed
        })
        .collect::<Vec<_>>();

    fn param_type_name_fn<S: Spec>(param: &S::Parameter) -> String {
        param.name().to_owned()
    }

    for param in outer_params {
        function = append_param(ctx, function, &param, param_type_name_fn::<S>)?;
    }

    for param in path_op.parameters() {
        function = append_param(
            ctx,
            function,
            &param.resolve_fully(),
            param_type_name_fn::<S>,
        )?;
    }

    // add request body as function parameter if defined
    if let Some(request_body) = path_op.request_body() {
        // closure to build name from {operationFragment}Content pattern
        // - called if needed.
        let op_fragment_content_fn =
            || translate::path_method_to_rust_type_name(method.clone(), path_name) + "Content";
        let type_ref = map_content(
            ctx,
            &request_body.resolve_fully().content(),
            op_fragment_content_fn,
        )?;
        let body_param_name = derive_function_param_name("body", &function);
        function = function.param(body_param_name, type_ref);
    }

    Ok(impl_builder.function(function.build()))
}

fn parse_into_fn_result<S: Spec>(
    ctx: &mut Context<S>,
    path_name: &str,
    path_item: &S::PathItem,
    method: http::Method,
    path_op: &S::Operation,
) -> anyhow::Result<TypeRef> {
    let success_type = build_response_type(ctx, path_name, method.clone(), path_op, true)?;
    let error_type = build_response_type(ctx, path_name, method, path_op, false)?;

    Ok(TypeRef::GenericInstance {
        generic_type: Box::new(ctx.cm.type_result()),
        type_parameter: vec![success_type, error_type], // FIXME: need to assign proper result type params
    })
}

fn build_response_type<S: Spec>(
    ctx: &mut Context<S>,
    path_name: &str,
    method: http::Method,
    path_op: &S::Operation,
    build_for_success: bool,
) -> anyhow::Result<TypeRef> {
    fn is_success(status_spec: StatusSpec) -> bool {
        match status_spec {
            types::StatusSpec::Informational(_)
            | types::StatusSpec::Informational1XX
            | types::StatusSpec::Success(_)
            | types::StatusSpec::Success2XX
            | types::StatusSpec::Redirection(_)
            | types::StatusSpec::Redirection3XX => true,
            _ => false,
        }
    }

    let responses = path_op
        .responses()
        .filter(|(status_spec, _)| match status_spec {
            types::StatusSpec::Default => true,
            s => !build_for_success ^ is_success(s.clone()),
        })
        .collect::<Vec<_>>();

    let resonses_name_suffix: &str = if build_for_success {
        "Success"
    } else {
        "Error"
    };

    let type_ref = match (build_for_success, responses.len()) {
        (true, 0) => ctx.cm.type_unit(),
        (true, 1) => {
            let single_response = responses.get(0).unwrap();
            let status_spec = single_response.0.clone();
            let content = single_response.1.resolve().resolve_fully().content();
            map_content(ctx, &content, || {
                content_enum_name(&method, path_name, &status_spec)
            })?
        }
        _ => {
            let enum_name = translate::path_method_to_rust_type_name(method.clone(), path_name)
                + resonses_name_suffix;
            let mut e = EnumBuilder::new(&enum_name);

            for (status_spec, response) in responses {
                let status_spec = &status_spec;
                let content = response.resolve_fully().content();
                let variant_name = translate::status_spec_to_rust_type_name(status_spec.clone());
                let variant_type = map_content(ctx, &content, || {
                    content_enum_name(&method, path_name, &status_spec)
                })?;
                e = e.tuple_variant(&variant_name, vec![variant_type])?;
            }

            if !build_for_success {
                e = e.tuple_variant_with_input(
                    "UnknownResponse",
                    vec![quote!(::http::Response<::std::vec::Vec<u8>>)],
                )?;
                e = e.tuple_variant_with_input(
                    "OtherError",
                    vec![quote!(::std::boxed::Box<dyn ::std::error::Error>)],
                )?
            }

            ctx.m.insert_enum(e.build()?)?
        }
    };
    Ok(type_ref)
}

fn content_enum_name(method: &http::Method, path_name: &str, status_spec: &StatusSpec) -> String {
    let prefix = translate::path_method_to_rust_type_name(method.clone(), path_name);
    prefix + translate::status_spec_to_rust_type_name(status_spec.clone()).as_str()
}

fn map_content<S: Spec>(
    ctx: &mut Context<S>,
    content: &HashMap<String, S::MediaType>,
    content_name_fn: impl Fn() -> String,
) -> anyhow::Result<TypeRef> {
    let mapped_type;
    match content.len() {
        0 => mapped_type = ctx.cm.type_unit(),
        1 => {
            let (media_type_key, media_type) = content.iter().next().unwrap();
            mapped_type = map_media_type::<S>(ctx, media_type_key, media_type, content_name_fn);
        }
        _ => {
            mapped_type = map_enum_from_content::<S>(ctx, content, content_name_fn)?;
        }
    };
    Ok(mapped_type)
}

fn map_enum_from_content<S: Spec>(
    ctx: &mut Context<S>,
    content: &HashMap<String, S::MediaType>,
    content_name_fn: impl Fn() -> String,
) -> anyhow::Result<TypeRef> {
    // TODO: disambiguate!
    let enum_name = content_name_fn();
    let mut e = EnumBuilder::new(&enum_name);

    for (media_type_key, media_type) in content.iter() {
        let variant_name = translate::media_type_range_to_rust_type_name(media_type_key);
        let content_variant_name_fn = || enum_name.clone() + variant_name.as_str();
        let variant_type =
            map_media_type::<S>(ctx, media_type_key, media_type, content_variant_name_fn);
        e = e.tuple_variant(&variant_name, vec![variant_type])?;
    }

    let e = e.build()?;
    Ok(ctx.m.insert_enum(e)?)
}

fn map_media_type<S: Spec>(
    ctx: &mut Context<S>,
    media_type_key: &str,
    media_type: &S::MediaType,
    schema_name_fn: impl Fn() -> String,
) -> TypeRef {
    match media_type.schema() {
        Some(schema) => {
            let schema = schema.resolve();

            match type_ref_of(ctx, &schema, &schema_name_fn()).ok() {
                Some(type_ref) => type_ref,
                None => todo!(),
            }
        }
        None => todo!("emit some type that implements Read<u8>, like BufRead<u8>, etc."),
    }
}

fn derive_function_param_name(name_candidate: &str, function: &FunctionBuilder) -> String {
    let existing_names = function.param_names();

    let mapped_name = translate::parameter_to_rust_fn_param(name_candidate);
    translate::uncollide(&existing_names, mapped_name)
}

/// Append a an OAS operation parameter as rust function parameter
/// to the given FunctionBuilder, while respecting Rust
/// conventions and naming uniqueness constraints. This may lead
/// to different names than those in the spec.
fn append_param<S: Spec>(
    ctx: &mut Context<S>,
    function: FunctionBuilder,
    param: &S::Parameter,
    param_type_name_fn: impl Fn(&S::Parameter) -> String,
) -> anyhow::Result<FunctionBuilder> {
    let mapped_name = derive_function_param_name(param.name(), &function);

    // TODO: params are incredibly complex in OAS. Currently we ignore most
    // of this complexity, however, it may severely impact the way parameters
    // are serialized. See the sections in the spec, starting from here:
    // https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-2-2-fixed-fields-for-use-with-schema
    let candidate_param_type_name = param_type_name_fn(param);
    let mapped_type;
    if let Some(schema) = param.schema() {
        mapped_type = type_ref_of(ctx, &schema, &candidate_param_type_name)?;
    } else if let Some(content) = param.content() {
        mapped_type = map_content(ctx, &content, || candidate_param_type_name.clone())?;
    } else {
        return Err(anyhow!(
            "invariant violated in OAS spec: parameter {} has neither 'schema' nor 'content' defined!",
            param.name()
        ));
    }

    // finally add parameter
    Ok(function.param(mapped_name, mapped_type))
}

fn type_ref_of<S: Spec>(
    ctx: &mut Context<S>,
    schema: &RefOr<S::Schema>,
    candidate_name: &str,
) -> anyhow::Result<TypeRef> {
    if let Some(type_ref) = ctx.mapping.schema_mapping.get(schema) {
        // mapped type found for RefOr
        return Ok(type_ref.clone());
    }

    // If we get there, there are only two options (assuming that we
    // already mapped all types in #/components/schemas, which we should
    // have before calling this function):
    // * the schema is inlined (RefOr::Object) and hasn't been mapped yet
    // * the schema is inlined and directly mapped to a primitive type
    // * the schema is RefOr::Reference, whose URI points to a nonexisting
    //   target (they should all exist because they should have been mapped
    //   before, see above)

    match schema {
        RefOr::Reference(_) => Err(anyhow!(
            "no mapping found for schema URI reference {schema:?}"
        )),
        RefOr::Object(schema) => match &schema.type_() {
            Some(types) => {
                if types.len() != 1 {
                    // violation of rules for 'type' keyword in
                    // https://spec.openapis.org/oas/v3.0.4.html#schema-object
                    return Err(anyhow!(
                        "encountered schema with a type property that has zero or multiple types. It is expected to have exactly one"
                    ));
                } else {
                    match types.get(0).unwrap() {
                        types::Type::Null => Ok(ctx.cm.type_unit()),
                        types::Type::Boolean => Ok(ctx.cm.type_bool()),
                        types::Type::Object => {
                            parse_schema(ctx, schema, Some(candidate_name.to_string()))
                        }
                        types::Type::Array => {
                            // check for violations against rules for 'items' in
                            // https://spec.openapis.org/oas/v3.0.4.html#schema-object
                            let items = schema
                                .items()
                                .ok_or(anyhow!("'items' must be present when 'type' is 'array'"))?;
                            if items.len() != 1 {
                                return Err(anyhow!("'items' must contain exactly one schema"));
                            }
                            let item_schema = items.get(0).unwrap().resolve();
                            let candidate_item_name = candidate_name.to_string() + "Item";
                            let item_type =
                                type_ref_of(ctx, &item_schema, &candidate_item_name).unwrap();
                            Ok(ctx.cm.type_instance(&ctx.cm.type_vec(), &vec![item_type]))
                        }
                        types::Type::Number => match schema.format() {
                            Some(format) => Ok(match format {
                                types::Format::Int32 => ctx.cm.type_i32(),
                                types::Format::Int64 => ctx.cm.type_i64(),
                                types::Format::Float => ctx.cm.type_f32(),
                                types::Format::Double => ctx.cm.type_f64(),
                                _ => ctx.cm.type_f64(),
                            }),
                            None => Ok(ctx.cm.type_f64()),
                        },
                        types::Type::String => {
                            // FIXME: we need to implement enums here!
                            Ok(ctx.cm.type_string())
                        }
                    }
                }
            }
            None => todo!("produced type should refer to json::JSON"),
        },
    }
}
