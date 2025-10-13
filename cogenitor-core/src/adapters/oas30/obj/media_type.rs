use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use indexmap::IndexMap;
use openapiv3::ParameterSchemaOrContent;

use crate::{
    adapters::oas30::{OAS30Pointer, OAS30Source, ResponseSource, into_ref_or},
    types::{MediaType, RefOr},
};

use super::{OAS30Spec, ParameterSource, RequestBodySource, SchemaSource};

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
                ParameterSchemaOrContent::Schema(_reference_or) => panic!(
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

pub fn into_oas30_content(
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
        self.inner().schema.as_ref().map(|m| {
            into_ref_or(m, &self, |src| {
                SchemaSource::MediaType(Box::new(src.clone()))
            })
        })
    }
}
