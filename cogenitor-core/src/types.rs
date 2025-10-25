use std::{collections::HashMap, io, num::ParseIntError, str::FromStr, string::ParseError};

use json::JsonValue;

/// An implementation of an OAS spec, specific to our needs for code generation
pub trait Spec: FromStr<Err = anyhow::Error> {
    type Schema: Schema;
    type Components: Components<Self>;
    type PathItem: PathItem<Self>;
    type Parameter: Parameter<Self>;
    type MediaType: MediaType<Self>;
    type Operation: Operation<Self>;
    type RequestBody: RequestBody<Self>;
    type Response: Response<Self>;

    fn from_reader(r: impl io::Read) -> anyhow::Result<impl Spec>;

    fn components(&self) -> Option<Self::Components>;

    fn paths(&self) -> impl Iterator<Item = (String, Self::PathItem)>;

    fn schemata_iter(&self) -> impl Iterator<Item = (String, RefOr<Self::Schema>)>;
}

pub trait Components<S: Spec> {
    fn schemas(&self) -> impl Iterator<Item = (String, RefOr<S::Schema>)>;
}

/**
representation of possible values of the [type](https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.21)
keyword.
Note that we do not distinguish between `number` and `integer` for the same reasons described in the section about integegers [here](https://spec.openapis.org/oas/v3.0.4.html#data-types)
*/
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Null,
    Boolean,
    Object,
    Array,
    Number,
    String,
}

/**
Formats, as per
https://spec.openapis.org/oas/v3.0.4.html#data-type-format
*/
#[derive(Debug, PartialEq)]
pub enum Format {
    Int32,
    Int64,
    Float,
    Double,
    Byte,
    Binary,
    Date,
    DateTime,
    Password,
}

#[derive(Debug)]
pub enum BooleanOrSchema<S>
where
    S: Schema,
{
    Boolean(bool),
    Schema(S),
}

/**
Represents a schema for validating a JSON data item.
We use this for type generation, so only fields relevant for this purpose are implemented.
See https://spec.openapis.org/oas/v3.0.4.html#schema-object
*/
#[allow(unused)]
pub trait Schema: Clone + std::fmt::Debug + std::hash::Hash + Eq + ByReference {
    /**
    If this schema is named (i.e. a YAML/JSON key is associated with its definition),
    this method returns that name.
    Note that this will not return names of primitive types. However, if a primitive
    type is referenced in the `type` schema property and that schema is named, the
    name will be returned (so this represents a type alias)
    */
    fn name(&self) -> Option<&str>;

    /**
     */
    fn type_(&self) -> Option<Vec<Type>>;
    /**
    https://spec.openapis.org/oas/v3.0.4.html#data-type-format
    https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-7
    */
    fn format(&self) -> Option<Format>;
    /**
    https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6.1
     */
    fn title(&self) -> Option<&str>;
    fn description(&self) -> Option<&str>;

    fn required(&self) -> Option<Vec<&str>>;

    fn all_of(&self) -> Option<Vec<impl Schema>>;
    fn any_of(&self) -> Option<Vec<impl Schema>>;
    fn one_of(&self) -> Option<Vec<impl Schema>>;
    fn enum_(&self) -> Option<Vec<JsonValue>>;

    /** https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.16 */
    // TODO: change 'impl Schema' to RefOr<impl Schema>
    fn properties(&self) -> HashMap<String, RefOr<Self>>;
    /** https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.17 */
    fn pattern_properties(&self) -> HashMap<String, RefOr<impl Schema>>;
    /** https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.18 */
    fn addtional_properties(&self) -> BooleanOrSchema<impl Schema>;

    /**
    see 'items' following https://spec.openapis.org/oas/v3.0.4.html#x4-7-24-1-json-schema-keywords
    see https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.9
    */
    fn items(&self) -> Option<Vec<RefOr<Self>>>;
}

// https://spec.openapis.org/oas/v3.0.4.html#x4-7-10-operation-object
pub trait PathItem<S: Spec> {
    // see 'get', 'put', ... in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-9-1-fixed-fields
    fn operations_iter(&self) -> impl Iterator<Item = (http::Method, S::Operation)>;
    // see 'parameters' in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-9-1-fixed-fields
    fn parameters(&self) -> impl Iterator<Item = RefOr<S::Parameter>>;
}

// see https://spec.openapis.org/oas/v3.0.4.html#x4-7-10
#[allow(unused)]
pub trait Operation<S: Spec> {
    // see 'parameters' in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-10-1-fixed-fields
    fn parameters(&self) -> impl Iterator<Item = RefOr<S::Parameter>>;
    fn operation_id(&self) -> Option<&str>;
    fn request_body(&self) -> Option<RefOr<S::RequestBody>>;
    fn responses(&self) -> impl Iterator<Item = (StatusSpec, RefOr<S::Response>)>;
}

#[derive(Clone, Copy, Debug, Hash, PartialEq)]
pub enum StatusSpec {
    /// specifies all responses
    Default,
    Informational(u16),
    Informational1XX,
    Success(u16),
    Success2XX,
    Redirection(u16),
    Redirection3XX,
    ClientError(u16),
    ClientError4XX,
    ServerError(u16),
    ServerError5XX,
}

/// https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-1-parameter-locations
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ParameterLocation {
    Query,
    Header,
    Path,
    Cookie,
}

/// see https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-parameter-object
pub trait Parameter<S: Spec>: ByReference + Clone {
    /// see https://spec.openapis.org/oas/v3.0.4.html#parameter-in
    fn in_(&self) -> ParameterLocation;
    fn name(&self) -> &str;

    /// `Parameter` must either contain a `schema` or a `content` field
    /// - so only either one of them can be `None`
    fn schema(&self) -> Option<RefOr<S::Schema>>;

    fn content(&self) -> Option<HashMap<String, S::MediaType>>;
}

/// see https://spec.openapis.org/oas/v3.0.4.html#request-body-object
#[allow(unused)]
pub trait RequestBody<S: Spec>: ByReference + Clone {
    fn content(&self) -> HashMap<String, S::MediaType>;
    fn required(&self) -> bool;
}

// see https://spec.openapis.org/oas/v3.0.4.html#response-object
pub trait Response<S: Spec>: ByReference + Clone {
    fn content(&self) -> HashMap<String, S::MediaType>;
}

/// see https://spec.openapis.org/oas/v3.0.4.html#media-type-object
pub trait MediaType<S: Spec> {
    fn schema(&self) -> Option<RefOr<S::Schema>>;
}

/// Types implementing `Reference` contain the path in the OAS tree
/// as well as the means necessary to resolve that path.
/// Effectively, a type implementing `Reference` encapsulates
/// the URI (e.g. `#/components/schemas/Pet`) and a handle to the
/// underlying OpenAPI data structure. The `resolve()` method
/// performs the actual resolution. Note that `Reference`s
/// are meant to be infallible: Implemenations assume that the
/// URIs they were constructed from can _always_ be resolved
/// successfully.
pub trait Reference<T>: PartialEq + Eq + Clone + std::fmt::Debug
where
    T: ByReference,
{
    /// resolve the URI to the actual target object
    fn resolve(&self) -> RefOr<T>;

    /// the URI to resolve to the target object
    fn uri(&self) -> &str;
}

/// Marker trait for for OAS object traits that also support
/// being available via references. For instance [Schema]
/// will implement [ByReference], as it is available via
/// `$ref` in an OAS document, but [Operation] will not.
pub trait ByReference: Sized {
    type Reference: Reference<Self>;
}

/// Implementation of the OAS 'reference or ... object' pattern.
///
/// Several (but by far not all) OAS objects can be referenced
/// by URIs via the `$ref` keyword. For instance, a parameter
/// type can be expressed by an inline schema
///
/// ```yaml
/// paths:
///   'baz'
///     get:
///      parameters:
///      - name: foo
///         schema:
///           type: object
///           properties:
///             bar:
///               type: string
///  ```
///
/// or by a reference to a named schema:
///
/// ```yaml
/// paths:
///   'baz'
///     get:
///      parameters:
///      - name: foo
///         schema:
///           $ref: '#/components/schemas/Foo'
/// ...
///
/// components:
///   schemas:
///     Foo:
///       type: object
///       properties:
///         bar:
///           type: string
/// ```
///
/// So the 'schema' property in 'parameters' allows _either_ a reference _or_ an inlined OAS object.
/// This is true for several other OAS objects as well. Therefore, `RefOr` implements this pattern;
/// it's variants implement the reference and the inline object case.
///
#[derive(Clone, Debug)]
pub enum RefOr<O>
where
    O: ByReference,
{
    Reference(O::Reference),
    Object(O),
}

impl<O> RefOr<O>
where
    O: ByReference,
    <O as ByReference>::Reference: Clone,
    O: Clone,
{
    pub fn resolve(&self) -> RefOr<O> {
        match self {
            RefOr::Reference(r) => r.resolve(),
            RefOr::Object(_o) => (*self).clone(),
        }
    }

    pub fn resolve_fully(&self) -> O {
        let mut ro = self.resolve();
        while let RefOr::Reference(_) = &ro {
            ro = ro.resolve()
        }
        ro.as_object().unwrap()
    }

    pub fn as_object(&self) -> Option<O> {
        match self {
            RefOr::Reference(_) => None,
            RefOr::Object(o) => Some(o.clone()),
        }
    }
}

impl<O> std::hash::Hash for RefOr<O>
where
    O: ByReference,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl<O: PartialEq> PartialEq for RefOr<O>
where
    O: ByReference,
    O: Clone,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Reference(l0), Self::Reference(r0)) => l0 == r0,
            (Self::Object(l0), Self::Object(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl<O: Eq> Eq for RefOr<O>
where
    RefOr<O>: PartialEq,
    O: ByReference,
{
}
