use crate::codemodel::{PushFunction, TypeRef, function::Function};

#[derive(Debug)]
pub enum Implementation {
    InherentImpl {
        implementing_type: TypeRef,
        associated_functions: Vec<Function>,
    },
}

pub struct ImplementationBuilder {
    type_: TypeRef,
    associated_functions: Vec<Function>,
}

impl ImplementationBuilder {
    pub fn new_inherent(type_: TypeRef) -> Self {
        Self {
            type_,
            associated_functions: Vec::default(),
        }
    }

    pub fn build(self) -> Implementation {
        Implementation::InherentImpl {
            implementing_type: self.type_,
            associated_functions: self.associated_functions,
        }
    }
}

impl PushFunction for ImplementationBuilder {
    fn push_function(&mut self, function: Function) {
        self.associated_functions.push(function)
    }
}
