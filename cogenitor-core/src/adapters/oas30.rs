mod obj;

#[cfg(test)]
mod test;

use std::fmt::Debug;
use std::hash::Hash;
use std::str::FromStr;
use std::{borrow::Borrow, rc::Rc};

use openapiv3::{OpenAPI, ReferenceOr};

use crate::types::{ByReference, RefOr, Reference, StatusSpec};

pub use obj::*;

/// provides means to resolve `openapiv3` OAS objects from URI or `ReferenceOr<T>` instances.
/// For each
trait OAS3Resolver<T> {
    /// Resolve the `openapiv3` object of type `T` in `ReferenceOr<T>` to `&T` for both cases:
    /// * If `ReferenceOr<T>` is an actual schema object `T`, the reference `&T` is returned, wrapped in `Some`.
    /// * Otherwise `ReferenceOr<T>` is a reference (e.g. `#/components/schemas/Pet`). This reference
    ///   is resolved by calling `Self::resolve_reference(uri)`. If that reference proves unresolveable,
    ///   `None` is returned.
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

    /// Yield base URI for the particular `T` we allow to resolve here
    fn prefix(&self) -> &'static str;

    /// Attempt to resolve the specified URI reference into an actual
    /// reference to `T`
    fn resolve_reference(&self, reference: &str) -> Option<&T>;
}

/// An abstract source to the `openapiv3` schema object type specfied in the `OAS30Type`.
/// Typically, this trait is implemented by an enum with variants for each
/// position in a OAS 3.0 spec document structure where a particular
/// schema object may occur. For instance, a `openapiv3::Schema` may occur
/// in `#/components/schemas`, below a `MediaType` object, or inside the
/// properties of another `Schema`, among other places. The enum lists all such
/// places, while the variants may store additional information, like the name of the
/// name of the schema located below `#/components/schemas/`, or the name of the
/// property and the source to the parent schema.
///
/// Access to the actual schema object instance is provided via the `inner()` method.
pub trait OAS30Source: std::fmt::Debug + Hash + PartialEq {
    type OAS30Type;
    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b;
}

#[derive(Clone)]
pub struct OAS30Pointer<S: OAS30Source> {
    openapi: Rc<OpenAPI>,
    ref_source: S,
}

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

impl TryFrom<&openapiv3::StatusCode> for StatusSpec {
    type Error = <StatusSpec as FromStr>::Err;
    fn try_from(s: &openapiv3::StatusCode) -> Result<Self, Self::Error> {
        let s = s.to_string();
        StatusSpec::from_str(&s)
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
