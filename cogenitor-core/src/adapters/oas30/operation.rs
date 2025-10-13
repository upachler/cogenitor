use std::fmt::Debug;
use std::hash::Hash;

use http::Method;

use crate::adapters::oas30::{
    OAS30Pointer, OAS30Source, OAS30Spec, ParameterSource, PathItemSource, RequestBodySource,
    ResponseSource, into_ref_or, to_parameters_iter,
};
use crate::types::{Operation, RefOr, Spec, StatusSpec};

#[derive(Debug, Clone, Hash, PartialEq)]
pub struct OperationSource {
    pub(crate) path_item: PathItemSource,
    pub(crate) method: http::Method,
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
            into_ref_or(request_body, self, |src| RequestBodySource::Operation {
                source_ref: src.clone(),
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
                    into_ref_or(ro_response, self, |src| ResponseSource::Operation {
                        content_index,
                        ref_source: src.clone(),
                    }),
                )
            },
        )
    }
}
