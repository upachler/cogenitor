use std::{collections::HashMap, io::Read, path::Path};

use codemodel::{Codemodel, TypeRef};
use json::JsonValue;
use types::{BooleanOrSchema, Schema, Spec};

mod codemodel;
mod types;

mod adapters;

pub fn generate_from_path<S: Spec>(path: &Path) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(path)?;

    generate_from_reader::<S>(&mut file)
}

pub fn generate_from_reader<S: Spec>(input: impl Read) -> anyhow::Result<()> {
    let spec = S::from_reader(input)?;

    let mut cm = Codemodel::new();

    let type_map = populate_types(&spec, &mut cm)?;

    Ok(())
}

/** Maps OpenAPI type names to actual Codemodel [TypeRef]s instances */
struct TypeMapping {
    mapping: HashMap<String, TypeRef>,
}

fn populate_types(spec: &impl Spec, cm: &mut Codemodel) -> anyhow::Result<TypeMapping> {
    let mapping = HashMap::new();

    for (name, schema) in spec.schemata_iter() {
        _ = parse_schema(&schema, Some(name), cm)?;
    }

    Ok(TypeMapping { mapping })
}

/** The rust type we're converting a JSON schema item into */
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
                    if let Some(e) = schema.enum_() {
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
) -> anyhow::Result<TypeRef> {
    let kind = type_kind_of(schema);
    /*
        match kind {
            TypeKind::String => {
                let string_type = cm.type_string(&self);
                if let Some(name) = name {
                    return cm.build_type_alias(name, string_type)?;
                } else {
                    return string_type;
                }
            }
        }
    */
    unimplemented!("parsing {schema:?}")
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    static PETSTORE_YAML: &[u8] = include_bytes!("test-data/petstore.yaml");
    #[test]
    fn it_works() {
        let reader = Cursor::new(PETSTORE_YAML);
        super::generate_from_reader::<adapters::oas30::OAS30Spec>(reader)
            .expect("reading petstore.yaml failed");
    }
}
