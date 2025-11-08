use std::borrow::Cow;

use crate::codemodel::{Attr, NamedItem, PushAttr};

use super::function::Function;

#[derive(Debug)]
pub struct Trait {
    name: String,
    associated_functions: Vec<Function>,
    attribute_list: Vec<Attr>,
}

impl NamedItem for Trait {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

impl Trait {
    pub fn function_iter(&self) -> impl Iterator<Item = &Function> {
        self.associated_functions.iter()
    }

    pub fn attr_iter(&self) -> impl Iterator<Item = &Attr> {
        self.attribute_list.iter()
    }
}

pub struct TraitBuilder {
    inner: Trait,
}

impl TraitBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            inner: Trait {
                name: name.to_string(),
                associated_functions: Vec::new(),
                attribute_list: Vec::new(),
            },
        }
    }

    pub fn function(mut self, function: Function) -> Self {
        self.inner.associated_functions.push(function);
        self
    }

    pub fn build(self) -> Result<Trait, crate::codemodel::CodeError> {
        Ok(self.inner)
    }
}

impl PushAttr for Trait {
    fn push_attr(&mut self, attr: Attr) {
        self.attribute_list.push(attr)
    }
}

impl PushAttr for TraitBuilder {
    fn push_attr(&mut self, attr: Attr) {
        self.inner.attribute_list.push(attr);
    }
}

impl crate::codemodel::PushFunction for TraitBuilder {
    fn push_function(&mut self, function: Function) {
        self.inner.associated_functions.push(function);
    }
}
