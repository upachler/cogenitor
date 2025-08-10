use std::borrow::Cow;

use crate::codemodel::{NamedItem, TypeRef};

#[derive(Debug)]
pub struct Function {
    name: String,
    function_params: Vec<FunctionParam>,
    return_type: TypeRef,
}

impl Function {
    pub fn function_params_iter(&self) -> impl Iterator<Item = &FunctionParam> {
        self.function_params.iter()
    }

    pub fn return_type(&self) -> &TypeRef {
        &self.return_type
    }
}

impl NamedItem for Function {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

#[derive(Debug)]
pub struct FunctionParam {
    pub name: String,
    pub type_: TypeRef,
}

pub struct FunctionBuilder {
    name: String,
    function_params: Vec<FunctionParam>,
    return_type: TypeRef,
}

impl FunctionBuilder {
    pub fn new(name: String, return_type: TypeRef) -> Self {
        Self {
            name,
            function_params: Default::default(),
            return_type,
        }
    }

    pub fn param(mut self, name: String, type_: TypeRef) -> Self {
        self.function_params.push(FunctionParam { name, type_ });
        self
    }

    pub fn build(self) -> Function {
        Function {
            name: self.name,
            function_params: self.function_params,
            return_type: self.return_type,
        }
    }

    pub fn param_names(&self) -> Vec<&str> {
        self.function_params
            .iter()
            .map(|p| p.name.as_ref())
            .collect::<Vec<&str>>()
    }
}
