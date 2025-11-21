use crate::{FieldResolver, OperationType};

pub enum TypeFull {
    Type(String),
    List(String),
    NonNull(String),
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
    pub fields: Vec<Field>,
}

impl ObjectType {
    pub fn new(name: String, fields: Vec<Field>, is_top_level_type: Option<OperationType>) -> Self {
        Self {
            name,
            fields,
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
