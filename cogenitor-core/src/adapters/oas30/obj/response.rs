use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use crate::{
    adapters::oas30::{
        MediaTypeSource, OAS3Resolver, OAS30Pointer, OAS30Source, OAS30Spec, OperationSource,
        SourceFromUri, into_oas30_content,
    },
    types::{Response, Spec},
};

impl OAS3Resolver<openapiv3::Response> for openapiv3::OpenAPI {
    fn prefix(&self) -> &'static str {
        "#/components/responses/"
    }

    fn resolve_reference(&self, reference: &str) -> Option<&openapiv3::Response> {
        let ro = self.components.as_ref()?.responses.get(reference)?;
        self.resolve(ro)
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
