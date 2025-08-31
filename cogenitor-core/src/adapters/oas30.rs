use std::hash::Hash;
use std::io::BufReader;
use std::str::FromStr;
use std::{borrow::Borrow, collections::HashMap, rc::Rc};

use http::Method;
use openapiv3::{OpenAPI, ReferenceOr, Type};

use crate::translate::schema_to_rust_typename;
#[cfg(test)]
use crate::types::Format;
use crate::types::{
    BooleanOrSchema, Components, Operation, Parameter, ParameterLocation, PathItem, RefOr,
    Reference, Schema, Spec,
};

pub struct OAS30Spec {
    openapi: Rc<OpenAPI>,
}

trait OAS3Resolver<T> {
    fn resolve<'a, S>(&'a self, ro: &'a ReferenceOr<S>) -> Option<&'a T>
    where
        S: Borrow<T>,
    {
        match ro {
            ReferenceOr::Reference { reference } => {
                let prefix = self.prefix();
                let reference = reference.strip_prefix(prefix).expect(&format!(
                    "Only references to '{prefix}*' are supported, '{reference}' does not match"
                ));
                Some(self.resolve_reference(reference).expect(
                    format!("expected reference {reference} not found in OpenAPI object").as_ref(),
                ))
            }
            ReferenceOr::Item(s) => Some(s.borrow()),
        }
    }

    fn prefix(&self) -> &str;
    fn resolve_reference(&self, reference: &str) -> Option<&T>;
}

impl OAS3Resolver<openapiv3::Schema> for openapiv3::OpenAPI {
    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::Schema> {
        let ro = self.components.as_ref()?.schemas.get(reference)?;
        self.resolve(ro)
    }
    fn prefix(&self) -> &str {
        "#/components/schemas/"
    }
}

impl OAS3Resolver<openapiv3::PathItem> for openapiv3::OpenAPI {
    fn prefix(&self) -> &str {
        "#/paths/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::PathItem> {
        let ro = self.paths.paths.get(reference)?;
        self.resolve(ro)
    }
}

#[derive(Clone)]
enum SchemaSource {
    Uri(String),
    SchemaName(String),
    SchemaProperty((Box<OAS30SchemaPointer>, String)),
    AdditionalProperties(Box<OAS30SchemaPointer>),
    Items(Box<OAS30SchemaPointer>),
    OperationParam(openapiv3::Schema),
}

impl std::fmt::Debug for SchemaSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaSource::Uri(uri) => f.write_fmt(format_args!("'{uri}'")),
            SchemaSource::SchemaName(name) => f.write_fmt(format_args!("'{name}'")),
            SchemaSource::AdditionalProperties(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.additionalProperties"))
            }
            SchemaSource::SchemaProperty((oas30_schema_ref, name)) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.properties.{name}"))
            }
            SchemaSource::Items(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.items"))
            }
            SchemaSource::OperationParam(_) => f.write_str("InlineSchema"),
        }
    }
}

impl Hash for SchemaSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            SchemaSource::Uri(uri) => uri.hash(state),
            SchemaSource::SchemaName(n) => n.hash(state),
            SchemaSource::SchemaProperty(p) => {
                state.write("p".as_bytes());
                p.0.hash(state);
                p.1.hash(state);
            }
            SchemaSource::AdditionalProperties(r) => {
                state.write("a".as_bytes());
                r.hash(state)
            }
            SchemaSource::Items(r) => {
                state.write("i".as_bytes());
                r.hash(state);
            }
            SchemaSource::OperationParam(_) => {
                state.write("inline".as_bytes());
                // Note: We can't hash the schema content easily, so we just use a constant
                // This means inline schemas will hash to the same value, which is not ideal
                // but should work for basic functionality
            }
        }
    }
}

impl PartialEq for SchemaSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SchemaSource::SchemaName(s), SchemaSource::SchemaName(o)) => s.eq(o),
            (SchemaSource::SchemaProperty(s), SchemaSource::SchemaProperty(o)) => s.eq(o),
            (SchemaSource::AdditionalProperties(s), SchemaSource::AdditionalProperties(o)) => {
                s.eq(o)
            }
            (SchemaSource::Items(s), SchemaSource::Items(o)) => s.eq(o),
            (SchemaSource::OperationParam(_), SchemaSource::OperationParam(_)) => {
                todo!("this is broken, needs to compare path properly");
                // For simplicity, we consider all inline schemas as different
                // A proper implementation would compare schema content
                false
            }
            _ => false,
        }
    }
}
impl Eq for SchemaSource {}

fn schema_from_additional_properties(
    oas_schema: &openapiv3::Schema,
) -> Option<&ReferenceOr<openapiv3::Schema>> {
    use openapiv3::*;
    match &oas_schema.schema_kind {
        SchemaKind::Type(Type::Object(o)) => match &o.additional_properties {
            Some(AdditionalProperties::Schema(o)) => Some(o.as_ref()),
            _ => None,
        },
        _ => None,
    }
}
fn schema_from_items(
    oas_schema: &openapiv3::Schema,
) -> Option<&ReferenceOr<Box<openapiv3::Schema>>> {
    use openapiv3::*;
    match &oas_schema.schema_kind {
        SchemaKind::Type(Type::Array(a)) => a.items.as_ref(),
        _ => None,
    }
}

fn schema_from_property<'a, 'b>(
    oas_schema: &'a openapiv3::Schema,
    name: &str,
) -> Option<&'a ReferenceOr<Box<openapiv3::Schema>>> {
    use openapiv3::*;
    match &oas_schema.schema_kind {
        SchemaKind::Type(Type::Object(o)) => o.properties.get(name),
        _ => None,
    }
}

#[derive(Clone)]
pub struct OAS30SchemaPointer {
    openapi: Rc<OpenAPI>,
    ref_source: SchemaSource,
}

impl std::fmt::Debug for OAS30SchemaPointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ref_source = &self.ref_source;
        f.write_fmt(format_args!("OAS30SchemaRef[{ref_source:?}]"))?;
        Ok(())
    }
}

impl Hash for OAS30SchemaPointer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ref_source.hash(state);
    }
}

impl PartialEq for OAS30SchemaPointer {
    fn eq(&self, other: &Self) -> bool {
        self.ref_source.eq(&other.ref_source)
    }
}
impl Eq for OAS30SchemaPointer {}

impl OAS30SchemaPointer {
    fn inner(&self) -> &openapiv3::Schema {
        match &self.ref_source {
            SchemaSource::Uri(uri) => {
                let schema_name = uri
                    .strip_prefix(OAS3Resolver::<openapiv3::Schema>::prefix(
                        self.openapi.as_ref(),
                    ))
                    .unwrap();
                self.openapi.resolve_reference(schema_name).unwrap()
            }
            SchemaSource::SchemaName(schema_name) => {
                self.openapi.resolve_reference(schema_name).unwrap()
            }
            SchemaSource::AdditionalProperties(schema_ref) => {
                let ro = schema_from_additional_properties(schema_ref.inner()).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
            SchemaSource::Items(schema_ref) => {
                let ro = schema_from_items(schema_ref.inner()).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
            SchemaSource::SchemaProperty((schema_ref, name)) => {
                let ro = schema_from_property(schema_ref.inner(), name).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
            SchemaSource::OperationParam(schema) => schema,
        }
    }
}

struct OAS30SchemaReference {
    openapi: Rc<OpenAPI>,
    uri: String,
}

impl Reference for OAS30SchemaReference {
    type Target = OAS30SchemaPointer;

    fn resolve(&self) -> Self::Target {
        OAS30SchemaPointer {
            openapi: self.openapi.clone(),
            ref_source: SchemaSource::Uri(self.uri.clone()),
        }
    }

    fn uri(&self) -> &str {
        &self.uri
    }
}

impl From<&openapiv3::Type> for crate::types::Type {
    fn from(value: &openapiv3::Type) -> Self {
        use crate::types::Type;
        match value {
            openapiv3::Type::Number(_) | openapiv3::Type::Integer(_) => Type::Number,
            openapiv3::Type::Array(_) => Type::Array,
            openapiv3::Type::Object(_) => Type::Object,
            openapiv3::Type::String(_) => Type::String,
            openapiv3::Type::Boolean(_) => Type::Boolean,
        }
    }
}

fn schema_name_of_reference_or(
    reference_or: &ReferenceOr<impl Borrow<openapiv3::Schema>>,
) -> Option<&str> {
    let prefix = "#/components/schemas/";
    match reference_or {
        ReferenceOr::Reference { reference } => {
            let schema_name = reference.strip_prefix(prefix).expect("reference to schema '{reference}' does not start with OAS standard schema prefix {prefix}");
            Some(schema_name)
        }
        ReferenceOr::Item(_) => None,
    }
}

impl Schema for OAS30SchemaPointer {
    fn name(&self) -> Option<&str> {
        match &self.ref_source {
            SchemaSource::Uri(uri) => uri.rsplit('/').last(),
            SchemaSource::SchemaName(name) => Some(name),
            SchemaSource::SchemaProperty((ref_source, name)) => {
                // the name of a schema referenced via a property of
                // onother schema is either tne name in the reference
                // (e.g. '#/components/schemas/MySchemaName') or
                // None for cases where the schema is inlined
                if let openapiv3::SchemaKind::Type(Type::Object(o)) =
                    &ref_source.inner().schema_kind
                {
                    schema_name_of_reference_or(o.properties.get(name)?)
                } else {
                    None
                }
            }
            SchemaSource::Items(schema_ref) => {
                if let openapiv3::SchemaKind::Type(Type::Array(a)) = &schema_ref.inner().schema_kind
                {
                    schema_name_of_reference_or(a.items.as_ref()?)
                } else {
                    None
                }
            }
            SchemaSource::AdditionalProperties(schema_ref) => {
                if let openapiv3::SchemaKind::Type(Type::Object(o)) =
                    &schema_ref.inner().schema_kind
                {
                    match o.additional_properties.as_ref()? {
                        openapiv3::AdditionalProperties::Any(_) => None,
                        openapiv3::AdditionalProperties::Schema(reference_or) => {
                            let reference_or = reference_or.as_ref();
                            Some(schema_name_of_reference_or(&reference_or)?)
                        }
                    }
                } else {
                    None
                }
            }
            SchemaSource::OperationParam(_) => None,
        }
    }

    fn type_(&self) -> Option<Vec<crate::types::Type>> {
        match &(self.inner().schema_kind) {
            openapiv3::SchemaKind::Type(t) => Some(vec![t.into()]),
            _ => unimplemented!(),
        }
    }

    fn format(&self) -> Option<crate::types::Format> {
        use openapiv3::*;
        match &self.inner().schema_kind {
            SchemaKind::Type(Type::Number(number_type)) => match number_type.format {
                VariantOrUnknownOrEmpty::Item(number_format) => {
                    let fmt = match number_format {
                        NumberFormat::Float => crate::types::Format::Float,
                        NumberFormat::Double => crate::types::Format::Double,
                    };
                    Some(fmt)
                }
                _ => None,
            },
            SchemaKind::Type(Type::Integer(integer_type)) => match integer_type.format {
                VariantOrUnknownOrEmpty::Item(integer_format) => {
                    let fmt = match integer_format {
                        IntegerFormat::Int32 => crate::types::Format::Int32,
                        IntegerFormat::Int64 => crate::types::Format::Int64,
                    };
                    Some(fmt)
                }
                _ => None,
            },
            SchemaKind::Type(Type::String(string_type)) => match string_type.format {
                VariantOrUnknownOrEmpty::Item(string_format) => {
                    let fmt = match string_format {
                        StringFormat::Date => crate::types::Format::Date,
                        StringFormat::DateTime => crate::types::Format::DateTime,
                        StringFormat::Password => crate::types::Format::Password,
                        StringFormat::Byte => crate::types::Format::Byte,
                        StringFormat::Binary => crate::types::Format::Binary,
                    };
                    Some(fmt)
                }
                VariantOrUnknownOrEmpty::Unknown(_) => todo!(),
                VariantOrUnknownOrEmpty::Empty => todo!(),
            },
            _ => None,
        }
    }

    fn title(&self) -> Option<&str> {
        todo!()
    }

    fn description(&self) -> Option<&str> {
        todo!()
    }

    fn required(&self) -> Option<Vec<&str>> {
        todo!()
    }

    fn all_of(&self) -> Option<Vec<impl Schema>> {
        Option::<Vec<OAS30SchemaPointer>>::None
    }

    fn any_of(&self) -> Option<Vec<impl Schema>> {
        Option::<Vec<OAS30SchemaPointer>>::None
    }

    fn one_of(&self) -> Option<Vec<impl Schema>> {
        Option::<Vec<OAS30SchemaPointer>>::None
    }

    fn enum_(&self) -> Option<Vec<json::JsonValue>> {
        todo!()
    }

    fn properties(&self) -> std::collections::HashMap<String, impl Schema> {
        use openapiv3::*;
        let mut m = HashMap::new();
        match &self.inner().schema_kind {
            SchemaKind::Type(Type::Object(t)) => {
                for (k, _v) in t.properties.iter() {
                    let ref_source =
                        SchemaSource::SchemaProperty((Box::new(self.clone()), k.clone()));
                    let type_ = OAS30SchemaPointer {
                        openapi: self.openapi.clone(),
                        ref_source,
                    };
                    m.insert(k.to_string(), type_);
                }
            }
            _ => (),
        };
        m
    }

    fn pattern_properties(&self) -> std::collections::HashMap<String, impl Schema> {
        HashMap::<_, OAS30SchemaPointer>::new()
    }

    fn addtional_properties(&self) -> crate::types::BooleanOrSchema<impl Schema> {
        use openapiv3::*;
        let inner = self.inner();
        match &inner.schema_kind {
            SchemaKind::Type(Type::Object(ObjectType {
                additional_properties: Some(AdditionalProperties::Any(any)),
                ..
            })) => BooleanOrSchema::Boolean(*any),
            SchemaKind::Type(Type::Object(_)) => {
                if schema_from_additional_properties(inner).is_some() {
                    BooleanOrSchema::<Self>::Schema(Self {
                        openapi: self.openapi.clone(),
                        ref_source: SchemaSource::AdditionalProperties(Box::new(self.clone())),
                    })
                } else {
                    BooleanOrSchema::<Self>::Boolean(true)
                }
            }
            _ => BooleanOrSchema::<Self>::Boolean(true),
        }
    }

    fn items(&self) -> Option<Vec<impl Schema>> {
        match &self.inner().schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Array(_)) => {
                let ref_source = SchemaSource::Items(Box::new(self.clone()));
                Some(vec![OAS30SchemaPointer {
                    openapi: self.openapi.clone(),
                    ref_source,
                }])
            }
            _ => None,
        }
    }
}

impl FromStr for OAS30Spec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        let openapi: OpenAPI = serde_yaml::from_str(s)?;
        Ok(openapi.into())
    }
}

impl From<OpenAPI> for OAS30Spec {
    fn from(openapi: OpenAPI) -> Self {
        OAS30Spec {
            openapi: Rc::new(openapi),
        }
    }
}

impl crate::Spec for OAS30Spec {
    type Schema = OAS30SchemaPointer;

    fn from_reader(r: impl std::io::Read) -> anyhow::Result<impl crate::Spec> {
        let r = BufReader::new(r);
        let openapi: OpenAPI = serde_yaml::from_reader(r)?;
        Ok(OAS30Spec::from(openapi))
    }

    fn schemata_iter(
        &self,
    ) -> impl Iterator<Item = (String, RefOr<impl Reference<Target = impl Schema>>)> {
        SchemaIterator {
            openapi: self.openapi.clone(),
            curr: 0,
            end: self
                .openapi
                .components
                .as_ref()
                .map(|c| c.schemas.len())
                .unwrap_or(0),
        }
    }

    fn paths(&self) -> impl Iterator<Item = (String, impl PathItem)> {
        let paths: Vec<String> = self
            .openapi
            .paths
            .paths
            .iter()
            .filter_map(|(path, path_item_ref)| {
                if let ReferenceOr::Item(path_item) = path_item_ref {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        PathIterator {
            paths,
            current: 0,
            openapi: self.openapi.clone(),
        }
    }

    fn components(&self) -> Option<impl Components> {
        self.openapi.components.as_ref().map(|_| self)
    }
}

impl Components for &OAS30Spec {
    fn schemas(
        &self,
    ) -> impl Iterator<Item = (String, RefOr<impl Reference<Target = impl Schema>>)> {
        // TODO: move implementation here
        Spec::schemata_iter(*self)
    }
}

struct SchemaIterator {
    openapi: Rc<OpenAPI>,
    curr: usize,
    end: usize,
}

impl Iterator for SchemaIterator {
    type Item = (String, RefOr<OAS30SchemaReference>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == self.end {
            return None;
        }
        let v = self
            .openapi
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get_index(self.curr)
            .unwrap();
        let schema_name = v.0;
        let openapi = self.openapi.clone();
        self.curr = self.curr + 1;

        let ref_or = match v.1 {
            ReferenceOr::Reference { reference } => RefOr::Reference(OAS30SchemaReference {
                openapi,
                uri: reference.clone(),
            }),
            ReferenceOr::Item(item) => RefOr::Object(OAS30SchemaPointer {
                openapi,
                ref_source: SchemaSource::SchemaName(schema_name.clone()),
            }),
        };
        let r = (schema_name.clone(), ref_or);
        Some(r)
    }
}

// Path Iterator Implementation
struct PathIterator {
    paths: Vec<String>,
    current: usize,
    openapi: Rc<OpenAPI>,
}

impl Iterator for PathIterator {
    type Item = (String, OAS30PathItemRef);

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.paths.get(self.current);
        if let Some(path) = path {
            self.current += 1;
            return Some((
                path.clone(),
                OAS30PathItemRef {
                    path: path.clone(),
                    openapi: self.openapi.clone(),
                },
            ));
        }
        None
    }
}

// OAS30 PathItem Implementation
#[derive(Clone, Debug)]
pub struct OAS30PathItemRef {
    path: String,
    openapi: Rc<OpenAPI>,
}

fn extract_operation(
    operations: &mut Vec<OAS30OperationRef>,
    method: Method,
    operation: &Option<openapiv3::Operation>,
    path_item: &OAS30PathItemRef,
) {
    if operation.is_some() {
        operations.push(OAS30OperationRef {
            path_item: path_item.clone(),
            method: method,
        });
    }
}

pub struct OAS30ParametersRef {}

fn extract_parameter(
    parameters: &mut Vec<OAS30Parameter>,
    location: ParameterLocation,
    data: &openapiv3::ParameterData,
    openapi: Rc<OpenAPI>,
) {
    let param_name = data.name.clone();
    let schema = match &data.format {
        openapiv3::ParameterSchemaOrContent::Schema(schema_ref) => Some(schema_ref.clone()),
        openapiv3::ParameterSchemaOrContent::Content(_) => None,
    };
    parameters.push(OAS30Parameter {
        param_name,
        location,
        schema,
        openapi,
    });
}

fn to_parameters_iter(
    oas30_parameters: &Vec<openapiv3::ReferenceOr<openapiv3::Parameter>>,
    openapi: Rc<OpenAPI>,
) -> impl Iterator<Item = impl Parameter> {
    let mut params = Vec::new();
    for param_ref in oas30_parameters {
        match param_ref {
            ReferenceOr::Item(param) => match param {
                openapiv3::Parameter::Query { parameter_data, .. } => extract_parameter(
                    &mut params,
                    ParameterLocation::Query,
                    parameter_data,
                    openapi.clone(),
                ),
                openapiv3::Parameter::Header { parameter_data, .. } => extract_parameter(
                    &mut params,
                    ParameterLocation::Header,
                    parameter_data,
                    openapi.clone(),
                ),
                openapiv3::Parameter::Path { parameter_data, .. } => extract_parameter(
                    &mut params,
                    ParameterLocation::Path,
                    parameter_data,
                    openapi.clone(),
                ),
                openapiv3::Parameter::Cookie { parameter_data, .. } => extract_parameter(
                    &mut params,
                    ParameterLocation::Cookie,
                    parameter_data,
                    openapi.clone(),
                ),
            },
            _ => (),
        }
    }
    params.into_iter()
}

impl OAS30PathItemRef {
    fn resolve_inner(&self) -> Option<&openapiv3::PathItem> {
        let ro_opt = self.openapi.paths.paths.get(&self.path);
        ro_opt.unwrap(); // TODO: remove!
        ro_opt.and_then(|ro| self.openapi.resolve(ro))
    }
}

impl PathItem for OAS30PathItemRef {
    fn operations_iter(&self) -> impl Iterator<Item = (Method, impl Operation)> {
        let mut operations = Vec::new();

        let path_item = self.resolve_inner().unwrap();
        extract_operation(&mut operations, Method::GET, &path_item.get, self);
        extract_operation(&mut operations, Method::PUT, &path_item.put, self);
        extract_operation(&mut operations, Method::POST, &path_item.post, self);
        extract_operation(&mut operations, Method::DELETE, &path_item.delete, self);
        extract_operation(&mut operations, Method::OPTIONS, &path_item.options, self);
        extract_operation(&mut operations, Method::HEAD, &path_item.head, self);
        extract_operation(&mut operations, Method::PATCH, &path_item.patch, self);
        extract_operation(&mut operations, Method::TRACE, &path_item.trace, self);

        operations.into_iter().map(|op| (op.method.clone(), op))
    }

    fn parameters(&self) -> impl Iterator<Item = impl Parameter> {
        to_parameters_iter(
            &self.resolve_inner().unwrap().parameters,
            self.openapi.clone(),
        )
    }
}

// OAS30 Operation Implementation
#[derive(Debug)]
pub struct OAS30OperationRef {
    path_item: OAS30PathItemRef,
    method: http::Method,
}

impl OAS30OperationRef {
    fn resolve_inner(&self) -> Option<&openapiv3::Operation> {
        self.path_item
            .resolve_inner()
            .and_then(|path_item| match self.method {
                Method::GET => path_item.get.as_ref(),
                Method::DELETE => path_item.delete.as_ref(),
                Method::HEAD => path_item.head.as_ref(),
                Method::OPTIONS => path_item.options.as_ref(),
                Method::PATCH => path_item.patch.as_ref(),
                Method::POST => path_item.post.as_ref(),
                Method::PUT => path_item.put.as_ref(),
                Method::TRACE => path_item.trace.as_ref(),
                _ => panic!("unhandled method {:?}", self.method),
            })
    }
}

impl Operation for OAS30OperationRef {
    fn parameters(&self) -> impl Iterator<Item = impl Parameter> {
        to_parameters_iter(
            &self
                .resolve_inner()
                .expect(&format!("cannot resolve inner for {self:?}"))
                .parameters,
            self.path_item.openapi.clone(),
        )
    }

    fn operation_id(&self) -> Option<&str> {
        self.resolve_inner()?.operation_id.as_deref()
    }
}

// OAS30 Parameter Implementation
pub struct OAS30Parameter {
    param_name: String,
    location: ParameterLocation,
    schema: Option<openapiv3::ReferenceOr<openapiv3::Schema>>,
    openapi: Rc<OpenAPI>,
}

impl Parameter for OAS30Parameter {
    fn in_(&self) -> ParameterLocation {
        self.location
    }

    fn name(&self) -> &str {
        &self.param_name
    }

    fn schema(&self) -> Option<impl Schema> {
        if let Some(schema_ref) = &self.schema {
            match schema_ref {
                ReferenceOr::Reference { reference } => {
                    let schema_name = reference
                        .strip_prefix("#/components/schemas/")
                        .expect(&format!("Only references to '#/components/schemas/*' are supported, '{reference}' does not match"));
                    Some(OAS30SchemaPointer {
                        openapi: self.openapi.clone(),
                        ref_source: SchemaSource::SchemaName(schema_name.to_string()),
                    })
                }
                ReferenceOr::Item(schema) => {
                    // For inline schemas in parameters, we create an OAS30SchemaRef with InlineSchema variant
                    Some(OAS30SchemaPointer {
                        openapi: self.openapi.clone(),
                        ref_source: SchemaSource::OperationParam(schema.clone()),
                    })
                }
            }
        } else {
            None
        }
    }
}

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
    let schema = nf.1.resolve();
    let nf_props = schema.properties();

    let schema = nf_props.get("number_unformatted").unwrap();
    assert_eq!(type_of(schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), None);

    let schema = nf_props.get("number_double").unwrap();
    assert_eq!(type_of(schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Double));

    let schema = nf_props.get("number_float").unwrap();
    assert_eq!(type_of(schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Float));

    let schema = nf_props.get("integer_int64").unwrap();
    assert_eq!(type_of(schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Int64));

    let schema = nf_props.get("integer_int32").unwrap();
    assert_eq!(type_of(schema), Some(crate::types::Type::Number));
    assert_eq!(schema.format(), Some(Format::Int32));
}

#[cfg(test)]
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

#[cfg(test)]
#[test]
fn test_simple_paths() {
    use crate::types::Spec;
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

#[cfg(test)]
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

#[cfg(test)]
fn test_path_parameters_impl(spec: impl Spec) {
    // Test path_iter() implementation - should return exactly one parameterized path
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
    let param = &path_params[0];
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
    let param = &get_params[0];
    assert_eq!(param.name(), "with_foo");
    assert_eq!(param.in_(), ParameterLocation::Query);
}
