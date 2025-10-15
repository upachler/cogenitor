use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use openapiv3::ReferenceOr;

use super::super::{
    MediaTypeSource, OAS3Resolver, OAS30Pointer, OAS30Source, SourceFromUri, into_oas30_content,
};
use super::{OAS30Spec, OperationSource};
use crate::types::RequestBody;

impl OAS3Resolver<openapiv3::RequestBody> for openapiv3::OpenAPI {
    fn prefix(&self) -> &'static str {
        "#/components/requestBodies/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::RequestBody> {
        let ro = self.components.as_ref()?.request_bodies.get(reference)?;
        self.resolve(ro)
    }
}

impl SourceFromUri for RequestBodySource {
    fn from_uri(uri: &str) -> Self {
        RequestBodySource::Uri {
            uri: uri.to_string(),
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
