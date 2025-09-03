use http::Uri;
use std::{collections::HashMap, io, str::FromStr};

use json::JsonValue;

/** An implementation of an OAS spec, specific to our needs for code generation */
pub trait Spec: FromStr<Err = anyhow::Error> {
    type Schema: Schema;

    fn from_reader(r: impl io::Read) -> anyhow::Result<impl Spec>;

    fn components(&self) -> Option<impl Components>;

    fn paths(&self) -> impl Iterator<Item = (String, impl PathItem)>;

    fn schemata_iter(&self) -> impl Iterator<Item = (String, RefOr<impl Schema>)>;
}

pub trait Components {
    fn schemas(&self) -> impl Iterator<Item = (String, RefOr<impl Schema>)>;
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
    fn properties(&self) -> HashMap<String, impl Schema>;
    /** https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.17 */
    fn pattern_properties(&self) -> HashMap<String, impl Schema>;
    /** https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.18 */
    fn addtional_properties(&self) -> BooleanOrSchema<impl Schema>;

    /**
    see 'items' following https://spec.openapis.org/oas/v3.0.4.html#x4-7-24-1-json-schema-keywords
    see https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.9
    */
    fn items(&self) -> Option<Vec<impl Schema>>;
}

// https://spec.openapis.org/oas/v3.0.4.html#x4-7-10-operation-object
pub trait PathItem {
    // see 'get', 'put', ... in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-9-1-fixed-fields
    fn operations_iter(&self) -> impl Iterator<Item = (http::Method, impl Operation)>;
    // see 'parameters' in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-9-1-fixed-fields
    fn parameters(&self) -> impl Iterator<Item = impl Parameter>;
}

// see https://spec.openapis.org/oas/v3.0.4.html#x4-7-10
pub trait Operation {
    // see 'parameters' in  https://spec.openapis.org/oas/v3.0.4.html#x4-7-10-1-fixed-fields
    fn parameters(&self) -> impl Iterator<Item = impl Parameter>;
    fn operation_id(&self) -> Option<&str>;
}

/// https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-1-parameter-locations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterLocation {
    Query,
    Header,
    Path,
    Cookie,
}

/// see https://spec.openapis.org/oas/v3.0.4.html#x4-7-12-parameter-object
pub trait Parameter {
    /// see https://spec.openapis.org/oas/v3.0.4.html#parameter-in
    fn in_(&self) -> ParameterLocation;
    fn name(&self) -> &str;

    /// `Parameter` must either contain a `schema` or a `content` field
    /// - so only either one of them can be `None`
    fn schema(&self) -> Option<impl Schema>;
}

struct OASPath {}

/// types implementing Reference contain the path in the OAS tree
/// as well as the means necessary to resolve that path
pub trait Reference<T> {
    /// resolve the URI to the actual target object
    fn resolve(&self) -> T;

    /// the URI to resolve to the target object
    fn uri(&self) -> &str;
}

pub trait ByReference: Sized {
    type Reference: Reference<Self>;
}

pub enum RefOr<T>
where
    T: ByReference,
{
    Reference(T::Reference),
    Object(T),
}

impl<T> RefOr<T>
where
    T: ByReference,
    T: Clone,
{
    pub fn resolve(&self) -> T {
        match self {
            RefOr::Reference(r) => r.resolve(),
            RefOr::Object(o) => o.clone(),
        }
    }
}
