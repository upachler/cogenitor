use std::{io::Cursor, str::FromStr};
use test_log::test;
use types::Components;

use crate::codemodel::{NamedItem, Scope, implementation::Implementation};

use super::*;

static PETSTORE_YAML: &[u8] = include_bytes!("../../test-data/petstore.yaml");

#[test]
fn test_oas_petstore() {
    let reader = Cursor::new(PETSTORE_YAML);
    super::generate_from_reader(reader).expect("reading petstore.yaml failed");
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
