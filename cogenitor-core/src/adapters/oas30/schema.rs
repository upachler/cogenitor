use std::hash::Hash;
use std::{borrow::Borrow, collections::HashMap};

use openapiv3::{ParameterSchemaOrContent, ReferenceOr, Type};

use crate::adapters::oas30::{
    MediaTypeSource, OAS3Resolver, OAS30Pointer, OAS30Source, ParameterSource, SourceFromUri,
};
use crate::types::{BooleanOrSchema, RefOr, Schema};

use super::into_ref_or;

#[derive(Clone)]
pub enum SchemaSource {
    Uri(String),
    SchemaProperty((Box<SchemaSource>, String)),
    AdditionalProperties(Box<SchemaSource>),
    Items(Box<SchemaSource>),
    OperationParam(Box<ParameterSource>),
    MediaType(Box<MediaTypeSource>),
}

impl OAS30Source for SchemaSource {
    type OAS30Type = openapiv3::Schema;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        match self {
            SchemaSource::Uri(uri) => {
                let schema_name = uri
                    .strip_prefix(OAS3Resolver::<openapiv3::Schema>::prefix(openapi))
                    .unwrap();
                openapi.resolve_reference(schema_name).unwrap()
            }
            SchemaSource::AdditionalProperties(schema_ref) => {
                let ro = schema_from_additional_properties(schema_ref.inner(openapi)).unwrap();
                openapi.resolve(ro).unwrap()
            }
            SchemaSource::Items(schema_ref) => {
                let ro = schema_from_items(schema_ref.inner(openapi)).unwrap();
                openapi.resolve(ro).unwrap()
            }
            SchemaSource::SchemaProperty((schema_ref, name)) => {
                let ro = schema_from_property(schema_ref.inner(openapi), name).unwrap();
                openapi.resolve(ro).unwrap()
            }
            SchemaSource::MediaType(mediatype_source) => {
                let ro = mediatype_source.inner(openapi).schema.as_ref().unwrap();
                openapi.resolve(ro).unwrap()
            }
            SchemaSource::OperationParam(param_pointer) => {
                if let ParameterSchemaOrContent::Schema(schema_ro) =
                    &param_pointer.inner(openapi).parameter_data_ref().format
                {
                    schema_ro.as_item().unwrap()
                } else {
                    panic!(
                        "source created for schema from operation param where there is none defined"
                    )
                }
            }
        }
    }
}

impl SourceFromUri for SchemaSource {
    fn from_uri(uri: &str) -> Self {
        SchemaSource::Uri(uri.to_string())
    }
}

impl std::fmt::Debug for SchemaSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaSource::Uri(uri) => f.write_fmt(format_args!("'{uri}'")),
            SchemaSource::AdditionalProperties(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.additionalProperties"))
            }
            SchemaSource::SchemaProperty((oas30_schema_ref, name)) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.properties.{name}"))
            }
            SchemaSource::Items(oas30_schema_ref) => {
                f.write_fmt(format_args!("{oas30_schema_ref:?}.items"))
            }
            SchemaSource::MediaType(mediatype_source) => {
                f.write_fmt(format_args!("{mediatype_source:?}.schema"))
            }
            SchemaSource::OperationParam(_) => f.write_str("InlineSchema"),
        }
    }
}

impl Hash for SchemaSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            SchemaSource::Uri(uri) => uri.hash(state),
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
            SchemaSource::MediaType(p) => {
                state.write("m".as_bytes());
                p.hash(state);
            }
        }
    }
}

impl PartialEq for SchemaSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SchemaSource::Uri(s), SchemaSource::Uri(o)) => s.eq(o),

            (SchemaSource::SchemaProperty(s), SchemaSource::SchemaProperty(o)) => s.eq(o),
            (SchemaSource::AdditionalProperties(s), SchemaSource::AdditionalProperties(o)) => {
                s.eq(o)
            }
            (SchemaSource::Items(s), SchemaSource::Items(o)) => s.eq(o),
            (SchemaSource::OperationParam(_), SchemaSource::OperationParam(_)) => {
                todo!("this is broken, needs to compare path properly");
                // For simplicity, we consider all inline schemas as different
                // A proper implementation would compare schema content
                #[allow(unreachable_code)]
                false
            }
            (SchemaSource::MediaType(s), SchemaSource::MediaType(o)) => s.eq(o),
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

pub type OAS30SchemaPointer = OAS30Pointer<SchemaSource>;

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

impl Schema for OAS30Pointer<SchemaSource> {
    fn name(&self) -> Option<&str> {
        match &self.ref_source {
            SchemaSource::Uri(uri) => uri.rsplit('/').last(),
            SchemaSource::SchemaProperty((ref_source, name)) => {
                // the name of a schema referenced via a property of
                // onother schema is either tne name in the reference
                // (e.g. '#/components/schemas/MySchemaName') or
                // None for cases where the schema is inlined
                if let openapiv3::SchemaKind::Type(Type::Object(o)) =
                    &ref_source.inner(&self.openapi).schema_kind
                {
                    schema_name_of_reference_or(o.properties.get(name)?)
                } else {
                    None
                }
            }
            SchemaSource::Items(schema_ref) => {
                if let openapiv3::SchemaKind::Type(Type::Array(a)) =
                    &schema_ref.inner(&self.openapi).schema_kind
                {
                    schema_name_of_reference_or(a.items.as_ref()?)
                } else {
                    None
                }
            }
            SchemaSource::AdditionalProperties(schema_ref) => {
                if let openapiv3::SchemaKind::Type(Type::Object(o)) =
                    &schema_ref.inner(&self.openapi).schema_kind
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
            SchemaSource::MediaType(mediatype_source) => mediatype_source
                .inner(&self.openapi)
                .schema
                .as_ref()
                .and_then(|ro| schema_name_of_reference_or(ro)),
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

    fn properties(&self) -> std::collections::HashMap<String, RefOr<Self>> {
        use openapiv3::*;
        let mut m = HashMap::new();
        match &self.inner().schema_kind {
            SchemaKind::Type(Type::Object(t)) => {
                for (k, v) in t.properties.iter() {
                    let ro = into_ref_or(&v, self, |src| {
                        SchemaSource::SchemaProperty((Box::new(src.clone()), k.clone()))
                    });
                    m.insert(k.to_string(), ro);
                }
            }
            _ => (),
        };
        m
    }

    fn pattern_properties(&self) -> std::collections::HashMap<String, RefOr<impl Schema>> {
        HashMap::<_, RefOr<OAS30SchemaPointer>>::new()
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
                        ref_source: SchemaSource::AdditionalProperties(Box::new(
                            self.ref_source.clone(),
                        )),
                    })
                } else {
                    BooleanOrSchema::<Self>::Boolean(true)
                }
            }
            _ => BooleanOrSchema::<Self>::Boolean(true),
        }
    }

    fn items(&self) -> Option<Vec<RefOr<Self>>> {
        match &self.inner().schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Array(a)) => {
                if let Some(ro_items) = &a.items {
                    Some(vec![into_ref_or(ro_items, self, |src| {
                        SchemaSource::Items(Box::new(src.clone()))
                    })])
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
