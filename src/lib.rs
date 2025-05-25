use anyhow::anyhow;
use std::str::FromStr;
use std::{collections::HashMap, io::Read, path::Path};

use codemodel::fqtn::FQTN;
use codemodel::{Codemodel, Module, StructBuilder, TypeRef};
use types::{BooleanOrSchema, Schema, Spec};

mod codemodel;
mod types;

mod adapters;

pub fn generate_from_path<S: Spec>(path: &Path) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(path)?;

    generate_from_reader::<S>(&mut file)
}

pub fn generate_from_str<S: Spec>(s: &str) -> anyhow::Result<()> {
    let spec = S::from_str(s)?;
    generate_code(&spec)?;
    Ok(())
}

pub fn generate_from_reader<S: Spec>(input: impl Read) -> anyhow::Result<()> {
    let spec = S::from_reader(input)?;
    generate_code(&spec)?;
    Ok(())
}

fn generate_code<S: Spec>(spec: &S) -> anyhow::Result<()> {
    let mut cm = Codemodel::new();

    let mut m = Module::new("crate");

    let type_map = populate_types(spec, &mut cm, &mut m)?;

    Ok(())
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
    let mapping = HashMap::new();

    for (name, schema) in spec.schemata_iter() {
        _ = parse_schema(&schema, Some(name), cm, m)?;
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
    m: &mut Module,
) -> anyhow::Result<TypeRef> {
    let kind = type_kind_of(schema)?;

    match &kind {
        TypeKind::Struct => {
            let mut b = StructBuilder::new(name.as_ref().unwrap());
            for (name, schema) in schema.properties() {
                let type_ref = type_ref_of(cm, &schema)?;
                b = b.field(&name, type_ref)?;
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

fn type_ref_of(cm: &mut Codemodel, schema: &impl Schema) -> anyhow::Result<TypeRef> {
    match schema.name() {
        Some(name) => {
            let fqtn = FQTN::from_str(name)?;
            Ok(cm
                .find_type(&fqtn)
                .ok_or(anyhow!("type {name} not found"))?)
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
                        types::Type::Object => todo!(),
                        types::Type::Array => todo!(),
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
    use std::io::Cursor;

    use super::*;

    static PETSTORE_YAML: &[u8] = include_bytes!("test-data/petstore.yaml");
    #[test]
    fn test_oas_petstore() {
        let reader = Cursor::new(PETSTORE_YAML);
        super::generate_from_reader::<adapters::oas30::OAS30Spec>(reader)
            .expect("reading petstore.yaml failed");
    }
}
