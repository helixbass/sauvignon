use std::collections::HashMap;

use crate::{FieldResolver, IndexMap, OperationType};

pub enum TypeFull {
    Type(String),
    List(String),
    NonNull(String),
}

impl TypeFull {
    pub fn name(&self) -> &str {
        match self {
            Self::Type(name) => name,
            Self::List(name) => name,
            Self::NonNull(name) => name,
        }
    }
}

pub enum Type {
    Object(ObjectType),
    Scalar(ScalarType),
}

impl Type {
    pub fn is_query_type(&self) -> bool {
        matches!(
            self,
            Self::Object(type_) if type_.is_query_type()
        )
    }
}

pub trait TypeInterface {
    fn name(&self) -> &str;
}

impl TypeInterface for Type {
    fn name(&self) -> &str {
        match self {
            Self::Object(type_) => type_.name(),
            Self::Scalar(type_) => type_.name(),
        }
    }
}

pub struct ObjectType {
    name: String,
    pub is_top_level_type: Option<OperationType>,
    // TODO: are the fields on a type ordered?
    pub fields: IndexMap<String, Field>,
}

impl ObjectType {
    pub fn new(name: String, fields: Vec<Field>, is_top_level_type: Option<OperationType>) -> Self {
        Self {
            name,
            fields: IndexMap::from_iter(
                fields.into_iter().map(|field| (field.name.clone(), field)),
            ),
            is_top_level_type,
        }
    }

    pub fn is_query_type(&self) -> bool {
        matches!(self.is_top_level_type, Some(OperationType::Query))
    }
}

impl TypeInterface for ObjectType {
    fn name(&self) -> &str {
        &self.name
    }
}

pub enum ScalarType {
    BuiltIn(BuiltInScalarType),
}

impl TypeInterface for ScalarType {
    fn name(&self) -> &str {
        match self {
            Self::BuiltIn(type_) => type_.name(),
        }
    }
}

pub enum BuiltInScalarType {
    String(StringType),
}

impl TypeInterface for BuiltInScalarType {
    fn name(&self) -> &str {
        match self {
            Self::String(type_) => type_.name(),
        }
    }
}

pub struct StringType {}

impl StringType {
    pub fn new() -> Self {
        Self {}
    }
}

impl TypeInterface for StringType {
    fn name(&self) -> &str {
        "String"
    }
}

pub struct Field {
    pub name: String,
    pub type_: TypeFull,
    pub resolver: FieldResolver,
}

impl Field {
    pub fn new(name: String, type_: TypeFull, resolver: FieldResolver) -> Self {
        Self {
            name,
            type_,
            resolver,
        }
    }
}

pub fn builtin_types() -> HashMap<String, Type> {
    HashMap::from_iter([("String".to_owned(), string_type())])
}

pub fn string_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::String(
        StringType::new(),
    )))
}

pub struct Union {
    pub name: String,
    pub types: Vec<String>,
}

impl Union {
    pub fn new(name: String, types: Vec<String>) -> Self {
        Self { name, types }
    }
}
