use std::borrow::Cow;

use crate::codemodel::{NamedItem, TypeRef};

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
    fn name(&self) -> Cow<str> {
        Cow::Borrowed(&self.name)
    }
}

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
}
