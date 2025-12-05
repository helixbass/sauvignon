use std::collections::HashMap;

use crate::{
    CarverOrPopulator, DependencyType, DependencyValue, FieldResolver, IndexMap,
    InternalDependency, InternalDependencyResolver, LiteralValueInternalDependencyResolver,
    OperationType, StringCarver,
};

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

    pub fn as_object(&self) -> &ObjectType {
        match self {
            Self::Object(object) => object,
            _ => panic!("expected object"),
        }
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
    pub name: String,
    pub is_top_level_type: Option<OperationType>,
    // TODO: are the fields on a type ordered?
    pub fields: IndexMap<String, Field>,
    pub implements: Vec<String>,
    pub typename_field: Field,
}

impl ObjectType {
    pub fn new(
        name: String,
        fields: Vec<Field>,
        is_top_level_type: Option<OperationType>,
        implements: Vec<String>,
    ) -> Self {
        let typename_field = Field::new_typename(name.clone());

        Self {
            name,
            fields: fields
                .into_iter()
                .map(|field| (field.name.clone(), field))
                .collect(),
            is_top_level_type,
            implements,
            typename_field,
        }
    }

    pub fn is_query_type(&self) -> bool {
        matches!(self.is_top_level_type, Some(OperationType::Query))
    }

    pub fn field(&self, name: &str) -> &Field {
        match name {
            "__typename" => &self.typename_field,
            name => &self.fields[name],
        }
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

    pub fn new_typename(type_name: String) -> Self {
        Self {
            name: "__typename".to_owned(),
            type_: TypeFull::Type("String".to_owned()),
            resolver: FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "__typename".to_owned(),
                    DependencyType::String,
                    InternalDependencyResolver::LiteralValue(
                        LiteralValueInternalDependencyResolver(DependencyValue::String(type_name)),
                    ),
                )],
                CarverOrPopulator::Carver(Box::new(StringCarver::new("__typename".to_owned()))),
            ),
        }
    }
}

pub fn builtin_types() -> HashMap<String, Type> {
    [("String".to_owned(), string_type())].into_iter().collect()
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

pub struct Interface {
    pub name: String,
    // TODO: are the fields on a type/interface ordered?
    pub fields: IndexMap<String, InterfaceField>,
    pub implements: Vec<String>,
}

impl Interface {
    pub fn new(name: String, fields: Vec<InterfaceField>, implements: Vec<String>) -> Self {
        Self {
            name,
            fields: fields
                .into_iter()
                .map(|field| (field.name.clone(), field))
                .collect(),
            implements,
        }
    }
}

pub struct InterfaceField {
    pub name: String,
    pub type_: TypeFull,
}

impl InterfaceField {
    pub fn new(name: String, type_: TypeFull) -> Self {
        Self { name, type_ }
    }
}
