use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    error::Error,
    rc::Rc,
    str::FromStr,
};

mod fqtn;

pub trait Scope {
    fn build_struct(&mut self, name: &str) -> StructBuilder;
    fn find_type(&self, name: &str) -> Option<TypeRef>;
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
    pub fn type_u16(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U16))
    }
    pub fn type_u32(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U32))
    }
    pub fn type_u64(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U64))
    }
    pub fn type_i8(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::I8))
    }
    pub fn type_i16(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::I16))
    }
    pub fn type_i32(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::I32))
    }
    pub fn type_i64(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::I64))
    }
    pub fn type_f32(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::F32))
    }
    pub fn type_f64(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::F64))
    }
    pub fn type_bool(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::Bool))
    }
    pub fn type_string(&self) -> TypeRef {
        todo!("std::string::String requires crate and module scopes to be implemented")
    }
}

pub struct StructBuilder {
    scope: Rc<RefCell<ScopeImpl>>,

    name: String,
    fields: Vec<Field>,
}

#[derive(thiserror::Error, Debug)]
pub enum StructBuilderError {
    #[error("name missing")]
    NameMissing,
    #[error("a field with that name already exists")]
    DuplicateFieldName,
    #[error("a struct with that name already exists")]
    DuplicateStructName,
}

impl StructBuilder {
    pub fn new(scope: Rc<RefCell<ScopeImpl>>, name: &str) -> Self {
        StructBuilder {
            scope,
            name: name.to_string(),
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

    pub fn build(mut self) -> Result<TypeRef, StructBuilderError> {
        let name = self.name;

        let mut field_list = Vec::new();
        field_list.append(&mut self.fields);
        let s = Struct { name, field_list };

        match self.scope.borrow_mut().insert_struct(s) {
            Ok(s) => Ok(TypeRef::Struct(s)),
            Err(_) => Err(StructBuilderError::DuplicateStructName),
        }
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
    Bool,
}

#[derive(Clone)]
pub enum TypeRef {
    Struct(Rc<Struct>),
    Builtin(Rc<Builtin>),
    Reference(Rc<TypeRef>),
}

impl From<Rc<Struct>> for TypeRef {
    fn from(value: Rc<Struct>) -> Self {
        Self::Struct(value)
    }
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
    type_list: Vec<TypeRef>,
    type_map: HashMap<String, TypeRef>,
}

impl ScopeImpl {
    fn insert_struct(&mut self, s: Struct) -> Result<Rc<Struct>, CodeError> {
        if self.type_map.contains_key(s.name()) {
            return Err(CodeError::ItemAlreadyPresent);
        }

        let struct_ref = Rc::new(s);

        self.type_map
            .insert(struct_ref.name().to_string(), struct_ref.clone().into());
        self.type_list.push(struct_ref.clone().into());

        Ok(struct_ref.into())
    }
}

impl Scope for Rc<RefCell<ScopeImpl>> {
    fn build_struct(&mut self, name: &str) -> StructBuilder {
        StructBuilder::new(self.clone(), name)
    }

    fn find_type(&self, name: &str) -> Option<TypeRef> {
        match self.borrow().type_map.get(name) {
            Some(t) => Some(t.clone()),
            None => todo!(),
        }
    }
}

impl Scope for Codemodel {
    fn build_struct(&mut self, name: &str) -> StructBuilder {
        self.root_scope.build_struct(name)
    }

    fn find_type(&self, name: &str) -> Option<TypeRef> {
        self.root_scope.find_type(name)
    }
}

#[test]
fn test_buider() -> Result<(), anyhow::Error> {
    let mut cm = Codemodel::new();

    cm.build_struct("Test")
        .field("foo", cm.type_u8())
        .unwrap()
        .field("bar", cm.type_u8())
        .unwrap();

    Ok(())
}
