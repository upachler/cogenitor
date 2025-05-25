use std::{collections::HashMap, io, str::FromStr};

use json::JsonValue;

/** An implementation of an OAS spec, specific to our needs for code generation */
pub trait Spec: FromStr<Err = anyhow::Error> {
    type Schema: Schema;

    fn from_reader(r: impl io::Read) -> anyhow::Result<impl Spec>;

    fn schemata_iter(&self) -> impl Iterator<Item = (String, Self::Schema)>;
}

/**
representation of possible values of the [type](https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-5.21)
keyword.
*/
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
pub trait Schema: Clone + std::fmt::Debug {
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
}
