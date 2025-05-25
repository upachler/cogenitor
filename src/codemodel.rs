use std::{
    borrow::Cow, collections::HashMap, error::Error, fmt::Display, ops::Deref, rc::Rc, str::FromStr,
};

use fqtn::FQTN;
use lazy_static::lazy_static;

pub mod fqtn;

pub trait Scope {
    fn find_type(&self, name: &str) -> Option<TypeRef>;
    fn find_module(&self, name: &str) -> Option<ModuleRef>;
}

pub struct Codemodel {
    crate_namespace: Namespace<ModuleRef>,
}

lazy_static! {
    static ref STRING_TYPE_NAME: FQTN = FQTN::from_str("std::string::String").unwrap();
}

impl Codemodel {
    pub fn new() -> Self {
        let mut cm = Codemodel {
            crate_namespace: Namespace::default(),
        };

        Self::fill_std(&mut cm).unwrap();
        cm
    }

    fn fill_std(&mut self) -> Result<&mut Self, CodeError> {
        let mut std = Module::new("std");
        let mut string = Module::new("string");
        let string_struct = Struct {
            name: "String".to_string(),
            field_list: Default::default(),
        };
        string.insert_struct(string_struct)?;
        std.insert_module(string)?;
        self.insert_crate(std)?;
        Ok(self)
    }

    pub fn insert_crate(&mut self, crate_module: Module) -> Result<ModuleRef, CodeError> {
        self.crate_namespace.insert_item(crate_module.into())
    }
    pub fn find_type(&self, fqtn: &FQTN) -> Option<TypeRef> {
        let mut module = self.crate_namespace.find_item(fqtn.crate_name())?;
        for m in fqtn.module_iter() {
            module = module.find_module(m)?;
        }
        module.item.find_type(fqtn.type_name())
    }

    pub fn find_crate(&self, crate_name: &str) -> Option<ModuleRef> {
        self.crate_namespace.find_item(crate_name)
    }

    #[allow(unused)]
    pub fn type_u8(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U8))
    }
    #[allow(unused)]
    pub fn type_u16(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U16))
    }
    #[allow(unused)]
    pub fn type_u32(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U32))
    }
    #[allow(unused)]
    pub fn type_u64(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::U64))
    }
    #[allow(unused)]
    pub fn type_i8(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::I8))
    }
    #[allow(unused)]
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
        self.find_type(&STRING_TYPE_NAME).unwrap()
    }
}

pub struct StructBuilder {
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
    pub fn new(name: &str) -> Self {
        StructBuilder {
            name: name.to_string(),
            fields: Vec::new(),
        }
    }

    /** Add new field with given name and type, referenced by name */
    pub fn field(mut self, name: &str, type_ref: TypeRef) -> Result<Self, StructBuilderError> {
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

    pub fn build(mut self) -> Result<Struct, StructBuilderError> {
        let name = self.name;

        let mut field_list = Vec::new();
        field_list.append(&mut self.fields);
        Ok(Struct { name, field_list })
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

impl Builtin {
    fn name_ref(&self) -> &str {
        match self {
            Builtin::U8 => "u8",
            Builtin::U16 => "u16",
            Builtin::U32 => "u32",
            Builtin::U64 => "u64",
            Builtin::I8 => "i8",
            Builtin::I16 => "i16",
            Builtin::I32 => "i32",
            Builtin::I64 => "i64",
            Builtin::F32 => "f32",
            Builtin::F64 => "f64",
            Builtin::Bool => "bool",
        }
    }
}
impl NamedItem for Builtin {
    fn name(&self) -> Cow<str> {
        Cow::Borrowed(self.name_ref())
    }
}

#[derive(Clone)]
pub enum TypeRef {
    Struct(Rc<Struct>),
    Builtin(Rc<Builtin>),
    Alias(Rc<TypeRef>),
}

impl NamedItem for TypeRef {
    fn name(&self) -> Cow<str> {
        match self {
            TypeRef::Struct(s) => s.name(),
            TypeRef::Builtin(b) => b.name(),
            TypeRef::Alias(r) => r.name(),
        }
    }
}
impl From<Struct> for TypeRef {
    fn from(value: Struct) -> Self {
        Self::Struct(Rc::new(value))
    }
}

trait NamedItem {
    fn name(&self) -> Cow<str>;
}

struct Field {
    name: String,
    type_ref: TypeRef,
}

pub struct Struct {
    name: String,
    field_list: Vec<Field>,
}

impl Struct {
    pub fn field_iter(&self) -> impl Iterator {
        self.field_list.iter()
    }
}

impl NamedItem for Struct {
    fn name(&self) -> Cow<str> {
        Cow::Borrowed(&self.name)
    }
}

#[derive(Debug)]
pub enum CodeError {
    /** it was attempted to insert an element with a name that is already used */
    ItemAlreadyPresent,
}

impl Display for CodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeError::ItemAlreadyPresent => f.write_str("item already present"),
        }
    }
}

impl std::error::Error for CodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}

/**
A namespace stores items that each have a unique name
*/
struct Namespace<T> {
    item_list: Vec<T>,
    item_map: HashMap<String, T>,
}

impl<T: NamedItem + Clone> Namespace<T> {
    fn insert_item(&mut self, named_item: T) -> Result<T, CodeError> {
        if self.item_map.contains_key(named_item.name().as_ref()) {
            return Err(CodeError::ItemAlreadyPresent);
        }

        self.item_map
            .insert(named_item.name().to_string(), named_item.clone().into());
        self.item_list.push(named_item.clone().into());

        Ok(named_item)
    }

    fn find_item(&self, name: &str) -> Option<T> {
        self.item_map.get(name).map(Clone::clone)
    }

    fn contains_item(&self, name: &str) -> bool {
        self.item_map.contains_key(name)
    }
}

impl<T> Default for Namespace<T> {
    fn default() -> Self {
        Self {
            item_list: Vec::default(),
            item_map: HashMap::default(),
        }
    }
}

pub struct Module {
    name: String,
    type_namespace: Namespace<TypeRef>,
    module_namespace: Namespace<ModuleRef>,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.name;
        f.write_fmt(format_args!("Module {name}"))
    }
}

/**
Wrapper around Rc<T> for named code model items like modules and crates
*/
#[derive(Debug)]
pub struct CodeModelRef<T: NamedItem> {
    item: Rc<T>,
}
impl<T: NamedItem> PartialEq for CodeModelRef<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.item, &other.item)
    }
}
impl<T: NamedItem> Clone for CodeModelRef<T> {
    fn clone(&self) -> Self {
        Self {
            item: self.item.clone(),
        }
    }
}

impl<T: NamedItem> From<T> for CodeModelRef<T> {
    fn from(value: T) -> Self {
        Self {
            item: Rc::new(value),
        }
    }
}

impl<T: NamedItem> Deref for CodeModelRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.deref()
    }
}

impl<T: NamedItem> NamedItem for CodeModelRef<T> {
    fn name(&self) -> Cow<str> {
        // we need Cow::Owned here because .borrow() returns a temporary value,
        // so the reference to the item is only valid within the scope of this
        // method
        Cow::Owned(self.item.name().to_string())
    }
}

type ModuleRef = CodeModelRef<Module>;

impl Module {
    pub fn new(name: &str) -> Self {
        Module {
            name: name.to_string(),
            module_namespace: Default::default(),
            type_namespace: Default::default(),
        }
    }
    pub fn insert_struct(&mut self, s: Struct) -> Result<TypeRef, CodeError> {
        let struct_ref = TypeRef::from(s);
        self.type_namespace.insert_item(struct_ref.clone())?;
        Ok(struct_ref)
    }
    fn insert_module(&mut self, m: Module) -> Result<ModuleRef, CodeError> {
        let m: ModuleRef = m.into();
        self.module_namespace.insert_item(m.clone())?;
        Ok(m)
    }
}

impl NamedItem for Module {
    fn name(&self) -> Cow<str> {
        Cow::Borrowed(&self.name)
    }
}

impl Scope for Module {
    fn find_type(&self, name: &str) -> Option<TypeRef> {
        self.type_namespace.find_item(name)
    }

    fn find_module(&self, name: &str) -> Option<ModuleRef> {
        self.module_namespace.find_item(name)
    }
}

#[test]
fn test_crates_and_mods() -> Result<(), anyhow::Error> {
    let mut cm = Codemodel::new();

    let c = Module::new("crate");
    let crate_ref = cm.insert_crate(c)?;
    assert_eq!(
        crate_ref,
        cm.find_crate("crate").expect("'crate' not found")
    );

    Ok(())
}

#[test]
fn test_buider() -> Result<(), anyhow::Error> {
    let mut cm = Codemodel::new();

    let s = StructBuilder::new("Test")
        .field("foo", cm.type_u8())
        .unwrap()
        .field(
            "bar",
            cm.find_type(&FQTN::from_str("std::string::String").unwrap())
                .unwrap(),
        )
        .unwrap()
        .build()?;
    let mut m = Module::new("crate");
    m.insert_struct(s)?;
    cm.insert_crate(m)?;

    let type_test = cm
        .find_type(&FQTN::from_str("crate::Test").unwrap())
        .expect("Type not found");
    match type_test {
        TypeRef::Struct(s) => assert_eq!(s.field_iter().count(), 2),
        _ => panic!("unexpected type variant"),
    }

    Ok(())
}
