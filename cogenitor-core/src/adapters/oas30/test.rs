//! Tests for OAS 3.0 adapter implementation

use std::str::FromStr;

use crate::{
    adapters::oas30::OAS30Spec,
    types::{Format, Parameter, ParameterLocation, Schema, Spec},
};

#[test]
fn test_empty() {
    use crate::types::Spec;

    let oas = r"
openapi: 3.0.0
info:
    title: Empty API
    version: v1
paths:";
    println!("parsing {oas}");
    let spec = OAS30Spec::from_str(oas).unwrap();
    assert!(spec.schemata_iter().next().is_none());
}

#[test]
fn test_number_formats() {
    use crate::types::Spec;

    let oas = r"
openapi: 3.0.0
info:
    title: Number Formats
    version: v1
paths: {}
components:
    schemas:
        NumberFormats:
            type: object
            properties:
                number_unformatted:
                    type: number
                number_double:
                    type: number
                    format: double
                number_float:
                    type: number
                    format: float
                integer_int64:
                    type: integer
                    format: int64
                integer_int32:
                    type: integer
                    format: int32
";
    println!("parsing {oas}");
    let spec = OAS30Spec::from_str(oas).unwrap();
    let nf = spec.schemata_iter().next().unwrap();
    assert_eq!(nf.0, "NumberFormats");
    let schema = nf.1.resolve_fully();
    let nf_props = schema.properties();

    let schema = nf_props.get("number_unformatted").unwrap().resolve_fully();
    assert_eq!(type_of(&schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), None);

    let schema = nf_props.get("number_double").unwrap().resolve_fully();
    assert_eq!(type_of(&schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Double));

    let schema = nf_props.get("number_float").unwrap().resolve_fully();
    assert_eq!(type_of(&schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Float));

    let schema = nf_props.get("integer_int64").unwrap().resolve_fully();
    assert_eq!(type_of(&schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Int64));

    let schema = nf_props.get("integer_int32").unwrap().resolve_fully();
    assert_eq!(type_of(&schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Int32));
}

fn type_of(s: &impl Schema) -> Option<crate::types::Type> {
    if let Some(types) = s.type_() {
        if types.len() != 1 {
            None
        } else {
            Some(types[0].clone())
        }
    } else {
        None
    }
}

#[test]
fn test_simple_paths() {
    use crate::types::{Operation, PathItem, Spec};
    use http::Method;

    let oas = r"
openapi: 3.0.0
info:
    title: Test simple paths
    version: v1
paths:
    '/foo':
        description: get and put a 'foo'
        get:
            responses:
                200:
                    description: 'a simple response'
                    content:
                        application/json:
                            schema:
                                type: string
        put:
            requestBody:
              content:
                application/json:
                  schema:
                    type: string
            responses:
                204:
                    description: 'a simple response'
";
    println!("parsing {oas}");
    let spec = OAS30Spec::from_str(oas).unwrap();

    // Test path_iter() implementation - should return exactly one path
    let paths: Vec<_> = spec.paths().collect();
    assert_eq!(paths.len(), 1);

    // Verify the path name is correctly parsed
    let (path_name, path_item) = &paths[0];
    assert_eq!(path_name, "/foo");

    // Test operations_iter() - should return GET and PUT operations
    let operations: Vec<_> = path_item.operations_iter().collect();
    assert_eq!(operations.len(), 2);

    // Verify GET operation is present and correctly parsed
    let get_op = operations.iter().find(|(method, _)| *method == Method::GET);
    assert!(get_op.is_some(), "GET operation should be present");

    let (_, get_operation) = get_op.unwrap();
    assert_eq!(get_operation.operation_id(), None);

    // Verify PUT operation is present and correctly parsed
    let put_op = operations.iter().find(|(method, _)| *method == Method::PUT);
    assert!(put_op.is_some(), "PUT operation should be present");

    let (_, put_operation) = put_op.unwrap();
    assert_eq!(put_operation.operation_id(), None);

    // Test parameters() at path level - should be empty for this simple case
    let path_params: Vec<_> = path_item.parameters().collect();
    assert_eq!(path_params.len(), 0, "Path should have no parameters");

    // Test parameters() at operation level - should be empty for this simple case
    let get_params: Vec<_> = get_operation.parameters().collect();
    assert_eq!(
        get_params.len(),
        0,
        "GET operation should have no parameters"
    );

    let put_params: Vec<_> = put_operation.parameters().collect();
    assert_eq!(
        put_params.len(),
        0,
        "PUT operation should have no parameters"
    );
}

#[test]
fn test_path_parameters() {
    let oas = r"
openapi: 3.0.0
info:
    title: Test simple paths
    version: v1
paths:
    '/bars/{bar_name}':
        description: access bars
        parameters:
            -   in: path
                name: bar_name
                schema:
                    type: string
                required: true
        get:
            parameters:
                -   name: with_foo
                    in: query
                    schema:
                        type: boolean
            responses:
                200:
                    description: 'a bar'
                    content:
                        application/json:
                            schema:
                                type: string
                404:
                    description: 'bar was not found'
components:
    schemas:
        Bar:
            type: object
            properties:
                name:
                    type: string
                associated_foo:
                    type: string
            required:
                -   name
";
    println!("parsing {oas}");
    let spec = OAS30Spec::from_str(oas).unwrap();
    test_path_parameters_impl(spec)
}

fn test_path_parameters_impl(spec: impl Spec) {
    // Test path_iter() implementation - should return exactly one parameterized path

    use http::Method;

    use crate::types::{Operation, PathItem};
    let paths: Vec<_> = spec.paths().collect();
    assert_eq!(paths.len(), 1);

    // Verify the parameterized path name is correctly parsed (includes {bar_name})
    let (path_name, path_item) = &paths[0];
    assert_eq!(path_name, "/bars/{bar_name}");

    // Test path-level parameters - should have the bar_name path parameter
    // This tests parameter extraction from the path item's parameters array
    let path_params: Vec<_> = path_item.parameters().collect();
    assert_eq!(path_params.len(), 1, "Path should have one parameter");

    // Verify path parameter properties: name and location
    let param = &path_params[0].resolve_fully();
    assert_eq!(param.name(), "bar_name");
    assert_eq!(param.in_(), ParameterLocation::Path);

    // Test operations_iter() - should return GET operation with its own parameters
    let operations: Vec<_> = path_item.operations_iter().collect();
    assert_eq!(operations.len(), 1);

    // Verify GET operation is present and correctly parsed
    let get_op = operations.iter().find(|(method, _)| *method == Method::GET);
    assert!(get_op.is_some(), "GET operation should be present");

    let (_, get_operation) = get_op.unwrap();
    assert_eq!(get_operation.operation_id(), None);

    // Test operation-level parameters - should have the with_foo query parameter
    // This tests parameter extraction from the operation's parameters array
    let get_params: Vec<_> = get_operation.parameters().collect();
    assert_eq!(
        get_params.len(),
        1,
        "GET operation should have one parameter"
    );

    // Verify operation parameter properties: name and location (query vs path)
    let param = &get_params[0].as_object().unwrap();
    assert_eq!(param.name(), "with_foo");
    assert_eq!(param.in_(), ParameterLocation::Query);
}
