use std::{
    borrow::Cow, cell::RefCell, collections::HashMap, error::Error, fmt::Display, ops::Deref,
    rc::Rc, str::FromStr,
};

use fqtn::FQTN;
use lazy_static::lazy_static;
use proc_macro2::TokenStream;

use crate::codemodel::{implementation::Implementation, simplepath::SimplePath};

pub mod fqtn;
pub mod function;
pub mod implementation;
pub mod simplepath;

pub trait Scope {
    fn find_type(&self, name: &str) -> Option<TypeRef>;
    fn find_module(&self, name: &str) -> Option<ModuleRef>;
}

pub struct Codemodel {
    crate_namespace: Namespace<ModuleRef>,
}

lazy_static! {
    static ref STRING_TYPE_NAME: FQTN = FQTN::from_str("std::string::String").unwrap();
    static ref VEC_TYPE_NAME: FQTN = FQTN::from_str("std::vec::Vec").unwrap();
    static ref RESULT_TYPE_NAME: FQTN = FQTN::from_str("std::result::Result").unwrap();
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
        let string_struct = StructBuilder::new("String").build().unwrap();
        string.insert_struct(string_struct)?;
        std.insert_module(string)?;

        let mut vec = Module::new("vec");
        let vec_struct = StructBuilder::new("Vec").build().unwrap();
        vec.insert_struct(vec_struct)?;
        std.insert_module(vec)?;

        let mut result = Module::new("result");
        let result_struct = StructBuilder::new("Result").build().unwrap();
        result.insert_struct(result_struct)?;
        std.insert_module(result)?;

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

    /** Create a reference to an _instance_ if a generic type (e.g. Vec<u8>) */
    pub fn type_instance(&mut self, generic_type: &TypeRef, type_params: &[TypeRef]) -> TypeRef {
        TypeRef::GenericInstance {
            generic_type: Box::new(generic_type.clone()),
            type_parameter: type_params.into(),
        }
    }

    pub fn type_unit(&self) -> TypeRef {
        TypeRef::Builtin(Rc::new(Builtin::Unit))
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

    pub fn type_vec(&self) -> TypeRef {
        self.find_type(&VEC_TYPE_NAME).unwrap()
    }

    pub fn type_result(&self) -> TypeRef {
        self.find_type(&RESULT_TYPE_NAME).unwrap()
    }

    pub fn type_self(&self) -> TypeRef {
        TypeRef::SelfType
    }

    pub fn type_ref_self(&self) -> TypeRef {
        TypeRef::Reference {
            referenced_type: self.type_self().into(),
            mutable: false,
            lifetime: None,
        }
    }

    pub fn type_ref_mut_self(&self) -> TypeRef {
        TypeRef::Reference {
            referenced_type: self.type_self().into(),
            mutable: true,
            lifetime: None,
        }
    }
}

pub enum FieldListBuilderError {
    DuplicateFieldName,
}

impl From<FieldListBuilderError> for StructBuilderError {
    fn from(value: FieldListBuilderError) -> Self {
        match value {
            FieldListBuilderError::DuplicateFieldName => StructBuilderError::DuplicateFieldName,
        }
    }
}

impl From<FieldListBuilderError> for EnumBuilderError {
    fn from(value: FieldListBuilderError) -> Self {
        match value {
            FieldListBuilderError::DuplicateFieldName => EnumBuilderError::DuplicateFieldName,
        }
    }
}

/// Builder for constructing field lists with duplicate field name checking.
///
/// This is a shared utility used internally by both `StructBuilder` and
/// `EnumBuilder` to provide consistent field building behavior
/// and eliminate code duplication.
#[derive(Debug)]
pub struct FieldListBuilder {
    fields: Vec<Field>,
}

/// Builder for constructing attribute lists with duplicate attribute name checking.
///
/// This is a shared utility used internally by both `StructBuilder` and
/// `EnumBuilder` to provide consistent attribute building behavior
/// and eliminate code duplication.
#[derive(Debug)]
pub struct AttrListBuilder {
    attrs: Vec<Attr>,
}

impl FieldListBuilder {
    fn new() -> Self {
        FieldListBuilder { fields: Vec::new() }
    }

    pub fn field(self, name: &str, type_ref: TypeRef) -> Result<Self, FieldListBuilderError> {
        self.field_impl(name, TypeRefOrTokenStream::TypeRef(type_ref))
    }

    pub fn field_with_input(
        self,
        name: &str,
        input: TokenStream,
    ) -> Result<Self, FieldListBuilderError> {
        self.field_impl(name, TypeRefOrTokenStream::TokenStream(input))
    }

    fn field_impl(
        mut self,
        name: &str,
        t_or_ts: TypeRefOrTokenStream,
    ) -> Result<Self, FieldListBuilderError> {
        if self.fields.iter().any(|f| f.name.eq(name)) {
            return Err(FieldListBuilderError::DuplicateFieldName);
        }
        let field = Field {
            name: name.to_string(),
            type_ref_or_ts: t_or_ts,
        };
        self.fields.push(field);
        Ok(self)
    }

    pub fn build(self) -> Vec<Field> {
        self.fields
    }
}

enum AttrListBuilderError {
    AttrPathInvalid,
}

impl From<AttrListBuilderError> for StructBuilderError {
    fn from(value: AttrListBuilderError) -> Self {
        match value {
            AttrListBuilderError::AttrPathInvalid => Self::AttrPathInvalid,
        }
    }
}

impl From<AttrListBuilderError> for EnumBuilderError {
    fn from(value: AttrListBuilderError) -> Self {
        match value {
            AttrListBuilderError::AttrPathInvalid => Self::AttrPathInvalid,
        }
    }
}

impl AttrListBuilder {
    fn new() -> Self {
        AttrListBuilder { attrs: Vec::new() }
    }

    fn attr(self, item_path: &str) -> Result<Self, AttrListBuilderError> {
        self.attr_with_input(item_path, TokenStream::new())
    }

    fn attr_with_input(
        mut self,
        item_path: &str,
        input: TokenStream,
    ) -> Result<Self, AttrListBuilderError> {
        self.attrs.push(Attr {
            path: SimplePath::new(item_path).map_err(|_| AttrListBuilderError::AttrPathInvalid)?,
            input,
        });
        Ok(self)
    }

    pub fn build(self) -> Vec<Attr> {
        self.attrs
    }
}

#[derive(Debug)]
pub struct StructBuilder {
    name: String,
    field_builder: FieldListBuilder,
    attr_builder: AttrListBuilder,
}

#[derive(thiserror::Error, Debug)]
pub enum StructBuilderError {
    #[error("a field with that name already exists")]
    DuplicateFieldName,
    #[error("the attribute item path specified is invalid")]
    AttrPathInvalid,
}

#[derive(Debug)]
pub(crate) enum TypeRefOrTokenStream {
    TypeRef(TypeRef),
    TokenStream(TokenStream),
}
impl TypeRefOrTokenStream {
    pub(crate) fn unwrap_type_ref(&self) -> &TypeRef {
        if let TypeRefOrTokenStream::TypeRef(type_ref) = self {
            type_ref
        } else {
            panic!("expected TypeRef variant, encountered {self:?}")
        }
    }
    pub(crate) fn unwrap_token_stream(&self) -> &TokenStream {
        if let TypeRefOrTokenStream::TokenStream(token_stream) = self {
            token_stream
        } else {
            panic!("expected TokenStream variant, encountered {self:?}")
        }
    }
}

/// Represents the data associated with an enum variant
#[derive(Debug)]
pub enum EnumVariantData {
    /// Unit variant (e.g., `Red`)
    Unit,
    /// Tuple variant (e.g., `Color(u8, u8, u8)`)
    Tuple(Vec<TypeRefOrTokenStream>),
    /// Struct variant (e.g., `Point { x: i32, y: i32 }`)
    Struct(Vec<Field>),
}

/// Represents a single variant of a Rust enum
#[derive(Debug)]
pub struct EnumVariant {
    name: String,
    data: EnumVariantData,
}

impl EnumVariant {
    pub fn data(&self) -> &EnumVariantData {
        &self.data
    }
}

impl NamedItem for EnumVariant {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

/// Represents a Rust enum with its variants
#[derive(Debug)]
pub struct Enum {
    name: String,
    variant_list: Vec<EnumVariant>,
    attribute_list: Vec<Attr>,
}

impl Enum {
    pub fn variant_iter(&self) -> impl Iterator<Item = &EnumVariant> {
        self.variant_list.iter()
    }

    pub fn attr_iter(&self) -> impl Iterator<Item = &Attr> {
        self.attribute_list.iter()
    }
}

impl NamedItem for Enum {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

/// Builder for constructing `Enum` instances
#[derive(Debug)]
pub struct EnumBuilder {
    name: String,
    variants: Vec<EnumVariant>,
    attr_builder: AttrListBuilder,
}

#[derive(thiserror::Error, Debug)]
pub enum EnumBuilderError {
    #[error("a variant with that name already exists")]
    DuplicateVariantName,
    #[error("a field with that name already exists")]
    DuplicateFieldName,
    #[error("the attribute item path specified is invalid")]
    AttrPathInvalid,
}

impl StructBuilder {
    pub fn new(name: &str) -> Self {
        StructBuilder {
            name: name.to_string(),
            field_builder: FieldListBuilder::new(),
            attr_builder: AttrListBuilder::new(),
        }
    }

    /** Add new field with given name and type, referenced by name */
    pub fn field(mut self, name: &str, type_ref: TypeRef) -> Result<Self, StructBuilderError> {
        self.field_builder = self.field_builder.field(name, type_ref)?;
        Ok(self)
    }

    pub fn field_with_input(
        mut self,
        name: &str,
        input: TokenStream,
    ) -> Result<Self, StructBuilderError> {
        self.field_builder = self.field_builder.field_with_input(name, input)?;
        Ok(self)
    }
    pub fn attr(mut self, name: &str) -> Result<Self, StructBuilderError> {
        self.attr_builder = self.attr_builder.attr(name)?;
        Ok(self)
    }

    pub fn attr_with_input(
        mut self,
        name: &str,
        input: TokenStream,
    ) -> Result<Self, StructBuilderError> {
        self.attr_builder = self.attr_builder.attr_with_input(name, input)?;
        Ok(self)
    }

    pub fn build(self) -> Result<Struct, StructBuilderError> {
        Ok(Struct {
            name: self.name,
            attribute_list: self.attr_builder.build(),
            field_list: self.field_builder.build(),
        })
    }
}

impl EnumBuilder {
    pub fn new(name: &str) -> Self {
        EnumBuilder {
            name: name.to_string(),
            variants: Vec::new(),
            attr_builder: AttrListBuilder::new(),
        }
    }

    pub fn unit_variant(mut self, name: &str) -> Result<Self, EnumBuilderError> {
        if self.variants.iter().any(|v| v.name.eq(name)) {
            return Err(EnumBuilderError::DuplicateVariantName);
        }
        let variant = EnumVariant {
            name: name.to_string(),
            data: EnumVariantData::Unit,
        };
        self.variants.push(variant);
        Ok(self)
    }

    pub fn tuple_variant(self, name: &str, types: Vec<TypeRef>) -> Result<Self, EnumBuilderError> {
        self.tuple_variant_impl(
            name,
            types
                .iter()
                .map(|t| TypeRefOrTokenStream::TypeRef(t.clone()))
                .collect(),
        )
    }

    pub fn tuple_variant_with_input(
        self,
        name: &str,
        types: Vec<TokenStream>,
    ) -> Result<Self, EnumBuilderError> {
        self.tuple_variant_impl(
            name,
            types
                .iter()
                .map(|t| TypeRefOrTokenStream::TokenStream(t.clone()))
                .collect(),
        )
    }

    fn tuple_variant_impl(
        mut self,
        name: &str,
        types: Vec<TypeRefOrTokenStream>,
    ) -> Result<Self, EnumBuilderError> {
        if self.variants.iter().any(|v| v.name.eq(name)) {
            return Err(EnumBuilderError::DuplicateVariantName);
        }
        let variant = EnumVariant {
            name: name.to_string(),
            data: EnumVariantData::Tuple(types),
        };
        self.variants.push(variant);
        Ok(self)
    }

    pub fn struct_variant<F>(
        mut self,
        name: &str,
        field_builder: F,
    ) -> Result<Self, EnumBuilderError>
    where
        F: FnOnce(FieldListBuilder) -> Result<FieldListBuilder, FieldListBuilderError>,
    {
        if self.variants.iter().any(|v| v.name.eq(name)) {
            return Err(EnumBuilderError::DuplicateVariantName);
        }

        let builder = FieldListBuilder::new();

        let completed_builder = field_builder(builder)?;
        let variant = EnumVariant {
            name: name.to_string(),
            data: EnumVariantData::Struct(completed_builder.build()),
        };
        self.variants.push(variant);
        Ok(self)
    }

    pub fn attr(mut self, name: &str) -> Result<Self, EnumBuilderError> {
        self.attr_builder = self.attr_builder.attr(name)?;
        Ok(self)
    }

    pub fn attr_with_input(
        mut self,
        name: &str,
        input: TokenStream,
    ) -> Result<Self, EnumBuilderError> {
        self.attr_builder = self.attr_builder.attr_with_input(name, input)?;
        Ok(self)
    }

    pub fn build(self) -> Result<Enum, EnumBuilderError> {
        Ok(Enum {
            name: self.name,
            variant_list: self.variants,
            attribute_list: self.attr_builder.build(),
        })
    }
}

#[derive(Debug)]
pub enum Builtin {
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
    Unit,
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
            Builtin::Unit => "()",
        }
    }
}
impl NamedItem for Builtin {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(self.name_ref())
    }
}

#[derive(Clone, Debug)]
pub enum Indirection {
    // an indirection stub. This is used to indicate that the indirections's
    // reference hasn't been set yet.
    Stub(String),
    Resolved(TypeRef),
}

#[derive(Clone, Debug)]
pub struct Alias {
    name: String,
    target: TypeRef,
}

impl Alias {
    pub fn target(&self) -> &TypeRef {
        &self.target
    }
}

impl NamedItem for Alias {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        (&self.name).into()
    }
}

#[derive(Clone, Debug)]
pub enum TypeRef {
    /// type name must be looked up via CodeModel
    Indirection(Rc<RefCell<Indirection>>),
    Struct(Rc<Struct>),
    Enum(Rc<Enum>),
    Builtin(Rc<Builtin>),
    Alias(Rc<Alias>),
    GenericInstance {
        generic_type: Box<TypeRef>,
        type_parameter: Vec<TypeRef>,
    },
    /// actual `Self`, the reference
    SelfType,
    /// a reference `&` to a type `T` - which may be mutable
    /// and/or have an attached lifetime specifier.
    /// E.g. for `&'a mut u32`
    /// `referenced_type` refers to `u32`'s `TypeRef`, `mutable` is `true`
    /// and `lifetime` is `Some("a".to_string())`
    Reference {
        referenced_type: Box<TypeRef>,
        mutable: bool,
        lifetime: Option<String>,
    },
}

impl PartialEq for TypeRef {
    fn eq(&self, other: &Self) -> bool {
        use TypeRef::*;
        match (self, other) {
            (Indirection(lhs), Indirection(rhs)) => Rc::ptr_eq(lhs, rhs),
            (Struct(lhs), Struct(rhs)) => Rc::ptr_eq(lhs, rhs),
            (Enum(lhs), Enum(rhs)) => Rc::ptr_eq(lhs, rhs),
            (Builtin(lhs), Builtin(rhs)) => Rc::ptr_eq(lhs, rhs),
            (Alias(lhs), Alias(rhs)) => Rc::ptr_eq(lhs, rhs),
            (
                GenericInstance {
                    generic_type: lhs_generic_type,
                    type_parameter: lhs_type_parameter,
                },
                GenericInstance {
                    generic_type: rhs_generic_type,
                    type_parameter: rhs_type_parameter,
                },
            ) => lhs_generic_type == rhs_generic_type && lhs_type_parameter == rhs_type_parameter,
            (SelfType, SelfType) => true,
            (
                Reference {
                    referenced_type: lhs_referenced_type,
                    mutable: lhs_mutable,
                    lifetime: lhs_lifetime,
                },
                Reference {
                    referenced_type: rhs_referenced_type,
                    mutable: rhs_mutable,
                    lifetime: rhs_lifetime,
                },
            ) => {
                lhs_referenced_type == rhs_referenced_type
                    && lhs_mutable == rhs_mutable
                    && lhs_lifetime == rhs_lifetime
            }
            _ => false,
        }
    }
}

impl NamedItem for TypeRef {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        match self {
            TypeRef::Indirection(i) => match i.borrow().deref() {
                Indirection::Stub(name) => Cow::Owned(name.to_string()),
                Indirection::Resolved(type_ref) => Cow::Owned(type_ref.name().to_string()),
            },
            TypeRef::Struct(s) => s.name(),
            TypeRef::Enum(e) => e.name(),
            TypeRef::Builtin(b) => b.name(),
            TypeRef::Alias(r) => r.name(),
            TypeRef::GenericInstance {
                generic_type,
                type_parameter,
            } => {
                let generic_type = generic_type.name();
                let param_list = type_parameter
                    .iter()
                    .map(|p| p.name().to_string())
                    .collect::<Vec<String>>()
                    .join(",");
                Cow::Owned(format!("{generic_type}<{param_list}>"))
            }
            TypeRef::SelfType => Cow::Borrowed("Self"),
            TypeRef::Reference {
                referenced_type,
                mutable,
                lifetime,
            } => {
                let type_name = referenced_type.name();
                let mutable = if *mutable { "mut " } else { "" };
                let lifetime = lifetime
                    .as_ref()
                    .map(|lt| format!("'{lt} "))
                    .unwrap_or("".to_string());
                Cow::Owned(format!("&{lifetime}{mutable}{type_name}"))
            }
        }
    }
}
impl From<Struct> for TypeRef {
    fn from(value: Struct) -> Self {
        Self::Struct(Rc::new(value))
    }
}

impl From<Enum> for TypeRef {
    fn from(value: Enum) -> Self {
        Self::Enum(Rc::new(value))
    }
}

pub trait NamedItem {
    fn name<'a>(&'a self) -> Cow<'a, str>;
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub type_ref_or_ts: TypeRefOrTokenStream,
}

impl Field {
    pub fn type_(&self) -> &TypeRefOrTokenStream {
        &self.type_ref_or_ts
    }
}
impl NamedItem for Field {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

#[derive(Debug)]
pub struct Struct {
    attribute_list: Vec<Attr>,
    name: String,
    field_list: Vec<Field>,
}

impl Struct {
    pub fn field_iter(&self) -> impl Iterator<Item = &Field> {
        self.field_list.iter()
    }

    pub fn attr_iter(&self) -> impl Iterator<Item = &Attr> {
        self.attribute_list.iter()
    }
}

impl NamedItem for Struct {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

#[derive(Debug)]
pub struct Attr {
    path: SimplePath,
    input: TokenStream,
}

impl Attr {
    pub fn path(&self) -> &SimplePath {
        &self.path
    }

    pub fn input(&self) -> &TokenStream {
        &self.input
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
    implementations: Vec<Implementation>,
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
    fn name<'a>(&'a self) -> Cow<'a, str> {
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
            implementations: Vec::new(),
        }
    }

    pub fn type_iter(&self) -> impl Iterator<Item = &TypeRef> {
        self.type_namespace.item_list.iter()
    }

    pub fn mod_iter(&self) -> impl Iterator<Item = &ModuleRef> {
        self.module_namespace.item_list.iter()
    }

    pub fn insert_implementation(&mut self, i: Implementation) -> Result<(), CodeError> {
        self.implementations.push(i);
        Ok(())
    }

    pub fn implementations_iter(&self) -> impl Iterator<Item = &Implementation> {
        self.implementations.iter()
    }

    pub fn insert_struct(&mut self, s: Struct) -> Result<TypeRef, CodeError> {
        let struct_ref = TypeRef::from(s);
        if let Some(TypeRef::Indirection(i)) =
            self.type_namespace.find_item(struct_ref.name().as_ref())
        {
            // if the name collides with a stub, we simply replace that stub,
            // otherwise it's a proper name collision
            let is_stub = match i.borrow().deref() {
                Indirection::Stub(_) => true,
                _ => false,
            };
            if is_stub {
                let replacement = Indirection::Resolved(struct_ref.clone());
                i.replace(replacement);
            } else {
                return Err(CodeError::ItemAlreadyPresent);
            }
        } else {
            self.type_namespace.insert_item(struct_ref.clone())?;
        }
        Ok(struct_ref)
    }

    pub fn insert_enum(&mut self, e: Enum) -> Result<TypeRef, CodeError> {
        let enum_ref = TypeRef::from(e);
        if let Some(TypeRef::Indirection(i)) =
            self.type_namespace.find_item(enum_ref.name().as_ref())
        {
            // if the name collides with a stub, we simply replace that stub,
            // otherwise it's a proper name collision
            let is_stub = match i.borrow().deref() {
                Indirection::Stub(_) => true,
                _ => false,
            };
            if is_stub {
                let replacement = Indirection::Resolved(enum_ref.clone());
                i.replace(replacement);
            } else {
                return Err(CodeError::ItemAlreadyPresent);
            }
        } else {
            self.type_namespace.insert_item(enum_ref.clone())?;
        }
        Ok(enum_ref)
    }

    pub fn insert_type_stub(&mut self, name: &str) -> Result<TypeRef, CodeError> {
        self.type_namespace
            .insert_item(TypeRef::Indirection(Rc::new(RefCell::new(
                Indirection::Stub(name.to_string()),
            ))))
    }

    pub fn insert_type_alias(&mut self, name: &str, target: TypeRef) -> Result<TypeRef, CodeError> {
        self.type_namespace
            .insert_item(TypeRef::Alias(Rc::new(Alias {
                name: name.to_string(),
                target,
            })))
    }

    fn insert_module(&mut self, m: Module) -> Result<ModuleRef, CodeError> {
        let m: ModuleRef = m.into();
        self.module_namespace.insert_item(m.clone())?;
        Ok(m)
    }
}

impl NamedItem for Module {
    fn name<'a>(&'a self) -> Cow<'a, str> {
        Cow::Borrowed(&self.name)
    }
}

impl Scope for Module {
    fn find_type(&self, name: &str) -> Option<TypeRef> {
        match self.type_namespace.find_item(name) {
            Some(target) => match target {
                TypeRef::Indirection(i) => match i.borrow().deref() {
                    Indirection::Stub(_) => None,
                    Indirection::Resolved(inner_target) => Some(inner_target.clone()),
                },
                _ => Some(target),
            },
            None => None,
        }
    }

    fn find_module(&self, name: &str) -> Option<ModuleRef> {
        self.module_namespace.find_item(name)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use quote::quote;

    use crate::codemodel::*;

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
    fn test_stub() -> Result<(), anyhow::Error> {
        let mut m = Module::new("crate");

        m.insert_type_stub("Foo")?;

        assert!(m.find_type("Foo").is_none());

        let s = StructBuilder::new("Foo").build()?;
        m.insert_struct(s)?;
        let foo_ref = m.find_type("Foo");
        if let Some(TypeRef::Struct(s)) = foo_ref {
            assert_eq!("Foo", s.name());
            return Ok(());
        } else {
            panic!("cannot find requested type 'Foo'");
        }
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

    #[test]
    fn test_attr_builder() -> Result<(), anyhow::Error> {
        let cm = Codemodel::new();

        // Test struct with attributes
        let s = StructBuilder::new("TestStruct")
            .attr("derive")?
            .attr_with_input("serde::serialize", TokenStream::new())?
            .field("name", cm.type_string())?
            .field("age", cm.type_u32())?
            .build()?;

        assert_eq!(s.attribute_list.len(), 2);
        assert_eq!(s.field_list.len(), 2);

        // Test enum with attributes
        let e = EnumBuilder::new("TestEnum")
            .attr("derive")?
            .attr_with_input("serde", TokenStream::new())?
            .unit_variant("A")?
            .tuple_variant("B", vec![cm.type_string()])?
            .struct_variant("C", |builder| {
                builder.field("x", cm.type_i32())?.field("y", cm.type_i32())
            })?
            .build()?;

        assert_eq!(e.attribute_list.len(), 2);
        assert_eq!(e.variant_list.len(), 3);

        Ok(())
    }

    #[test]
    fn test_duplicate_attr_allowed() -> Result<(), anyhow::Error> {
        // Test that duplicate attributes are allowed in struct
        let s = StructBuilder::new("TestStruct")
            .attr("derive")?
            .attr("derive")? // This should work
            .field("name", Codemodel::new().type_string())?
            .build()?;

        assert_eq!(s.attr_iter().count(), 2);

        // Test that duplicate attributes are allowed in enum
        let e = EnumBuilder::new("TestEnum")
            .attr("derive")?
            .attr("derive")? // This should work
            .unit_variant("A")?
            .build()?;

        assert_eq!(e.attr_iter().count(), 2);

        Ok(())
    }

    #[test]
    fn test_comprehensive_attr_usage() -> Result<(), anyhow::Error> {
        let cm = Codemodel::new();

        // Create a struct with multiple attributes
        let person_struct = StructBuilder::new("Person")
            .attr("serde::deserialize")?
            .attr_with_input("derive", quote!((Debug)))?
            .attr("repr")?
            .field("name", cm.type_string())?
            .field("age", cm.type_u32())?
            .field("active", cm.type_bool())?
            .build()?;

        // Verify struct attributes
        assert_eq!(person_struct.attr_iter().count(), 3);
        let attr_names: Vec<String> = person_struct
            .attr_iter()
            .map(|a| a.path().to_string())
            .collect();
        assert!(attr_names.iter().any(|n| n == "derive"));
        assert!(attr_names.iter().any(|n| n == "serde::deserialize"));
        assert!(attr_names.iter().any(|n| n == "repr"));

        // Create an enum with attributes
        let status_enum = EnumBuilder::new("Status")
            .attr("serialize")?
            .attr_with_input("derive", quote!((Default, serde::Deserialize)))?
            .unit_variant("Active")?
            .unit_variant("Inactive")?
            .tuple_variant("Pending", vec![cm.type_string()])?
            .struct_variant("Custom", |builder| {
                builder
                    .field("code", cm.type_i32())?
                    .field("message", cm.type_string())
            })?
            .build()?;

        // Verify enum attributes
        assert_eq!(status_enum.attr_iter().count(), 2);
        let enum_attr_names: Vec<String> = status_enum
            .attr_iter()
            .map(|a| a.path().to_string())
            .collect();
        assert!(enum_attr_names.contains(&"derive".to_string()));
        assert!(enum_attr_names.contains(&"derive".to_string()));
        assert_eq!(
            status_enum.attr_iter().nth(1).unwrap().input().to_string(),
            quote!((Default, serde::Deserialize)).to_string()
        );

        // Verify enum variants
        assert_eq!(status_enum.variant_iter().count(), 4);

        Ok(())
    }

    #[test]
    fn test_enum_builder() -> Result<(), anyhow::Error> {
        let mut cm = Codemodel::new();

        let e = EnumBuilder::new("Shape")
            .unit_variant("Circle")?
            .unit_variant("Square")?
            .tuple_variant("Rectangle", vec![cm.type_f64(), cm.type_f64()])?
            // Test struct variant with closure-based field builder
            .struct_variant("Point", |builder| {
                builder.field("x", cm.type_f64())?.field("y", cm.type_f64())
            })?
            .struct_variant("Line", |builder| {
                builder
                    .field("start_x", cm.type_f64())?
                    .field("start_y", cm.type_f64())?
                    .field("end_x", cm.type_f64())?
                    .field("end_y", cm.type_f64())
            })?
            .build()?;

        let mut m = Module::new("crate");
        m.insert_enum(e)?;
        cm.insert_crate(m)?;

        let type_shape = cm
            .find_type(&FQTN::from_str("crate::Shape").unwrap())
            .expect("Type not found");
        match type_shape {
            TypeRef::Enum(e) => {
                assert_eq!(e.variant_iter().count(), 5);

                // Collect variants into a map for easier checking
                let variants: std::collections::HashMap<String, &EnumVariant> = e
                    .variant_iter()
                    .map(|v| (v.name().to_string(), v))
                    .collect();

                // Check Circle variant (Unit)
                let circle = variants.get("Circle").expect("Circle variant not found");
                match circle.data() {
                    EnumVariantData::Unit => {}
                    _ => panic!("Circle should be a unit variant"),
                }

                // Check Square variant (Unit)
                let square = variants.get("Square").expect("Square variant not found");
                match square.data() {
                    EnumVariantData::Unit => {}
                    _ => panic!("Square should be a unit variant"),
                }

                // Check Rectangle variant (Tuple with 2 f64 types)
                let rectangle = variants
                    .get("Rectangle")
                    .expect("Rectangle variant not found");
                match rectangle.data() {
                    EnumVariantData::Tuple(types) => {
                        assert_eq!(types.len(), 2);
                        for type_ref_or_ts in types {
                            {
                                match type_ref_or_ts.unwrap_type_ref() {
                                    TypeRef::Builtin(builtin) => {
                                        assert_eq!(builtin.name(), "f64");
                                    }
                                    _ => panic!("Rectangle variant should contain f64 types"),
                                }
                            }
                        }
                    }
                    _ => panic!("Rectangle should be a tuple variant"),
                }

                // Check Point variant (Struct with x, y fields)
                let point = variants.get("Point").expect("Point variant not found");
                match point.data() {
                    EnumVariantData::Struct(fields) => {
                        assert_eq!(fields.len(), 2);
                        let field_map: std::collections::HashMap<String, &Field> =
                            fields.iter().map(|f| (f.name().to_string(), f)).collect();

                        let x_field = field_map.get("x").expect("x field not found");
                        assert_eq!(x_field.type_().unwrap_type_ref().name(), "f64");

                        let y_field = field_map.get("y").expect("y field not found");
                        assert_eq!(y_field.type_().unwrap_type_ref().name(), "f64");
                    }
                    _ => panic!("Point should be a struct variant"),
                }

                // Check Line variant (Struct with start_x, start_y, end_x, end_y fields)
                let line = variants.get("Line").expect("Line variant not found");
                match line.data() {
                    EnumVariantData::Struct(fields) => {
                        assert_eq!(fields.len(), 4);
                        let field_map: std::collections::HashMap<String, &Field> =
                            fields.iter().map(|f| (f.name().to_string(), f)).collect();

                        for field_name in ["start_x", "start_y", "end_x", "end_y"] {
                            let field = field_map
                                .get(field_name)
                                .expect(&format!("{field_name} field not found"));
                            assert_eq!(field.type_().unwrap_type_ref().name(), "f64");
                        }
                    }
                    _ => panic!("Line should be a struct variant"),
                }
            }
            _ => panic!("unexpected type variant"),
        }

        Ok(())
    }

    #[test]
    fn test_insert_implementation() -> Result<(), anyhow::Error> {
        use crate::codemodel::{
            function::FunctionBuilder,
            implementation::{Implementation, ImplementationBuilder},
        };

        let cm = Codemodel::new();
        let mut m = Module::new("crate");

        // Create a struct to implement for
        let test_struct = StructBuilder::new("TestStruct").build()?;
        let struct_ref = m.insert_struct(test_struct)?;

        // Create multiple functions with different signatures
        let new_fn = FunctionBuilder::new("new".to_string(), struct_ref.clone())
            .param("value".to_string(), cm.type_i32())
            .param("flag".to_string(), cm.type_bool())
            .build();
        let default_fn = FunctionBuilder::new("default".to_string(), struct_ref.clone()).build();
        let clone_fn = FunctionBuilder::new("clone".to_string(), struct_ref.clone())
            .param("src".to_string(), struct_ref.clone())
            .build();

        // Create implementation with multiple functions
        let implementation = ImplementationBuilder::new_inherent(struct_ref.clone())
            .function(new_fn)
            .function(default_fn)
            .function(clone_fn)
            .build();

        // Insert the implementation
        m.insert_implementation(implementation)?;

        // Verify the implementation was stored with all functions
        assert_eq!(m.implementations_iter().count(), 1);

        let stored_impl = m.implementations_iter().next().unwrap();
        match stored_impl {
            Implementation::InherentImpl {
                implementing_type,
                associated_functions,
            } => {
                assert_eq!(implementing_type.name(), "TestStruct");
                assert_eq!(associated_functions.len(), 3);

                // Check each function in detail
                let functions: std::collections::HashMap<
                    String,
                    &crate::codemodel::function::Function,
                > = associated_functions
                    .iter()
                    .map(|f| (f.name().to_string(), f))
                    .collect();

                // Check 'new' function
                let new_func = functions.get("new").unwrap();
                assert_eq!(new_func.return_type().name(), "TestStruct");
                let new_params: Vec<_> = new_func.function_params_iter().collect();
                assert_eq!(new_params.len(), 2);
                assert_eq!(new_params[0].name, "value");
                assert_eq!(new_params[0].type_.name(), "i32");
                assert_eq!(new_params[1].name, "flag");
                assert_eq!(new_params[1].type_.name(), "bool");

                // Check 'default' function
                let default_func = functions.get("default").unwrap();
                assert_eq!(default_func.return_type().name(), "TestStruct");
                let default_params: Vec<_> = default_func.function_params_iter().collect();
                assert_eq!(default_params.len(), 0);

                // Check 'clone' function
                let clone_func = functions.get("clone").unwrap();
                assert_eq!(clone_func.return_type().name(), "TestStruct");
                let clone_params: Vec<_> = clone_func.function_params_iter().collect();
                assert_eq!(clone_params.len(), 1);
                assert_eq!(clone_params[0].name, "src");
                assert_eq!(clone_params[0].type_.name(), "TestStruct");
            }
        }

        Ok(())
    }
}
