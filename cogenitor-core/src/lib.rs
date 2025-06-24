use anyhow::anyhow;
use proc_macro2::TokenStream;
use std::{collections::HashMap, io::Read, path::Path};

use codemodel::{Codemodel, Module, StructBuilder, TypeRef};
use types::{BooleanOrSchema, Schema, Spec};

mod codemodel;
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
        let type_ref = parse_schema(&schema, Some(name.clone()), cm, m, &mapping)?;
        mapping.insert(name, type_ref);
    }

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

    use crate::codemodel::Scope;

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
        super::generate_from_str::<adapters::oas30::OAS30Spec>(oas).unwrap()
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
}
