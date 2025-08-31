use anyhow::anyhow;
use proc_macro2::TokenStream;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
};

use codemodel::{Codemodel, Module, StructBuilder, TypeRef};
use types::{BooleanOrSchema, Schema, Spec};

use crate::{
    codemodel::{function::FunctionBuilder, implementation::ImplementationBuilder},
    types::{Operation, Parameter, PathItem, Reference},
};

pub mod codemodel;
mod codewriter;
mod translate;
mod types;

pub mod adapters;

pub fn generate_from_path<S: Spec>(path: &Path) -> anyhow::Result<TokenStream> {
    let mut file = std::fs::File::open(path)?;

    generate_from_reader::<S>(&mut file)
}

pub fn generate_from_str<S: Spec>(s: &str) -> anyhow::Result<TokenStream> {
    let spec = S::from_str(s)?;
    generate_code(&spec)
}

pub fn generate_from_reader<S: Spec>(input: impl Read) -> anyhow::Result<TokenStream> {
    let spec = S::from_reader(input)?;
    generate_code(&spec)
}

fn build_codemodel<S: Spec>(spec: &S) -> anyhow::Result<(Codemodel, TypeMapping)> {
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
struct TypeMapping {
    mapping: HashMap<String, TypeRef>,
}

fn populate_types(
    spec: &impl Spec,
    cm: &mut Codemodel,
    m: &mut Module,
) -> anyhow::Result<TypeMapping> {
    let mut mapping = HashMap::new();

    // in order to properly deal with cyclic data structures, we create
    // type stubs for all named schemata. This way, while constructing
    // a type from a schema, we can refer to another type that we
    // didn't construct yet.
    for (name, _) in spec.schemata_iter() {
        let rust_name = translate::schema_to_rust_typename(&name);
        let type_ref = m.insert_type_stub(&rust_name)?;
        mapping.insert(name, type_ref);
    }

    // we now construct all types properly. When inserting them into
    // the module, stubs are replaced by proper types.
    for (name, schema) in spec.schemata_iter() {
        println!("creating type for schema '{name}");
        match schema {
            types::RefOr::Reference(r) => {
                let (_, target_name) = r
                    .uri()
                    .rsplit_once('/')
                    .expect("URI does not end in type name separated by '/'");
                let alias_name = translate::schema_to_rust_typename(&name);
                let target = mapping.get(target_name).expect("type not found for schema");
                m.insert_type_alias(&alias_name, target.clone());
            }
            types::RefOr::Object(schema) => {
                let type_ref = parse_schema(&schema, Some(name.clone()), cm, m, &mapping)?;
                mapping.insert(name, type_ref);
            }
        }
    }

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
                client_impl,
                &mapping,
                &path,
                &path_item,
                method,
                &path_op,
            )?;
        }
    }
    m.insert_implementation(client_impl.build())?;

    Ok(TypeMapping { mapping })
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

fn parse_schema(
    schema: &impl Schema,
    name: Option<String>,
    cm: &mut Codemodel,
    m: &mut Module,
    mapping: &HashMap<String, TypeRef>,
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

fn parse_path_into_impl_fn(
    cm: &mut Codemodel,
    impl_builder: ImplementationBuilder,
    mapping: &HashMap<String, TypeRef>,
    path_name: &str,
    path_item: &impl PathItem,
    method: http::Method,
    path_op: &impl Operation,
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
        .filter(|param| {
            let shadowed = path_op
                .parameters()
                .any(|p| p.name() == param.name() && p.in_() == param.in_());
            !shadowed
        })
        .collect::<Vec<_>>();
    for param in outer_params {
        function = append_param(function, cm, mapping, &param)?;
    }

    for param in path_op.parameters() {
        function = append_param(function, cm, mapping, &param)?;
    }
    Ok(impl_builder.function(function.build()))
}

/// Append a parameter to the given FunctionBuilder, while respecting the
fn append_param(
    function: FunctionBuilder,
    cm: &mut Codemodel,
    mapping: &HashMap<String, TypeRef>,
    param: &impl Parameter,
) -> anyhow::Result<FunctionBuilder> {
    let existing_names = function.param_names();

    let mapped_name = translate::parameter_to_rust_fn_param(param.name());
    let mapped_name = translate::uncollide(&existing_names, mapped_name);

    let mapped_type;
    if let Some(schema) = param.schema() {
        mapped_type = type_ref_of(cm, mapping, &schema)?;
    } else {
        todo!("implement handling of 'content' field in OAS parameter object");
    }

    // finally add parameter
    Ok(function.param(mapped_name, mapped_type))
}

fn type_ref_of(
    cm: &mut Codemodel,
    mapping: &HashMap<String, TypeRef>,
    schema: &impl Schema,
) -> anyhow::Result<TypeRef> {
    match schema.name() {
        Some(name) => {
            let type_ref = mapping
                .get(name)
                .ok_or_else(|| anyhow!("no mapping found for schema type {name}"))?;
            Ok(type_ref.clone())
        }
        None => match &schema.type_() {
            Some(types) => {
                if types.len() != 1 {
                    return Err(anyhow!(
                        "encountered schema with a type property that has zero or multiple types. It is expected to have exactly one"
                    ));
                } else {
                    match types.get(0).unwrap() {
                        types::Type::Null => todo!(),
                        types::Type::Boolean => Ok(cm.type_bool()),
                        types::Type::Object => {
                            todo!(
                                "inlined schemata are currently unsupported, trying schema {schema:?}"
                            )
                        }
                        types::Type::Array => {
                            let items = schema.items().unwrap();
                            let item_schema = items.get(0).unwrap();
                            let item_type = type_ref_of(cm, mapping, item_schema).unwrap();
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
        assert!(mapping.mapping.contains_key("Pet"));
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
