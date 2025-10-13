use std::io::BufReader;
use std::rc::Rc;
use std::str::FromStr;

use openapiv3::{OpenAPI, ReferenceOr};

use crate::adapters::oas30::{
    ComponentsSource, MediaTypeSource, OAS30Pointer, OperationSource, ParameterSource,
    PathItemSource, RequestBodySource, ResponseSource, SchemaSource,
};
use crate::types::{Components, RefOr};

pub struct OAS30Spec {
    openapi: Rc<OpenAPI>,
}

impl FromStr for OAS30Spec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        let openapi: OpenAPI = serde_yaml::from_str(s)?;
        Ok(openapi.into())
    }
}

impl From<OpenAPI> for OAS30Spec {
    fn from(openapi: OpenAPI) -> Self {
        OAS30Spec {
            openapi: Rc::new(openapi),
        }
    }
}

impl crate::Spec for OAS30Spec {
    type Schema = OAS30Pointer<SchemaSource>;
    type Components = OAS30Pointer<ComponentsSource>;
    type PathItem = OAS30Pointer<PathItemSource>;
    type Parameter = OAS30Pointer<ParameterSource>;
    type MediaType = OAS30Pointer<MediaTypeSource>;
    type Operation = OAS30Pointer<OperationSource>;
    type RequestBody = OAS30Pointer<RequestBodySource>;
    type Response = OAS30Pointer<ResponseSource>;

    fn from_reader(r: impl std::io::Read) -> anyhow::Result<impl crate::Spec> {
        let r = BufReader::new(r);
        let openapi: OpenAPI = serde_yaml::from_reader(r)?;
        Ok(OAS30Spec::from(openapi))
    }

    fn schemata_iter(&self) -> impl Iterator<Item = (String, RefOr<Self::Schema>)> {
        self.components()
            .iter()
            .flat_map(|c| c.schemas())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn paths(&self) -> impl Iterator<Item = (String, Self::PathItem)> {
        let paths: Vec<String> = self
            .openapi
            .paths
            .paths
            .iter()
            .filter_map(|(path, path_item_ref)| {
                if let ReferenceOr::Item(_path_item) = path_item_ref {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        PathIterator {
            paths,
            current: 0,
            openapi: self.openapi.clone(),
        }
    }

    fn components(&self) -> Option<OAS30Pointer<ComponentsSource>> {
        self.openapi.components.as_ref().map(|_| OAS30Pointer {
            openapi: self.openapi.clone(),
            ref_source: ComponentsSource {},
        })
    }
}

// Path Iterator Implementation
struct PathIterator {
    paths: Vec<String>,
    current: usize,
    openapi: Rc<OpenAPI>,
}

impl Iterator for PathIterator {
    type Item = (String, OAS30Pointer<PathItemSource>);

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.paths.get(self.current);
        if let Some(path) = path {
            self.current += 1;
            return Some((
                path.clone(),
                OAS30Pointer {
                    ref_source: PathItemSource { path: path.clone() },
                    openapi: self.openapi.clone(),
                },
            ));
        }
        None
    }
}
