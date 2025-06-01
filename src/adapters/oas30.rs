use std::hash::Hash;
use std::io::BufReader;
use std::str::FromStr;
use std::{borrow::Borrow, collections::HashMap, rc::Rc};

use openapiv3::{OpenAPI, ReferenceOr};

use crate::types::{BooleanOrSchema, Schema, Spec};

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
                let reference = reference
                    .strip_prefix("#/components/schemas/")
                    .expect(&format!("Only references to '#/components/schemas/*' are supported, '{reference}' does not match"));
                self.resolve_reference(reference)
            }
            ReferenceOr::Item(s) => Some(s.borrow()),
        }
    }

    fn resolve_reference(&self, reference: &str) -> Option<&T>;
}

impl OAS3Resolver<openapiv3::Schema> for openapiv3::OpenAPI {
    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::Schema> {
        let ro = self
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get(reference)
            .expect(format!("expected reference {reference} not found in OpenAPI object").as_ref());
        self.resolve(ro)
    }
}

#[derive(Clone)]
enum RefSource {
    SchemaName(String),
    SchemaProperty((Box<OAS30SchemaRef>, String)),
    AdditionalProperties(Box<OAS30SchemaRef>),
    Items(Box<OAS30SchemaRef>),
}

impl std::fmt::Debug for RefSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefSource::SchemaName(name) => f.write_fmt(format_args!("'{name}'")),
            RefSource::AdditionalProperties(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.additionalProperties"))
            }
            RefSource::SchemaProperty((oas30_schema_ref, name)) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.properties.{name}"))
            }
            RefSource::Items(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.items"))
            }
        }
    }
}

impl Hash for RefSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            RefSource::SchemaName(n) => n.hash(state),
            RefSource::SchemaProperty(p) => {
                state.write("p".as_bytes());
                p.0.hash(state);
                p.1.hash(state);
            }
            RefSource::AdditionalProperties(r) => {
                state.write("a".as_bytes());
                r.hash(state)
            }
            RefSource::Items(r) => {
                state.write("".as_bytes());
                r.hash(state);
            }
        }
    }
}

impl PartialEq for RefSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RefSource::SchemaName(s), RefSource::SchemaName(o)) => s.eq(o),
            (RefSource::SchemaProperty(s), RefSource::SchemaProperty(o)) => s.eq(o),
            (RefSource::AdditionalProperties(s), RefSource::AdditionalProperties(o)) => s.eq(o),
            _ => false,
        }
    }
}
impl Eq for RefSource {}

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
pub struct OAS30SchemaRef {
    openapi: Rc<OpenAPI>,
    ref_source: RefSource,
}

impl std::fmt::Debug for OAS30SchemaRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ref_source = &self.ref_source;
        f.write_fmt(format_args!("OAS30SchemaRef[{ref_source:?}]"))?;
        Ok(())
    }
}

impl Hash for OAS30SchemaRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ref_source.hash(state);
    }
}

impl PartialEq for OAS30SchemaRef {
    fn eq(&self, other: &Self) -> bool {
        self.ref_source.eq(&other.ref_source)
    }
}
impl Eq for OAS30SchemaRef {}

impl OAS30SchemaRef {
    fn inner(&self) -> &openapiv3::Schema {
        match &self.ref_source {
            RefSource::SchemaName(schema_name) => {
                self.openapi.resolve_reference(schema_name).unwrap()
            }
            RefSource::AdditionalProperties(schema_ref) => {
                let ro = schema_from_additional_properties(schema_ref.inner()).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
            RefSource::Items(schema_ref) => {
                let ro = schema_from_items(schema_ref.inner()).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
            RefSource::SchemaProperty((schema_ref, name)) => {
                let ro = schema_from_property(schema_ref.inner(), name).unwrap();
                self.openapi.resolve(ro).unwrap()
            }
        }
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

impl Schema for OAS30SchemaRef {
    fn name(&self) -> Option<&str> {
        match &self.ref_source {
            RefSource::SchemaName(name) => Some(name),
            _ => None,
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
                VariantOrUnknownOrEmpty::Item(NumberFormat::Double) => {
                    Some(crate::types::Format::Double)
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
        Option::<Vec<OAS30SchemaRef>>::None
    }

    fn any_of(&self) -> Option<Vec<impl Schema>> {
        Option::<Vec<OAS30SchemaRef>>::None
    }

    fn one_of(&self) -> Option<Vec<impl Schema>> {
        Option::<Vec<OAS30SchemaRef>>::None
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
                    let ref_source = RefSource::SchemaProperty((Box::new(self.clone()), k.clone()));
                    let type_ = OAS30SchemaRef {
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
        HashMap::<_, OAS30SchemaRef>::new()
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
                        ref_source: RefSource::AdditionalProperties(Box::new(self.clone())),
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
                let ref_source = RefSource::Items(Box::new(self.clone()));
                Some(vec![OAS30SchemaRef {
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
    type Schema = OAS30SchemaRef;

    fn from_reader(r: impl std::io::Read) -> anyhow::Result<impl crate::Spec> {
        let r = BufReader::new(r);
        let openapi: OpenAPI = serde_yaml::from_reader(r)?;
        Ok(OAS30Spec::from(openapi))
    }

    fn schemata_iter(&self) -> impl Iterator<Item = (String, Self::Schema)> {
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
}

struct SchemaIterator {
    openapi: Rc<OpenAPI>,
    curr: usize,
    end: usize,
}

impl Iterator for SchemaIterator {
    type Item = (String, OAS30SchemaRef);

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
        let schema_name = v.0.clone();
        let openapi = self.openapi.clone();
        let r = (
            schema_name.clone(),
            OAS30SchemaRef {
                openapi,
                ref_source: RefSource::SchemaName(schema_name),
            },
        );
        self.curr = self.curr + 1;
        Some(r)
    }
}

#[test]
fn test_empty() {
    let oas = r"
openapi: 3.0.0
info:
    title: Empty API
    version: v1
paths:";
    println!("parting {oas}");
    let spec = OAS30Spec::from_str(oas).unwrap();
    assert!(spec.schemata_iter().next().is_none());
}
