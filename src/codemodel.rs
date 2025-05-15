use std::{cell::RefCell, collections::HashMap, rc::Rc, str::FromStr};

use regex::Regex;

mod fqtn;

pub trait Scope {
    fn build_struct(&mut self) -> StructBuilder;
}

pub struct Codemodel {
    root_scope: Rc<RefCell<ScopeImpl>>,
}

impl Codemodel {
    pub fn new() -> Self {
        Codemodel {
            root_scope: Rc::new(RefCell::new(ScopeImpl::default())),
        }
    }

    pub fn type_u8(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U8))
    }
}

pub struct StructBuilder {
    scope: Rc<RefCell<ScopeImpl>>,

    name: Option<String>,
    fields: Vec<Field>,
}

#[derive(Debug)]
pub enum StructBuilderError {
    NameMissing,
    DuplicateFieldName,
    DuplicateStructName,
}

impl StructBuilder {
    pub fn new(scope: Rc<RefCell<ScopeImpl>>) -> Self {
        StructBuilder {
            scope,
            name: None,
            fields: Vec::new(),
        }
    }

    /** Add new field with given name and type, referenced by name */
    pub fn field(
        &mut self,
        name: &str,
        type_ref: TypeRef,
    ) -> Result<&mut Self, StructBuilderError> {
        if self.fields.iter().any(|f| f.name.eq(name)) {
            return Err(StructBuilderError::DuplicateFieldName);
        }
        let field = Field {
            name: name.to_string(),
            type_ref,
        };
        self.fields.push(field);

        Ok(self)
    }

    pub fn build(mut self) -> Result<(), StructBuilderError> {
        let name = match self.name {
            Some(n) => n,
            None => return Err(StructBuilderError::NameMissing),
        };

        let mut field_list = Vec::new();
        field_list.append(&mut self.fields);
        let s = Struct { name, field_list };
        if let Err(_) = self.scope.borrow_mut().insert_struct(s) {
            return Err(StructBuilderError::DuplicateStructName);
        }
        Ok(())
    }
}

enum Builtin {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

pub enum TypeRef {
    Struct(Rc<Struct>),
    Builtin(Rc<Builtin>),
    Reference(Rc<TypeRef>),
}

struct Field {
    name: String,
    type_ref: TypeRef,
}

struct Struct {
    name: String,
    field_list: Vec<Field>,
}

impl Struct {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn field_iter(&self) -> impl Iterator {
        self.field_list.iter()
    }
}

pub enum CodeError {
    /** it was attempted to insert an element with a name that is already used */
    ItemAlreadyPresent,
}

#[derive(Default)]
struct ScopeImpl {
    struct_list: Vec<Rc<Struct>>,
    struct_map: HashMap<String, Rc<Struct>>,
}

impl ScopeImpl {
    fn insert_struct(&mut self, s: Struct) -> Result<(), CodeError> {
        if self.struct_map.contains_key(s.name()) {
            return Err(CodeError::ItemAlreadyPresent);
        }

        let struct_ref = Rc::new(s);

        self.struct_map
            .insert(struct_ref.name().to_string(), struct_ref.clone());
        self.struct_list.push(struct_ref);

        Ok(())
    }
}

impl Scope for Codemodel {
    fn build_struct(&mut self) -> StructBuilder {
        StructBuilder::new(self.root_scope.clone())
    }
}

#[test]
fn test_buider() -> Result<(), anyhow::Error> {
    let mut cm = Codemodel::new();

    cm.build_struct()
        .field("foo", cm.type_u8())
        .unwrap()
        .field("bar", cm.type_u8())
        .unwrap();

    Ok(())
}
