use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use openapiv3::{ParameterSchemaOrContent, ReferenceOr};

use crate::adapters::oas30::{
    MediaTypeSource, OAS3Resolver, OAS30Pointer, OAS30Source, OAS30Spec, OperationSource,
    PathItemSource, SchemaSource, SourceFromUri, into_oas30_content, into_ref_or,
};
use crate::types::{Parameter, ParameterLocation, RefOr};

pub fn to_parameters_iter(
    parent: &OAS30Pointer<impl OAS30Source>,
    oas30_parameters: &Vec<openapiv3::ReferenceOr<openapiv3::Parameter>>,
    parameter_source_factory: impl Fn(ParameterLocalId) -> ParameterSource,
) -> impl Iterator<Item = RefOr<OAS30Pointer<ParameterSource>>> {
    let mut params = Vec::new();
    for param_ref in oas30_parameters {
        let p = into_ref_or(param_ref, &parent, |_src| {
            let param = param_ref.as_item().unwrap();
            let param_id = ParameterLocalId {
                location: extract_location(&param),
                param_name: param.parameter_data_ref().name.clone(),
            };
            parameter_source_factory(param_id)
        });
        params.push(p);
    }
    params.into_iter()
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ParameterLocalId {
    param_name: String,
    location: ParameterLocation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterSource {
    Uri {
        uri: String,
    },
    Operation {
        source_ref: OperationSource,
        param_id: ParameterLocalId,
    },
    PathItem {
        source_ref: PathItemSource,
        param_id: ParameterLocalId,
    },
}

impl SourceFromUri for ParameterSource {
    fn from_uri(uri: &str) -> Self {
        ParameterSource::Uri {
            uri: uri.to_string(),
        }
    }
}
impl Hash for ParameterSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

fn extract_location(param: &openapiv3::Parameter) -> ParameterLocation {
    match param {
        openapiv3::Parameter::Query { .. } => ParameterLocation::Query,
        openapiv3::Parameter::Header { .. } => ParameterLocation::Header,
        openapiv3::Parameter::Path { .. } => ParameterLocation::Path,
        openapiv3::Parameter::Cookie { .. } => ParameterLocation::Cookie,
    }
}

impl ParameterSource {
    fn extract_param<'a>(
        params: &'a Vec<ReferenceOr<openapiv3::Parameter>>,
        param_id: &ParameterLocalId,
    ) -> &'a openapiv3::Parameter {
        params
            .iter()
            .find(|p| {
                let p = p.as_item().unwrap();
                let loc = extract_location(p);
                let pd = p.parameter_data_ref();
                pd.name == param_id.param_name && loc == param_id.location
            })
            .unwrap()
            .as_item()
            .unwrap()
    }
}

impl OAS30Source for ParameterSource {
    type OAS30Type = openapiv3::Parameter;

    fn inner<'a, 'b>(&'a self, openapi: &'b openapiv3::OpenAPI) -> &'b Self::OAS30Type
    where
        'a: 'b,
    {
        match self {
            ParameterSource::Uri { uri } => openapi.resolve_reference(uri).unwrap(),
            ParameterSource::Operation {
                source_ref,
                param_id,
            } => Self::extract_param(&source_ref.inner(openapi).parameters, param_id),
            ParameterSource::PathItem {
                source_ref,
                param_id,
            } => Self::extract_param(&source_ref.inner(openapi).parameters, param_id),
        }
    }
}

impl Parameter<OAS30Spec> for OAS30Pointer<ParameterSource> {
    fn in_(&self) -> ParameterLocation {
        extract_location(self.ref_source.inner(&self.openapi))
    }

    fn name(&self) -> &str {
        &self
            .ref_source
            .inner(&self.openapi)
            .parameter_data_ref()
            .name
    }

    fn schema(&self) -> Option<RefOr<OAS30Pointer<SchemaSource>>> {
        if let ParameterSchemaOrContent::Schema(schema_ref) =
            &self.inner().parameter_data_ref().format
        {
            Some(into_ref_or(schema_ref, self, |src| {
                SchemaSource::OperationParam(Box::new(src.clone()))
            }))
        } else {
            None
        }
    }

    fn content(&self) -> Option<HashMap<String, OAS30Pointer<MediaTypeSource>>> {
        match &self.inner().parameter_data_ref().format {
            ParameterSchemaOrContent::Schema(_reference_or) => None,
            ParameterSchemaOrContent::Content(index_map) => {
                Some(into_oas30_content(index_map, |content_index| {
                    OAS30Pointer {
                        openapi: self.openapi.clone(),
                        ref_source: MediaTypeSource::Parameter {
                            ref_source: self.ref_source.clone(),
                            content_index,
                        },
                    }
                }))
            }
        }
    }
}
