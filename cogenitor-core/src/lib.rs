use anyhow::anyhow;
use proc_macro2::TokenStream;
use quote::quote;
use std::{collections::HashMap, io::Read, path::Path};
use syn::Ident;

use codemodel::{Codemodel, Module, StructBuilder, TypeRef};
use types::{BooleanOrSchema, Schema, Spec};

use crate::{
    codemodel::{EnumBuilder, function::FunctionBuilder, implementation::ImplementationBuilder},
    types::{MediaType, Operation, Parameter, PathItem, RefOr, RequestBody},
};

pub mod codemodel;
mod codewriter;
mod translate;
mod types;

pub mod adapters;

// Structure to hold key-value pair arguments
#[derive(Default, Debug, PartialEq)]
pub struct ApiConfig {
    pub path: Option<String>,
    pub traits: bool,
    pub types: bool,
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

pub fn generate_mod<S: Spec>(config: ApiConfig) -> anyhow::Result<TokenStream> {
    let module_name = config
        .module_name
        .unwrap_or_else(|| "generated_api".to_string());
    let module_ident = Ident::new(&module_name, proc_macro2::Span::call_site());

    let path = config
        .path
        .ok_or(anyhow!("no path to OpenAPI file specified"))?;
    let path = std::path::Path::new(&path);
    let types = generate_from_path::<S>(path)?;

    let ts = quote! {
        pub mod #module_ident {
            #![allow(unused_imports)]

            use std::path::Path;

            #types
        }
    }
    .into();

    Ok(ts)
}

fn generate_from_path<S: Spec>(path: &Path) -> anyhow::Result<TokenStream> {
    let mut file = std::fs::File::open(path)?;

    generate_from_reader::<S>(&mut file)
}

fn generate_from_str<S: Spec>(s: &str) -> anyhow::Result<TokenStream> {
    let spec = S::from_str(s)?;
    generate_code(&spec)
}

fn generate_from_reader<S: Spec>(input: impl Read) -> anyhow::Result<TokenStream> {
    let spec = S::from_reader(input)?;
    generate_code(&spec)
}

fn build_codemodel<S: Spec>(spec: &S) -> anyhow::Result<(Codemodel, TypeMapping<S>)> {
    let mut cm = Codemodel::new();

    let mut m = Module::new("crate");

    let type_map = populate_types(spec, &mut cm, &mut m)?;
    cm.insert_crate(m)?;

    Ok((cm, type_map))
}

fn generate_code<S: Spec>(spec: &S) -> anyhow::Result<TokenStream> {
    let (codemodel, _) = build_codemodel(spec)?;

    let ts = codewriter::write_to_token_stream(&codemodel, "crate")?;

    println!("token stream: {ts}");
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

fn populate_types<S: Spec>(
    spec: &S,
    cm: &mut Codemodel,
    m: &mut Module,
) -> anyhow::Result<TypeMapping<S>> {
    let mut mapping = TypeMapping::new();

    // in order to properly deal with cyclic data structures, we create
    // type stubs for all named schemata. This way, while constructing
    // a type from a schema, we can refer to another type that we
    // didn't construct yet.
    for (name, schema) in spec.schemata_iter() {
        let rust_name = translate::schema_to_rust_typename(&name);
        let type_ref = m.insert_type_stub(&rust_name)?;
        mapping.schema_mapping.insert(schema, type_ref);
    }

    log::trace!("types stubs from schemas section constructed: {mapping:?}");

    // we now construct all types properly. When inserting them into
    // the module, stubs are replaced by proper types.
    for (name, ro_schema) in spec.schemata_iter() {
        println!("creating type for schema '{name}");
        match &ro_schema {
            RefOr::Reference(_) => {
                let alias_name = translate::schema_to_rust_typename(&name);
                let target = mapping
                    .schema_mapping
                    .get(&ro_schema.resolve())
                    .expect("type not found for schema");
                m.insert_type_alias(&alias_name, target.clone())?;
            }
            RefOr::Object(schema) => {
                let type_ref = parse_schema(schema, Some(name.clone()), cm, m, &mapping)?;
                mapping.schema_mapping.insert(ro_schema, type_ref);
            }
        }
    }

    log::trace!("types from schemas section constructed: {mapping:?}");

    let client_struct = StructBuilder::new("Client")
        .attr_with_input("derive", quote::quote!((Debug)))?
        .build()?;
    let client_struct = m.insert_struct(client_struct)?;

    let mut client_impl = ImplementationBuilder::new_inherent(client_struct);
    for (path, path_item) in spec.paths() {
        for (method, path_op) in path_item.operations_iter() {
            println!("creating method for {method} {path}");
            client_impl = parse_path_into_impl_fn(
                cm,
                m,
                client_impl,
                &mut mapping,
                &path,
                &path_item,
                method,
                &path_op,
            )?;
        }
    }
    m.insert_implementation(client_impl.build())?;

    Ok(mapping)
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
    schema: &S::Schema,
    name: Option<String>,
    cm: &mut Codemodel,
    m: &mut Module,
    mapping: &TypeMapping<S>,
) -> anyhow::Result<TypeRef> {
    let kind = type_kind_of(schema)?;

    match &kind {
        TypeKind::Struct => {
            let mut b = StructBuilder::new(name.as_ref().unwrap());
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
            for (name, schema) in schema.properties() {
                let rust_name = translate::property_to_rust_fieldname(&name);
                let schema = schema.resolve();
                let type_ref = type_ref_of(cm, mapping, &schema)?;
                b = b.field(&rust_name, type_ref)?;
            }
            let s = b.build()?;
            Ok(m.insert_struct(s)?)
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

fn parse_path_into_impl_fn<S: Spec>(
    cm: &mut Codemodel,
    m: &mut Module,
    impl_builder: ImplementationBuilder,
    mapping: &mut TypeMapping<S>,
    path_name: &str,
    path_item: &S::PathItem,
    method: http::Method,
    path_op: &S::Operation,
) -> anyhow::Result<ImplementationBuilder> {
    let candidate_name = translate::path_method_to_rust_fn_name(&method, path_name)?;

    let fn_name = candidate_name; // FIXME: handle collisions

    let return_type = TypeRef::GenericInstance {
        generic_type: Box::new(cm.type_result()),
        type_parameter: vec![cm.type_unit(), cm.type_unit()], // FIXME: need to assign proper result type params
    };
    let mut function =
        FunctionBuilder::new(fn_name, return_type).param("self".to_string(), cm.type_ref_self());

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
        function = append_param(function, cm, m, mapping, &param, param_type_name_fn::<S>)?;
    }

    for param in path_op.parameters() {
        function = append_param(
            function,
            cm,
            m,
            mapping,
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
            cm,
            m,
            mapping,
            &request_body.resolve_fully().content(),
            op_fragment_content_fn,
        )?;
        let body_param_name = derive_function_param_name("body", &function);
        function = function.param(body_param_name, type_ref);
    }

    Ok(impl_builder.function(function.build()))
}

fn map_content<S: Spec>(
    cm: &mut Codemodel,
    m: &mut Module,
    mapping: &mut TypeMapping<S>,
    content: &HashMap<String, S::MediaType>,
    enum_name_fn: impl Fn() -> String,
) -> anyhow::Result<TypeRef> {
    let mapped_type;
    match content.len() {
        0 => mapped_type = cm.type_unit(),
        1 => {
            let (media_type_key, media_type) = content.iter().next().unwrap();
            mapped_type = map_media_type::<S>(cm, mapping, media_type_key, media_type);
        }
        _ => {
            mapped_type = map_enum_from_content::<S>(cm, m, mapping, content, enum_name_fn)?;
        }
    };
    Ok(mapped_type)
}

fn map_enum_from_content<S: Spec>(
    cm: &mut Codemodel,
    m: &mut Module,
    mapping: &mut TypeMapping<S>,
    content: &HashMap<String, S::MediaType>,
    enum_name_fn: impl Fn() -> String,
) -> anyhow::Result<TypeRef> {
    // TODO: disambiguate!
    let enum_name = enum_name_fn();
    let mut e = EnumBuilder::new(&enum_name);

    for (media_type_key, media_type) in content.iter() {
        let variant_name = translate::media_type_range_to_rust_type_name(media_type_key);
        let variant_type = map_media_type::<S>(cm, mapping, media_type_key, media_type);
        e = e.tuple_variant(&variant_name, vec![variant_type])?;
    }

    let e = e.build()?;
    Ok(m.insert_enum(e)?)
}

fn map_media_type<S: Spec>(
    cm: &mut Codemodel,
    mapping: &TypeMapping<S>,
    media_type_key: &str,
    media_type: &S::MediaType,
) -> TypeRef {
    match media_type.schema() {
        Some(schema) => {
            let schema = schema.resolve();
            match type_ref_of(cm, mapping, &schema).ok() {
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
    function: FunctionBuilder,
    cm: &mut Codemodel,
    m: &mut Module,
    mapping: &mut TypeMapping<S>,
    param: &S::Parameter,
    param_content_type_name_fn: impl Fn(&S::Parameter) -> String,
) -> anyhow::Result<FunctionBuilder> {
    let mapped_name = derive_function_param_name(param.name(), &function);

    // TODO: params are incredibly complex in OAS. Currently we ignore most
    // of this complexity, however, it may severely impact the way parameters
    // are serialized. See the sections in the spec, starting from here:
    // https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-2-2-fixed-fields-for-use-with-schema
    let mapped_type;
    if let Some(schema) = param.schema() {
        mapped_type = type_ref_of(cm, mapping, &schema)?;
    } else if let Some(content) = param.content() {
        let enum_content_type_name = || param_content_type_name_fn(param);
        mapped_type = map_content(cm, m, mapping, &content, enum_content_type_name)?;
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
    cm: &mut Codemodel,
    mapping: &TypeMapping<S>,
    schema: &RefOr<S::Schema>,
) -> anyhow::Result<TypeRef> {
    if let Some(type_ref) = mapping.schema_mapping.get(schema) {
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
        RefOr::Reference(r) => Err(anyhow!(
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
                        types::Type::Null => Ok(cm.type_unit()),
                        types::Type::Boolean => Ok(cm.type_bool()),
                        types::Type::Object => {
                            todo!(
                                "inlined schemata are currently unsupported, trying schema {schema:?}"
                            )
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
                            let item_type = type_ref_of(cm, mapping, &item_schema).unwrap();
                            Ok(cm.type_instance(&cm.type_vec(), &vec![item_type]))
                        }
                        types::Type::Number => match schema.format() {
                            Some(format) => Ok(match format {
                                types::Format::Int32 => cm.type_i32(),
                                types::Format::Int64 => cm.type_i64(),
                                types::Format::Float => cm.type_f32(),
                                types::Format::Double => cm.type_f64(),
                                _ => cm.type_f64(),
                            }),
                            None => Ok(cm.type_f64()),
                        },
                        types::Type::String => {
                            // FIXME: we need to implement enums here!
                            Ok(cm.type_string())
                        }
                    }
                }
            }
            None => todo!("produced type should refer to json::JSON"),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, str::FromStr};
    use test_log::test;
    use types::Components;

    use crate::codemodel::{NamedItem, Scope, implementation::Implementation};

    use super::*;

    static PETSTORE_YAML: &[u8] = include_bytes!("../../test-data/petstore.yaml");

    #[test]
    fn test_oas_petstore() {
        let reader = Cursor::new(PETSTORE_YAML);
        super::generate_from_reader::<adapters::oas30::OAS30Spec>(reader)
            .expect("reading petstore.yaml failed");
    }

    #[test]
    fn test_empty() {
        let oas = r"
            openapi: 3.0.0
            info:
                title: Empty API
                version: v1
            paths:
            components:
            ";
        super::generate_from_str::<adapters::oas30::OAS30Spec>(oas).unwrap();
    }

    #[test]
    fn test_simple_pet() -> anyhow::Result<()> {
        let oas = r"
            openapi: 3.0.0
            info:
                title: Empty API
                version: v1
            paths:
            components:
                schemas:
                    Pet:
                        type: object
                        properties:
                            name:
                                type: string
                            species:
                                type: string";

        let spec = adapters::oas30::OAS30Spec::from_str(oas)?;
        assert_eq!(1, spec.schemata_iter().count());
        let (cm, mapping) = super::build_codemodel(&spec)?;
        let pet = spec
            .components()
            .unwrap()
            .schemas()
            .find_map(|(name, schema)| if name.eq("Pet") { Some(schema) } else { None })
            .unwrap();
        assert!(mapping.schema_mapping.contains_key(&pet));
        let crate_ = cm.find_crate("crate").unwrap();
        let pet = crate_.find_type("Pet").unwrap();
        match &pet {
            TypeRef::Struct(s) => {
                assert_eq!(2, s.field_iter().count())
            }
            _ => panic!("struct expected, found {pet:?}"),
        }
        Ok(())
    }

    #[test]
    fn test_simple_fn() -> anyhow::Result<()> {
        let oas = r"
openapi: 3.0.0
info:
    title: test for generating an endpoint function
    version: v1
paths:
    /nothing:
        get:
            responses:
                '204':
                    description: get no response here";

        let spec = adapters::oas30::OAS30Spec::from_str(oas)?;
        assert_eq!(1, spec.paths().count());
        let (cm, _mapping) = super::build_codemodel(&spec)?;
        let crate_ = cm.find_crate("crate").unwrap();
        assert!(crate_.type_iter().any(|t| t.name() == "Client"));
        let the_answer_get_fn = match crate_.implementations_iter().next().unwrap() {
            Implementation::InherentImpl {
                associated_functions,
                implementing_type: _,
            } => associated_functions
                .iter()
                .find(|f| f.name() == "nothing_get"),
        }
        .unwrap();

        assert_eq!(
            1,
            the_answer_get_fn.function_params_iter().count(),
            "function decl object: {the_answer_get_fn:?}"
        );

        Ok(())
    }

    #[test]
    fn test_fn_params() -> anyhow::Result<()> {
        let oas = r"
openapi: 3.0.0
info:
    title: test for generating an endpoint function
    version: v1
paths:
    /pet/findByStatus:
        get:
            tags:
                - pet
            summary: Finds Pets by status.
            description: Multiple status values can be provided with comma separated strings.
            operationId: findPetsByStatus
            parameters:
                -   name: status
                    in: query
                    description: Status values that need to be considered for filter
                    required: false
                    explode: true
                    schema:
                        type: string
                        default: available
                        enum:
                            - available
                            - pending
                            - sold
            responses: {}";

        let spec = adapters::oas30::OAS30Spec::from_str(oas)?;
        assert_eq!(1, spec.paths().count());
        let (cm, _mapping) = super::build_codemodel(&spec)?;
        let crate_ = cm.find_crate("crate").unwrap();
        assert!(crate_.type_iter().any(|t| t.name() == "Client"));
        let the_answer_get_fn = match crate_.implementations_iter().next().unwrap() {
            Implementation::InherentImpl {
                associated_functions,
                implementing_type: _,
            } => associated_functions
                .iter()
                .find(|f| f.name() == "pet_findbystatus_get"),
        }
        .unwrap();

        assert_eq!(
            2,
            the_answer_get_fn.function_params_iter().count(),
            "function decl object: {the_answer_get_fn:?}"
        );

        Ok(())
    }
}
