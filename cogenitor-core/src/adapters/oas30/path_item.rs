use std::fmt::Debug;
use std::hash::Hash;

use http::Method;

use crate::adapters::oas30::{
    OAS3Resolver, OAS30Pointer, OAS30Source, OAS30Spec, OperationSource, ParameterSource,
    to_parameters_iter,
};
use crate::types::{PathItem, RefOr};

// OAS30 PathItem Implementation
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct PathItemSource {
    pub(crate) path: String,
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

impl PathItem<OAS30Spec> for OAS30Pointer<PathItemSource> {
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
