use super::OAS30Spec;
use crate::adapters::oas30::{OAS30Pointer, OAS30Source, SchemaSource, into_ref_or};
use crate::types::{Components, RefOr};
use std::fmt::Debug;
use std::hash::Hash;

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
