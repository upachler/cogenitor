use std::hash::Hash;
use std::io::BufReader;
use std::str::FromStr;
use std::{borrow::Borrow, collections::HashMap, rc::Rc};

use http::Method;
use oas3::spec::{ObjectOrReference, ObjectSchema, Spec};

use crate::types::{
    BooleanOrSchema, Components, Operation, Parameter, ParameterLocation, PathItem, RefOr,
    Reference, Schema,
};

pub struct OAS31Spec {
    spec: Rc<Spec>,
}

trait OAS31Resolver<T> {
    fn resolve<'a, S>(&'a self, ro: &'a ObjectOrReference<S>) -> Option<&'a T>
    where
        S: Borrow<T>,
    {
        match ro {
            ObjectOrReference::Ref { ref_path } => {
                let reference = ref_path
                    .strip_prefix("#/components/schemas/")
                    .expect(&format!("Only references to '#/components/schemas/*' are supported, '{ref_path}' does not match"));
                self.resolve_reference(reference)
            }
            ObjectOrReference::Object(s) => Some(s.borrow()),
        }
    }

    fn resolve_reference(&self, reference: &str) -> Option<&T>;
}

impl OAS31Resolver<ObjectSchema> for Spec {
    fn resolve_reference(&self, reference: &str) -> Option<&ObjectSchema> {
        let schema_ref = self.components.as_ref()?.schemas.get(reference)?;
        self.resolve(schema_ref)
    }
}

#[derive(Clone)]
enum RefSource {
    SchemaName(String),
    SchemaProperty((Box<OAS31SchemaRef>, String)),
    AdditionalProperties(Box<OAS31SchemaRef>),
    Items(Box<OAS31SchemaRef>),
}

impl std::fmt::Debug for RefSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefSource::SchemaName(name) => f.write_fmt(format_args!("SchemaName({name})")),
            RefSource::SchemaProperty((_, name)) => {
                f.write_fmt(format_args!("SchemaProperty(_, {name})"))
            }
            RefSource::AdditionalProperties(_) => f.write_str("AdditionalProperties(_)"),
            RefSource::Items(_) => f.write_str("Items(_)"),
        }
    }
}

impl Hash for RefSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            RefSource::SchemaName(name) => {
                0.hash(state);
                name.hash(state);
            }
            RefSource::SchemaProperty((schema_ref, name)) => {
                1.hash(state);
                schema_ref.hash(state);
                name.hash(state);
            }
            RefSource::AdditionalProperties(schema_ref) => {
                2.hash(state);
                schema_ref.hash(state);
            }
            RefSource::Items(schema_ref) => {
                3.hash(state);
                schema_ref.hash(state);
            }
        }
    }
}

impl PartialEq for RefSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RefSource::SchemaName(a), RefSource::SchemaName(b)) => a == b,
            (RefSource::SchemaProperty((a1, a2)), RefSource::SchemaProperty((b1, b2))) => {
                a1 == b1 && a2 == b2
            }
            (RefSource::AdditionalProperties(a), RefSource::AdditionalProperties(b)) => a == b,
            (RefSource::Items(a), RefSource::Items(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for RefSource {}

fn schema_from_additional_properties(schema: &ObjectSchema) -> Option<&oas3::spec::Schema> {
    schema.additional_properties.as_ref()
}

fn schema_from_items(schema: &ObjectSchema) -> Option<&ObjectOrReference<ObjectSchema>> {
    schema.items.as_ref().map(|b| b.as_ref())
}

fn schema_from_property<'a>(
    schema: &'a ObjectSchema,
    property_name: &str,
) -> Option<&'a ObjectOrReference<ObjectSchema>> {
    schema.properties.get(property_name)
}

#[derive(Clone)]
pub struct OAS31SchemaRef {
    spec: Rc<Spec>,
    ref_source: RefSource,
}

impl std::fmt::Debug for OAS31SchemaRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ref_source = &self.ref_source;
        f.write_fmt(format_args!("OAS31SchemaRef[{ref_source:?}]"))?;
        Ok(())
    }
}

impl Hash for OAS31SchemaRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ref_source.hash(state);
    }
}

impl PartialEq for OAS31SchemaRef {
    fn eq(&self, other: &Self) -> bool {
        self.ref_source.eq(&other.ref_source)
    }
}
impl Eq for OAS31SchemaRef {}

impl OAS31SchemaRef {
    fn inner(&self) -> &ObjectSchema {
        match &self.ref_source {
            RefSource::SchemaName(schema_name) => self.spec.resolve_reference(schema_name).unwrap(),
            RefSource::AdditionalProperties(schema_ref) => {
                // For additional properties, we need to handle Schema enum
                match schema_from_additional_properties(schema_ref.inner()).unwrap() {
                    oas3::spec::Schema::Object(obj_ref) => self.spec.resolve(obj_ref).unwrap(),
                    oas3::spec::Schema::Boolean(_) => {
                        // Create a new default schema - we'll need to handle this differently
                        panic!("Boolean additional properties not supported yet")
                    }
                }
            }
            RefSource::Items(schema_ref) => {
                let ro = schema_from_items(schema_ref.inner()).unwrap();
                self.spec.resolve(ro).unwrap()
            }
            RefSource::SchemaProperty((schema_ref, name)) => {
                let ro = schema_from_property(schema_ref.inner(), name).unwrap();
                self.spec.resolve(ro).unwrap()
            }
        }
    }
}

impl From<oas3::spec::SchemaType> for crate::types::Type {
    fn from(value: oas3::spec::SchemaType) -> Self {
        use crate::types::Type;
        match value {
            oas3::spec::SchemaType::Number => Type::Number,
            oas3::spec::SchemaType::Integer => Type::Number,
            oas3::spec::SchemaType::Array => Type::Array,
            oas3::spec::SchemaType::Object => Type::Object,
            oas3::spec::SchemaType::String => Type::String,
            oas3::spec::SchemaType::Boolean => Type::Boolean,
            oas3::spec::SchemaType::Null => Type::Null,
        }
    }
}

fn schema_name_of_reference_or(reference_or: &ObjectOrReference<ObjectSchema>) -> Option<&str> {
    let prefix = "#/components/schemas/";
    match reference_or {
        ObjectOrReference::Ref { ref_path } => {
            let schema_name = ref_path.strip_prefix(prefix).expect(&format!("reference to schema '{ref_path}' does not start with OAS standard schema prefix {prefix}"));
            Some(schema_name)
        }
        ObjectOrReference::Object(_) => None,
    }
}

impl Schema for OAS31SchemaRef {
    fn name(&self) -> Option<&str> {
        match &self.ref_source {
            RefSource::SchemaName(name) => Some(name),
            RefSource::SchemaProperty((ref_source, name)) => {
                // the name of a schema referenced via a property of
                // another schema is either the name in the reference
                // (e.g. '#/components/schemas/MySchemaName') or
                // None for cases where the schema is inlined
                schema_name_of_reference_or(ref_source.inner().properties.get(name)?)
            }
            RefSource::Items(schema_ref) => {
                if let Some(items_ref) = &schema_ref.inner().items {
                    schema_name_of_reference_or(items_ref)
                } else {
                    None
                }
            }
            RefSource::AdditionalProperties(schema_ref) => {
                if let Some(additional_properties) = &schema_ref.inner().additional_properties {
                    match additional_properties {
                        oas3::spec::Schema::Object(obj_ref) => schema_name_of_reference_or(obj_ref),
                        oas3::spec::Schema::Boolean(_) => None,
                    }
                } else {
                    None
                }
            }
        }
    }

    fn type_(&self) -> Option<Vec<crate::types::Type>> {
        if let Some(schema_type_set) = &self.inner().schema_type {
            let types: Vec<crate::types::Type> = match schema_type_set {
                oas3::spec::SchemaTypeSet::Single(t) => vec![(*t).into()],
                oas3::spec::SchemaTypeSet::Multiple(types) => {
                    types.iter().map(|t| (*t).into()).collect()
                }
            };
            Some(types)
        } else {
            None
        }
    }

    fn format(&self) -> Option<crate::types::Format> {
        let schema = self.inner();
        if let Some(format) = &schema.format {
            match format.as_str() {
                "int32" => Some(crate::types::Format::Int32),
                "int64" => Some(crate::types::Format::Int64),
                "float" => Some(crate::types::Format::Float),
                "double" => Some(crate::types::Format::Double),
                "byte" => Some(crate::types::Format::Byte),
                "binary" => Some(crate::types::Format::Binary),
                "date" => Some(crate::types::Format::Date),
                "date-time" => Some(crate::types::Format::DateTime),
                "password" => Some(crate::types::Format::Password),
                _ => None,
            }
        } else {
            None
        }
    }

    fn title(&self) -> Option<&str> {
        self.inner().title.as_deref()
    }

    fn description(&self) -> Option<&str> {
        self.inner().description.as_deref()
    }

    fn required(&self) -> Option<Vec<&str>> {
        let required = &self.inner().required;
        if required.is_empty() {
            None
        } else {
            Some(required.iter().map(|s| s.as_str()).collect())
        }
    }

    fn all_of(&self) -> Option<Vec<impl Schema>> {
        let all_of = &self.inner().all_of;
        if all_of.is_empty() {
            None
        } else {
            let schemas: Vec<OAS31SchemaRef> = all_of
                .iter()
                .enumerate()
                .map(|(i, _)| OAS31SchemaRef {
                    spec: self.spec.clone(),
                    ref_source: RefSource::SchemaName(format!("allOf_{}", i)), // This is a simplification
                })
                .collect();
            Some(schemas)
        }
    }

    fn any_of(&self) -> Option<Vec<impl Schema>> {
        let any_of = &self.inner().any_of;
        if any_of.is_empty() {
            None
        } else {
            let schemas: Vec<OAS31SchemaRef> = any_of
                .iter()
                .enumerate()
                .map(|(i, _)| OAS31SchemaRef {
                    spec: self.spec.clone(),
                    ref_source: RefSource::SchemaName(format!("anyOf_{}", i)), // This is a simplification
                })
                .collect();
            Some(schemas)
        }
    }

    fn one_of(&self) -> Option<Vec<impl Schema>> {
        let one_of = &self.inner().one_of;
        if one_of.is_empty() {
            None
        } else {
            let schemas: Vec<OAS31SchemaRef> = one_of
                .iter()
                .enumerate()
                .map(|(i, _)| OAS31SchemaRef {
                    spec: self.spec.clone(),
                    ref_source: RefSource::SchemaName(format!("oneOf_{}", i)), // This is a simplification
                })
                .collect();
            Some(schemas)
        }
    }

    fn enum_(&self) -> Option<Vec<json::JsonValue>> {
        let enum_values = &self.inner().enum_values;
        if enum_values.is_empty() {
            None
        } else {
            let json_values: Vec<json::JsonValue> = enum_values
                .iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(json::JsonValue::String(s.clone())),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Some(json::JsonValue::Number(json::number::Number::from(i)))
                        } else if let Some(f) = n.as_f64() {
                            Some(json::JsonValue::Number(json::number::Number::from(f)))
                        } else {
                            None
                        }
                    }
                    serde_json::Value::Bool(b) => Some(json::JsonValue::Boolean(*b)),
                    serde_json::Value::Null => Some(json::JsonValue::Null),
                    _ => None,
                })
                .collect();
            Some(json_values)
        }
    }

    fn properties(&self) -> std::collections::HashMap<String, impl Schema> {
        let mut m = HashMap::new();
        let properties = &self.inner().properties;
        for (k, _v) in properties.iter() {
            let ref_source = RefSource::SchemaProperty((Box::new(self.clone()), k.clone()));
            let type_ = OAS31SchemaRef {
                spec: self.spec.clone(),
                ref_source,
            };
            m.insert(k.to_string(), type_);
        }
        m
    }

    fn pattern_properties(&self) -> std::collections::HashMap<String, impl Schema> {
        HashMap::<_, OAS31SchemaRef>::new() // TODO: Implement pattern properties support
    }

    fn addtional_properties(&self) -> crate::types::BooleanOrSchema<impl Schema> {
        let inner = self.inner();
        if let Some(additional_properties) = &inner.additional_properties {
            match additional_properties {
                oas3::spec::Schema::Boolean(b_schema) => BooleanOrSchema::Boolean(b_schema.0),
                oas3::spec::Schema::Object(_) => BooleanOrSchema::<Self>::Schema(Self {
                    spec: self.spec.clone(),
                    ref_source: RefSource::AdditionalProperties(Box::new(self.clone())),
                }),
            }
        } else {
            BooleanOrSchema::<Self>::Boolean(true)
        }
    }

    fn items(&self) -> Option<Vec<impl Schema>> {
        if let Some(_items) = &self.inner().items {
            let ref_source = RefSource::Items(Box::new(self.clone()));
            Some(vec![OAS31SchemaRef {
                spec: self.spec.clone(),
                ref_source,
            }])
        } else {
            None
        }
    }
}

impl FromStr for OAS31Spec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        let spec: Spec = oas3::from_str(s)?;
        Ok(spec.into())
    }
}

impl From<Spec> for OAS31Spec {
    fn from(spec: Spec) -> Self {
        OAS31Spec {
            spec: Rc::new(spec),
        }
    }
}

struct OAS31SchemaReference {
    spec: Rc<Spec>,
    uri: String,
}

impl Reference for OAS31SchemaReference {
    type Target = OAS31SchemaRef;

    fn resolve(&self) -> Self::Target {
        OAS31SchemaRef {
            spec: self.spec.clone(),
            ref_source: RefSource::SchemaName(self.uri.clone()),
        }
    }

    fn uri(&self) -> &str {
        &self.uri
    }
}

impl crate::Spec for OAS31Spec {
    type Schema = OAS31SchemaRef;

    fn from_reader(r: impl std::io::Read) -> anyhow::Result<impl crate::Spec> {
        let r = BufReader::new(r);
        let spec: Spec = oas3::from_reader(r)?;
        Ok(OAS31Spec::from(spec))
    }

    fn schemata_iter(
        &self,
    ) -> impl Iterator<Item = (String, RefOr<impl Reference<Target = impl Schema>>)> {
        SchemaIterator {
            spec: self.spec.clone(),
            curr: 0,
            end: self
                .spec
                .components
                .as_ref()
                .map(|c| c.schemas.len())
                .unwrap_or(0),
        }
    }

    fn paths(&self) -> impl Iterator<Item = (String, impl PathItem)> {
        let paths: Vec<(String, oas3::spec::PathItem)> = self
            .spec
            .paths
            .as_ref()
            .map(|paths| {
                paths
                    .iter()
                    .map(|(path, path_item)| (path.clone(), path_item.clone()))
                    .collect()
            })
            .unwrap_or_default();

        PathIterator { paths, current: 0 }
    }

    fn components(&self) -> Option<impl Components> {
        self.spec.components.as_ref().map(|_| self)
    }
}

impl Components for &OAS31Spec {
    fn schemas(
        &self,
    ) -> impl Iterator<Item = (String, RefOr<impl Reference<Target = impl Schema>>)> {
        crate::Spec::schemata_iter(*self)
    }
}

struct SchemaIterator {
    spec: Rc<Spec>,
    curr: usize,
    end: usize,
}

impl Iterator for SchemaIterator {
    type Item = (String, RefOr<OAS31SchemaReference>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == self.end {
            return None;
        }

        let schemas = &self.spec.components.as_ref()?.schemas;
        let (schema_name, schema_or_ref) = schemas.iter().nth(self.curr)?;
        let spec = self.spec.clone();

        let ref_or = match schema_or_ref {
            ObjectOrReference::Ref { ref_path } => RefOr::Reference(OAS31SchemaReference {
                spec,
                uri: ref_path.clone(),
            }),
            ObjectOrReference::Object(_) => RefOr::Object(OAS31SchemaRef {
                spec,
                ref_source: RefSource::SchemaName(schema_name.clone()),
            }),
        };

        let r = (schema_name.clone(), ref_or);
        self.curr = self.curr + 1;
        Some(r)
    }
}

// Path Iterator Implementation
struct PathIterator {
    paths: Vec<(String, oas3::spec::PathItem)>,
    current: usize,
}

impl Iterator for PathIterator {
    type Item = (String, OAS31PathItem);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((path, path_item)) = self.paths.get(self.current) {
            self.current += 1;
            return Some((
                path.clone(),
                OAS31PathItem {
                    path_item: path_item.clone(),
                },
            ));
        }
        None
    }
}

// OAS31 PathItem Implementation
pub struct OAS31PathItem {
    path_item: oas3::spec::PathItem,
}

fn extract_operation(
    operations: &mut Vec<(Method, OAS31Operation)>,
    method: http::Method,
    opt_op: &Option<oas3::spec::Operation>,
) {
    if let Some(op) = opt_op {
        operations.push((
            method,
            OAS31Operation {
                operation: op.clone(),
            },
        ));
    }
}

fn extract_parameter(
    parameters: &mut Vec<OAS31Parameter>,
    location: ParameterLocation,
    param: &oas3::spec::Parameter,
) {
    let param_name = param.name.clone();
    parameters.push(OAS31Parameter {
        param_name,
        location,
    });
}

fn to_parameters_iter(
    oas31_parameters: &Vec<ObjectOrReference<oas3::spec::Parameter>>,
) -> impl Iterator<Item = impl Parameter> {
    let mut params = Vec::new();
    for param_ref in oas31_parameters {
        if let ObjectOrReference::Object(param) = param_ref {
            let location = match param.location {
                oas3::spec::ParameterIn::Query => ParameterLocation::Query,
                oas3::spec::ParameterIn::Header => ParameterLocation::Header,
                oas3::spec::ParameterIn::Path => ParameterLocation::Path,
                oas3::spec::ParameterIn::Cookie => ParameterLocation::Cookie,
            };
            extract_parameter(&mut params, location, param);
        }
    }
    params.into_iter()
}

impl PathItem for OAS31PathItem {
    fn operations_iter(&self) -> impl Iterator<Item = (Method, impl Operation)> {
        let mut operations = Vec::new();

        extract_operation(&mut operations, Method::GET, &self.path_item.get);
        extract_operation(&mut operations, Method::PUT, &self.path_item.put);
        extract_operation(&mut operations, Method::POST, &self.path_item.post);
        extract_operation(&mut operations, Method::DELETE, &self.path_item.delete);
        extract_operation(&mut operations, Method::OPTIONS, &self.path_item.options);
        extract_operation(&mut operations, Method::HEAD, &self.path_item.head);
        extract_operation(&mut operations, Method::PATCH, &self.path_item.patch);
        extract_operation(&mut operations, Method::TRACE, &self.path_item.trace);

        operations.into_iter()
    }

    fn parameters(&self) -> impl Iterator<Item = impl Parameter> {
        to_parameters_iter(&self.path_item.parameters)
    }
}

// OAS31 Operation Implementation
pub struct OAS31Operation {
    operation: oas3::spec::Operation,
}

impl Operation for OAS31Operation {
    fn parameters(&self) -> impl Iterator<Item = impl Parameter> {
        to_parameters_iter(&self.operation.parameters)
    }

    fn operation_id(&self) -> Option<&str> {
        self.operation.operation_id.as_deref()
    }
}

// OAS31 Parameter Implementation
pub struct OAS31Parameter {
    param_name: String,
    location: ParameterLocation,
}

impl Parameter for OAS31Parameter {
    fn in_(&self) -> ParameterLocation {
        self.location
    }

    fn name(&self) -> &str {
        &self.param_name
    }

    fn schema(&self) -> Option<impl Schema> {
        todo!();
        Option::<OAS31SchemaRef>::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Spec;

    #[test]
    fn test_empty() {
        let oas = r"
openapi: 3.1.0
info:
    title: Empty API
    version: v1
paths: {}";
        println!("parsing {oas}");
        let spec = OAS31Spec::from_str(oas).unwrap();
        assert!(spec.schemata_iter().next().is_none());
    }

    #[test]
    fn test_number_formats() {
        let oas = r"
openapi: 3.1.0
info:
    title: Number Formats
    version: v1
paths: {}
components:
    schemas:
        NumberFormats:
            type: object
            properties:
                int32_field:
                    type: integer
                    format: int32
                int64_field:
                    type: integer
                    format: int64
                float_field:
                    type: number
                    format: float
                double_field:
                    type: number
                    format: double";

        let spec = OAS31Spec::from_str(oas).unwrap();
        let mut schemas: Vec<_> = spec.schemata_iter().collect();
        assert_eq!(schemas.len(), 1);

        let (name, schema) = schemas.pop().unwrap();
        assert_eq!(name, "NumberFormats");

        let resolved_schema = schema.resolve();
        let properties = resolved_schema.properties();
        assert_eq!(properties.len(), 4);

        let int32_field = properties.get("int32_field").unwrap();
        assert_eq!(int32_field.format(), Some(crate::types::Format::Int32));
    }

    #[test]
    fn test_simple_paths() {
        let oas = r"
openapi: 3.1.0
info:
    title: Simple Paths
    version: v1
paths:
    /users:
        get:
            operationId: getUsers
        post:
            operationId: createUser
    /users/{id}:
        parameters:
            - name: id
              in: path
              required: true
              schema:
                  type: string
        get:
            operationId: getUser";

        let spec = OAS31Spec::from_str(oas).unwrap();
        let paths: Vec<_> = spec.paths().collect();
        assert_eq!(paths.len(), 2);

        // Check first path
        let (path, path_item) = &paths[0];
        assert_eq!(path, "/users");
        let operations: Vec<_> = path_item.operations_iter().collect();
        assert_eq!(operations.len(), 2);

        // Check second path
        let (path, path_item) = &paths[1];
        assert_eq!(path, "/users/{id}");
        let operations: Vec<_> = path_item.operations_iter().collect();
        assert_eq!(operations.len(), 1);

        let parameters: Vec<_> = path_item.parameters().collect();
        assert_eq!(parameters.len(), 1);

        let param = &parameters[0];
        assert_eq!(param.name(), "id");
        assert_eq!(param.in_(), ParameterLocation::Path);
    }

    #[test]
    fn test_comprehensive_oas31_spec() {
        let oas = r"
openapi: 3.1.0
info:
    title: Comprehensive OAS 3.1 Test
    version: 1.0.0
    description: A comprehensive test of OpenAPI 3.1 features
paths:
    /pets:
        get:
            summary: List all pets
            operationId: listPets
            parameters:
                - name: limit
                  in: query
                  description: How many items to return at one time (max 100)
                  required: false
                  schema:
                      type: integer
                      format: int32
        post:
            summary: Create a pet
            operationId: createPets
    /pets/{petId}:
        parameters:
            - name: petId
              in: path
              required: true
              description: The id of the pet to retrieve
              schema:
                  type: string
        get:
            summary: Info for a specific pet
            operationId: showPetById
components:
    schemas:
        Pet:
            type: object
            required:
                - id
                - name
            properties:
                id:
                    type: integer
                    format: int64
                name:
                    type: string
                    examples:
                        - Fluffy
                        - Buddy
                tag:
                    type: string
                    description: Pet category
                status:
                    type: string
                    enum:
                        - available
                        - pending
                        - sold
                    default: available
                metadata:
                    type: object
                    additionalProperties:
                        type: string
                    description: Additional metadata
        Error:
            type: object
            properties:
                code:
                    type: integer
                    format: int32
                message:
                    type: string
            required:
                - code
                - message
        PetList:
            type: array
            items:
                $ref: '#/components/schemas/Pet'
            description: A list of pets";

        let spec = OAS31Spec::from_str(oas).unwrap();
        test_comprehensive_spec_impl(spec);
    }

    #[cfg(test)]
    pub fn test_comprehensive_spec_impl(spec: impl crate::types::Spec) {
        // Test schema iteration
        let schemas: Vec<_> = spec.schemata_iter().collect();
        assert_eq!(schemas.len(), 3);

        let schema_names: Vec<&str> = schemas.iter().map(|(name, _)| name.as_str()).collect();
        assert!(schema_names.contains(&"Pet"));
        assert!(schema_names.contains(&"Error"));
        assert!(schema_names.contains(&"PetList"));

        // Test Pet schema details
        let pet_schema = schemas
            .iter()
            .find(|(name, _)| name == "Pet")
            .unwrap()
            .1
            .resolve();
        assert_eq!(pet_schema.name(), Some("Pet"));

        let properties = pet_schema.properties();
        assert_eq!(properties.len(), 5); // id, name, tag, status, metadata
        assert!(properties.contains_key("id"));
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("tag"));
        assert!(properties.contains_key("status"));
        assert!(properties.contains_key("metadata"));

        // Test required fields
        let required = pet_schema.required().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"id"));
        assert!(required.contains(&"name"));

        // Test id field format
        let id_field = properties.get("id").unwrap();
        assert_eq!(id_field.format(), Some(crate::types::Format::Int64));

        // Test status field enum
        let status_field = properties.get("status").unwrap();
        let enum_values = status_field.enum_();
        assert!(enum_values.is_some());
        let enum_vals = enum_values.unwrap();
        assert_eq!(enum_vals.len(), 3);

        // Test metadata field additional properties
        let metadata_field = properties.get("metadata").unwrap();
        match metadata_field.addtional_properties() {
            crate::types::BooleanOrSchema::Schema(_) => {} // Expected
            crate::types::BooleanOrSchema::Boolean(_) => {
                panic!("Expected schema for additional properties")
            }
        }

        // Test PetList array items
        let pet_list_schema = schemas
            .iter()
            .find(|(name, _)| name == "PetList")
            .unwrap()
            .1
            .resolve();
        let items = pet_list_schema.items();
        assert!(items.is_some());
        assert_eq!(items.unwrap().len(), 1);

        // Test path iteration
        let paths: Vec<_> = spec.paths().collect();
        assert_eq!(paths.len(), 2);

        // Check /pets path
        let pets_path = paths.iter().find(|(path, _)| path == "/pets").unwrap();
        let operations: Vec<_> = pets_path.1.operations_iter().collect();
        assert_eq!(operations.len(), 2); // GET and POST

        // Check GET operation parameters
        let get_op = operations
            .iter()
            .find(|(method, _)| *method == http::Method::GET)
            .unwrap();
        let params: Vec<_> = get_op.1.parameters().collect();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name(), "limit");
        assert_eq!(params[0].in_(), ParameterLocation::Query);

        // Check /pets/{petId} path parameters
        let pet_id_path = paths
            .iter()
            .find(|(path, _)| path == "/pets/{petId}")
            .unwrap();
        let path_params: Vec<_> = pet_id_path.1.parameters().collect();
        assert_eq!(path_params.len(), 1);
        assert_eq!(path_params[0].name(), "petId");
        assert_eq!(path_params[0].in_(), ParameterLocation::Path);
    }
}
