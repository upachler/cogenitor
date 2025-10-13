mod schema;

pub use schema::*;

use std::fmt::Debug;
use std::hash::Hash;
use std::io::BufReader;
use std::str::FromStr;
use std::{borrow::Borrow, collections::HashMap, rc::Rc};

use http::Method;
use indexmap::IndexMap;
use openapiv3::{OpenAPI, ParameterSchemaOrContent, ReferenceOr};

use crate::types::{
    ByReference, Components, MediaType, Operation, Parameter, ParameterLocation, PathItem, RefOr,
    Reference, RequestBody, Response, Spec, StatusSpec,
};
#[cfg(test)]
use crate::types::{Format, Schema};

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
    fn prefix(&self) -> &'static str {
        "#/components/schemas/"
    }
}

impl OAS3Resolver<openapiv3::PathItem> for openapiv3::OpenAPI {
    fn prefix(&self) -> &'static str {
        "#/paths/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::PathItem> {
        let ro = self.paths.paths.get(reference)?;
        self.resolve(ro)
    }
}

impl OAS3Resolver<openapiv3::Parameter> for openapiv3::OpenAPI {
    fn prefix(&self) -> &str {
        "#/components/parameters/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::Parameter> {
        let ro = self.components.as_ref()?.parameters.get(reference)?;
        self.resolve(ro)
    }
}

impl OAS3Resolver<openapiv3::RequestBody> for openapiv3::OpenAPI {
    fn prefix(&self) -> &str {
        "#/components/requestBodies/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::RequestBody> {
        let ro = self.components.as_ref()?.request_bodies.get(reference)?;
        self.resolve(ro)
    }
}

impl OAS3Resolver<openapiv3::Response> for openapiv3::OpenAPI {
    fn prefix(&self) -> &str {
        "#/components/responses/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::Response> {
        let ro = self.components.as_ref()?.responses.get(reference)?;
        self.resolve(ro)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ComponentsSource;

impl OAS30Source for ComponentsSource {
    type OAS30Type = openapiv3::Components;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        openapi.components.as_ref().unwrap()
    }
}

pub trait OAS30Source: std::fmt::Debug + Hash + PartialEq {
    type OAS30Type;
    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b;
}

#[derive(Clone)]
pub struct OAS30Pointer<S: OAS30Source> {
    openapi: Rc<OpenAPI>, // TODO: remove openapi field, likely not needed
    ref_source: S,
}

pub type OAS30SchemaPointer = OAS30Pointer<SchemaSource>;

impl<S: OAS30Source> std::fmt::Debug for OAS30Pointer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ref_source = &self.ref_source;
        f.write_fmt(format_args!("OAS30Pointer[{ref_source:?}]"))?;
        Ok(())
    }
}

impl<S: OAS30Source + Hash> Hash for OAS30Pointer<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ref_source.hash(state);
    }
}

impl<S: OAS30Source + PartialEq> PartialEq for OAS30Pointer<S> {
    fn eq(&self, other: &Self) -> bool {
        self.ref_source.eq(&other.ref_source)
    }
}
impl<S: OAS30Source + Eq> Eq for OAS30Pointer<S> {}

impl<S: OAS30Source> OAS30Pointer<S> {
    fn inner(&self) -> &S::OAS30Type {
        self.ref_source.inner(&self.openapi)
    }
}

#[derive(Clone, PartialEq)]
pub struct OAS30Reference {
    openapi: Rc<OpenAPI>,
    uri: String,
}

impl Debug for OAS30Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAS30Reference")
            .field("uri", &self.uri)
            .finish()
    }
}
impl Hash for OAS30Reference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // NOTE: we are not hashing Rc<OpenAPI> because you typically only
        // ever use OAS30References with openapi fields pointing to the same
        // OpenAPI instance
        self.uri.hash(state);
    }
}

impl Eq for OAS30Reference {}

trait SourceFromUri {
    fn from_uri(uri: &str) -> Self;
}

impl SourceFromUri for RequestBodySource {
    fn from_uri(uri: &str) -> Self {
        RequestBodySource::Uri {
            uri: uri.to_string(),
        }
    }
}

impl<S: OAS30Source> Reference<OAS30Pointer<S>> for OAS30Reference
where
    S: SourceFromUri,
{
    fn resolve(&self) -> RefOr<OAS30Pointer<S>> {
        RefOr::Object(OAS30Pointer {
            openapi: self.openapi.clone(),
            ref_source: S::from_uri(&self.uri),
        })
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

impl<S: OAS30Source + SourceFromUri> ByReference for OAS30Pointer<S> {
    type Reference = OAS30Reference;
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
    type Schema = OAS30Pointer<SchemaSource>;
    type Components = OAS30Pointer<ComponentsSource>;
    type PathItem = OAS30Pointer<PathItemSource>;
    type Parameter = OAS30Pointer<ParameterSource>;
    type MediaType = OAS30Pointer<MediaTypeSource>;
    type Operation = OAS30Pointer<OperationSource>;
    type RequestBody = OAS30Pointer<RequestBodySource>;
    type Response = OAS30Pointer<ResponseSource>;

    fn from_reader(r: impl std::io::Read) -> anyhow::Result<impl crate::Spec> {
        let r = BufReader::new(r);
        let openapi: OpenAPI = serde_yaml::from_reader(r)?;
        Ok(OAS30Spec::from(openapi))
    }

    fn schemata_iter(&self) -> impl Iterator<Item = (String, RefOr<Self::Schema>)> {
        self.components()
            .iter()
            .flat_map(|c| c.schemas())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn paths(&self) -> impl Iterator<Item = (String, Self::PathItem)> {
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

    fn components(&self) -> Option<OAS30Pointer<ComponentsSource>> {
        self.openapi.components.as_ref().map(|_| OAS30Pointer {
            openapi: self.openapi.clone(),
            ref_source: ComponentsSource {},
        })
    }
}

impl Components<OAS30Spec> for OAS30Pointer<ComponentsSource> {
    fn schemas(&self) -> impl Iterator<Item = (String, RefOr<OAS30Pointer<SchemaSource>>)> {
        self.inner().schemas.iter().map(|(name, schema_ro)| {
            (
                name.clone(),
                into_ref_or(schema_ro, self, |_| {
                    SchemaSource::Uri(format!("#/components/schemas/{name}"))
                }),
            )
        })
    }
}

// Path Iterator Implementation
struct PathIterator {
    paths: Vec<String>,
    current: usize,
    openapi: Rc<OpenAPI>,
}

impl Iterator for PathIterator {
    type Item = (String, OAS30PathItemPointer);

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.paths.get(self.current);
        if let Some(path) = path {
            self.current += 1;
            return Some((
                path.clone(),
                OAS30PathItemPointer {
                    ref_source: PathItemSource { path: path.clone() },
                    openapi: self.openapi.clone(),
                },
            ));
        }
        None
    }
}

// OAS30 PathItem Implementation
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct PathItemSource {
    path: String,
}

type OAS30PathItemPointer = OAS30Pointer<PathItemSource>;

pub struct OAS30ParametersRef {}

fn to_parameters_iter(
    parent: &OAS30Pointer<impl OAS30Source>,
    oas30_parameters: &Vec<openapiv3::ReferenceOr<openapiv3::Parameter>>,
    parameter_source_factory: impl Fn(ParameterLocalId) -> ParameterSource,
) -> impl Iterator<Item = RefOr<OAS30Pointer<ParameterSource>>> {
    let mut params = Vec::new();
    for param_ref in oas30_parameters {
        let p = into_ref_or(param_ref, &parent, |p| {
            let param = param_ref.as_item().unwrap();
            let param_id = ParameterLocalId {
                location: extract_location(&param),
                param_name: param.parameter_data_ref().name.clone(),
            };
            parameter_source_factory(param_id)
        });
        params.push(p);
    }
    params.into_iter()
}

impl OAS30Source for PathItemSource {
    type OAS30Type = openapiv3::PathItem;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        let ro_opt = openapi.paths.paths.get(&self.path);
        ro_opt.and_then(|ro| openapi.resolve(ro)).unwrap()
    }
}

impl PathItem<OAS30Spec> for OAS30PathItemPointer {
    fn operations_iter(&self) -> impl Iterator<Item = (Method, OAS30Pointer<OperationSource>)> {
        let path_item = self.inner();
        vec![
            (Method::GET, &path_item.get),
            (Method::PUT, &path_item.put),
            (Method::POST, &path_item.post),
            (Method::DELETE, &path_item.delete),
            (Method::OPTIONS, &path_item.options),
            (Method::HEAD, &path_item.head),
            (Method::PATCH, &path_item.patch),
            (Method::TRACE, &path_item.trace),
        ]
        .into_iter()
        .filter_map(|(method, operation_opt)| operation_opt.as_ref().map(|_operation| method))
        .map(|method| {
            let ref_source = OperationSource {
                path_item: self.ref_source.clone(),
                method: method.clone(),
            };
            (
                method,
                OAS30Pointer {
                    openapi: self.openapi.clone(),
                    ref_source,
                },
            )
        })
    }

    fn parameters(&self) -> impl Iterator<Item = RefOr<OAS30Pointer<ParameterSource>>> {
        to_parameters_iter(self, &self.inner().parameters, |param_id| {
            ParameterSource::PathItem {
                source_ref: self.ref_source.clone(),
                param_id,
            }
        })
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub struct OperationSource {
    path_item: PathItemSource,
    method: http::Method,
}

impl OAS30Source for OperationSource {
    type OAS30Type = openapiv3::Operation;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        let path_item = self.path_item.inner(openapi);

        let op = match self.method {
            Method::GET => &path_item.get,
            Method::DELETE => &path_item.delete,
            Method::HEAD => &path_item.head,
            Method::OPTIONS => &path_item.options,
            Method::PATCH => &path_item.patch,
            Method::POST => &path_item.post,
            Method::PUT => &path_item.put,
            Method::TRACE => &path_item.trace,
            _ => panic!("unhandled method {:?}", self.method),
        };
        op.as_ref().unwrap()
    }
}

impl Operation<OAS30Spec> for OAS30Pointer<OperationSource> {
    fn parameters(&self) -> impl Iterator<Item = RefOr<OAS30Pointer<ParameterSource>>> {
        let source_ref = &self.ref_source;
        to_parameters_iter(self, &self.inner().parameters, |param_id| {
            ParameterSource::Operation {
                source_ref: source_ref.clone(),
                param_id,
            }
        })
    }

    fn operation_id(&self) -> Option<&str> {
        self.inner().operation_id.as_deref()
    }

    fn request_body(&self) -> Option<RefOr<OAS30Pointer<RequestBodySource>>> {
        self.inner().request_body.as_ref().map(|request_body| {
            into_ref_or(request_body, self, |_p| RequestBodySource::Operation {
                source_ref: self.ref_source.clone(),
            })
        })
    }

    fn responses(
        &self,
    ) -> impl Iterator<
        Item = (
            crate::types::StatusSpec,
            RefOr<<OAS30Spec as Spec>::Response>,
        ),
    > {
        self.inner().responses.responses.iter().enumerate().map(
            |(content_index, (status, ro_response))| {
                let status = StatusSpec::try_from(status).unwrap();
                (
                    status.clone(),
                    into_ref_or(ro_response, self, |p: &OperationSource| {
                        ResponseSource::Operation {
                            content_index,
                            ref_source: p.clone(),
                        }
                    }),
                )
            },
        )
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ParameterLocalId {
    param_name: String,
    location: ParameterLocation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterSource {
    Uri {
        uri: String,
    },
    Operation {
        source_ref: OperationSource,
        param_id: ParameterLocalId,
    },
    PathItem {
        source_ref: PathItemSource,
        param_id: ParameterLocalId,
    },
}

impl SourceFromUri for ParameterSource {
    fn from_uri(uri: &str) -> Self {
        ParameterSource::Uri {
            uri: uri.to_string(),
        }
    }
}
impl Hash for ParameterSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

fn extract_location(param: &openapiv3::Parameter) -> ParameterLocation {
    match param {
        openapiv3::Parameter::Query { .. } => ParameterLocation::Query,
        openapiv3::Parameter::Header { .. } => ParameterLocation::Header,
        openapiv3::Parameter::Path { .. } => ParameterLocation::Path,
        openapiv3::Parameter::Cookie { .. } => ParameterLocation::Cookie,
    }
}

impl ParameterSource {
    fn extract_param<'a>(
        params: &'a Vec<ReferenceOr<openapiv3::Parameter>>,
        param_id: &ParameterLocalId,
    ) -> &'a openapiv3::Parameter {
        params
            .iter()
            .find(|p| {
                let p = p.as_item().unwrap();
                let loc = extract_location(p);
                let pd = p.parameter_data_ref();
                pd.name == param_id.param_name && loc == param_id.location
            })
            .unwrap()
            .as_item()
            .unwrap()
    }
}

impl OAS30Source for ParameterSource {
    type OAS30Type = openapiv3::Parameter;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        match self {
            ParameterSource::Uri { uri } => openapi.resolve_reference(uri).unwrap(),
            ParameterSource::Operation {
                source_ref,
                param_id,
            } => Self::extract_param(&source_ref.inner(openapi).parameters, param_id),
            ParameterSource::PathItem {
                source_ref,
                param_id,
            } => Self::extract_param(&source_ref.inner(openapi).parameters, param_id),
        }
    }
}

impl Parameter<OAS30Spec> for OAS30Pointer<ParameterSource> {
    fn in_(&self) -> ParameterLocation {
        extract_location(self.ref_source.inner(&self.openapi))
    }

    fn name(&self) -> &str {
        &self
            .ref_source
            .inner(&self.openapi)
            .parameter_data_ref()
            .name
    }

    fn schema(&self) -> Option<RefOr<OAS30Pointer<SchemaSource>>> {
        if let ParameterSchemaOrContent::Schema(schema_ref) =
            &self.inner().parameter_data_ref().format
        {
            Some(into_ref_or(schema_ref, self, |p| {
                SchemaSource::OperationParam(Box::new(self.ref_source.clone()))
            }))
        } else {
            None
        }
    }

    fn content(&self) -> Option<HashMap<String, OAS30Pointer<MediaTypeSource>>> {
        match &self.inner().parameter_data_ref().format {
            ParameterSchemaOrContent::Schema(_reference_or) => None,
            ParameterSchemaOrContent::Content(index_map) => {
                Some(into_oas30_content(index_map, |content_index| {
                    OAS30Pointer {
                        openapi: self.openapi.clone(),
                        ref_source: MediaTypeSource::Parameter {
                            ref_source: self.ref_source.clone(),
                            content_index,
                        },
                    }
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum RequestBodySource {
    Uri { uri: String },
    Operation { source_ref: OperationSource },
}

impl OAS30Source for RequestBodySource {
    type OAS30Type = openapiv3::RequestBody;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        match self {
            RequestBodySource::Uri { uri } => openapi.resolve_reference(uri).unwrap(),
            RequestBodySource::Operation { source_ref } => source_ref
                .inner(openapi)
                .request_body
                .as_ref()
                .and_then(ReferenceOr::as_item)
                .unwrap(),
        }
    }
}

impl RequestBody<OAS30Spec> for OAS30Pointer<RequestBodySource> {
    fn content(&self) -> HashMap<String, OAS30Pointer<MediaTypeSource>> {
        into_oas30_content(&self.inner().content, |content_index| OAS30Pointer {
            openapi: self.openapi.clone(),
            ref_source: MediaTypeSource::RequestBody {
                ref_source: self.ref_source.clone(),
                content_index,
            },
        })
    }
    fn required(&self) -> bool {
        self.inner().required
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum ResponseSource {
    Uri {
        uri: String,
    },
    Operation {
        content_index: usize,
        ref_source: OperationSource,
    },
}

impl OAS30Source for ResponseSource {
    type OAS30Type = openapiv3::Response;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        match self {
            ResponseSource::Uri { uri } => openapi.resolve_reference(uri).unwrap(),
            ResponseSource::Operation {
                content_index,
                ref_source,
            } => {
                let ro = ref_source
                    .inner(openapi)
                    .responses
                    .responses
                    .get_index(*content_index)
                    .unwrap()
                    .1;
                openapi.resolve(ro).unwrap()
            }
        }
    }
}

impl Response<OAS30Spec> for OAS30Pointer<ResponseSource> {
    fn content(&self) -> HashMap<String, <OAS30Spec as Spec>::MediaType> {
        into_oas30_content(&self.inner().content, |content_index| OAS30Pointer {
            openapi: self.openapi.clone(),
            ref_source: MediaTypeSource::Response {
                ref_source: self.ref_source.clone(),
                content_index,
            },
        })
    }
}

impl SourceFromUri for ResponseSource {
    fn from_uri(uri: &str) -> Self {
        ResponseSource::Uri {
            uri: uri.to_string(),
        }
    }
}

impl TryFrom<&openapiv3::StatusCode> for StatusSpec {
    type Error = <StatusSpec as FromStr>::Err;
    fn try_from(s: &openapiv3::StatusCode) -> Result<Self, Self::Error> {
        let s = s.to_string();
        StatusSpec::from_str(&s)
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum MediaTypeSource {
    Parameter {
        ref_source: ParameterSource,
        content_index: usize,
    },
    RequestBody {
        ref_source: RequestBodySource,
        content_index: usize,
    },
    Response {
        ref_source: ResponseSource,
        content_index: usize,
    },
    //    Header{ref_source: HeaderSource, content: String}
}
impl OAS30Source for MediaTypeSource {
    type OAS30Type = openapiv3::MediaType;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        let (content, index) = match &self {
            MediaTypeSource::Parameter {
                ref_source,
                content_index,
            } => match &ref_source.inner(openapi).parameter_data_ref().format {
                ParameterSchemaOrContent::Schema(reference_or) => panic!(
                    "source was initialized for invalid parameter with 'schema' property, not 'content'"
                ),
                ParameterSchemaOrContent::Content(index_map) => (index_map, content_index),
            },
            MediaTypeSource::RequestBody {
                ref_source,
                content_index,
            } => (&ref_source.inner(openapi).content, content_index),
            MediaTypeSource::Response {
                ref_source,
                content_index,
            } => (&ref_source.inner(openapi).content, content_index),
        };
        content.get_index(*index).unwrap().1
    }
}

fn into_oas30_content(
    content: &IndexMap<String, openapiv3::MediaType>,
    src_fn: impl Fn(usize) -> OAS30Pointer<MediaTypeSource>,
) -> HashMap<String, OAS30Pointer<MediaTypeSource>> {
    content
        .as_slice()
        .iter()
        .enumerate()
        .map(|(content_index, (mt_key, _))| (mt_key.clone(), src_fn(content_index)))
        .collect()
}

impl MediaType<OAS30Spec> for OAS30Pointer<MediaTypeSource> {
    fn schema(&self) -> Option<RefOr<OAS30Pointer<SchemaSource>>> {
        self.inner()
            .schema
            .as_ref()
            .map(|m| into_ref_or(m, &self, |p| SchemaSource::MediaType(Box::new(p.clone()))))
    }
}

/// Convert the `openapiv3::ReferenceOr<I>` into our `RefOr<>`
/// abstraction for the OAS30 implementation.
/// `parent_pointer` is the OAS structure that is the parent
/// of the current item that we want to convert.
/// `src_fn` takes the source of the parent structure
/// and returns the source for structure we want to wrap in
/// `RefOr<>`
fn into_ref_or<S, T, I>(
    reference_or: &openapiv3::ReferenceOr<I>,
    parent_pointer: &OAS30Pointer<T>,
    src_fn: impl FnOnce(&T) -> S,
) -> RefOr<OAS30Pointer<S>>
where
    S: OAS30Source,
    S: SourceFromUri,
    T: OAS30Source,
{
    match reference_or {
        ReferenceOr::Reference { reference } => RefOr::Reference(OAS30Reference {
            openapi: parent_pointer.openapi.clone(),
            uri: reference.clone(),
        }),
        ReferenceOr::Item(_object) => {
            let s = src_fn(&parent_pointer.ref_source);
            let p = OAS30Pointer {
                openapi: parent_pointer.openapi.clone(),
                ref_source: s,
            };
            RefOr::Object(p)
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
