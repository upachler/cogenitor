use crate::codemodel::{PushFunction, TraitRef, TypeRef, function::Function};

#[derive(Debug)]
pub struct Implementation {
    pub impl_trait: Option<TraitRef>,
    pub implementing_type: TypeRef,
    pub associated_functions: Vec<Function>,
}

pub struct ImplementationBuilder {
    type_: TypeRef,
    for_trait: Option<TraitRef>,
    associated_functions: Vec<Function>,
}

impl ImplementationBuilder {
    /// Create new inherent implementation for a given type
    pub fn new_inherent(type_: TypeRef) -> Self {
        Self {
            type_,
            for_trait: None,
            associated_functions: Vec::default(),
        }
    }

    /// Create new trait implementation for a given type
    pub fn new_trait(impl_trait: TraitRef, for_type: TypeRef) -> Self {
        Self {
            type_: for_type,
            for_trait: Some(impl_trait),
            associated_functions: Vec::default(),
        }
    }

    pub fn build(self) -> Implementation {
        Implementation {
            implementing_type: self.type_,
            impl_trait: self.for_trait,
            associated_functions: self.associated_functions,
        }
    }
}

impl PushFunction for ImplementationBuilder {
    fn push_function(&mut self, function: Function) {
        self.associated_functions.push(function)
    }
}
