use std::collections::HashMap;

use crate::{
    ArgumentInternalDependencyResolver, CarverOrPopulator, DependencyType, DependencyValue,
    ExternalDependency, FieldResolver, IndexMap, InternalDependency, InternalDependencyResolver,
    LiteralValueInternalDependencyResolver, OperationType, StringCarver, ValuePopulator,
    ValuePopulatorList,
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
    pub introspection_fields: Option<HashMap<String, Field>>,
}

impl ObjectType {
    pub fn new(
        name: String,
        fields: Vec<Field>,
        is_top_level_type: Option<OperationType>,
        implements: Vec<String>,
    ) -> Self {
        let typename_field = Field::new_typename(name.clone());

        let introspection_fields = is_top_level_type
            .as_ref()
            .filter(|is_top_level_type| matches!(is_top_level_type, OperationType::Query))
            .map(|_| {
                [("__type".to_owned(), Field::new_introspection_type())]
                    .into_iter()
                    .collect()
            });

        Self {
            name,
            fields: fields
                .into_iter()
                .map(|field| (field.name.clone(), field))
                .collect(),
            is_top_level_type,
            implements,
            typename_field,
            introspection_fields,
        }
    }

    pub fn is_query_type(&self) -> bool {
        matches!(self.is_top_level_type, Some(OperationType::Query))
    }

    pub fn field(&self, name: &str) -> &Field {
        match name {
            "__typename" => &self.typename_field,
            "__type" if self.introspection_fields.is_some() => {
                &self.introspection_fields.as_ref().unwrap()["__type"]
            }
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
    pub params: IndexMap<String, Param>,
}

impl Field {
    pub fn new(name: String, type_: TypeFull, resolver: FieldResolver, params: Vec<Param>) -> Self {
        Self {
            name,
            type_,
            resolver,
            params: params
                .into_iter()
                .map(|param| (param.name.clone(), param))
                .collect(),
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
            params: Default::default(),
        }
    }

    pub fn new_introspection_type() -> Self {
        Self::new(
            "__type".to_owned(),
            TypeFull::Type("__Type".to_owned()),
            FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "name".to_owned(),
                    DependencyType::String,
                    InternalDependencyResolver::Argument(ArgumentInternalDependencyResolver::new(
                        "name".to_owned(),
                    )),
                )],
                CarverOrPopulator::Populator(Box::new(ValuePopulator::new("name".to_owned()))),
            ),
            vec![Param::new(
                "name".to_owned(),
                // TODO: presumably non-null?
                TypeFull::Type("String".to_owned()),
            )],
        )
    }
}

pub fn builtin_types() -> HashMap<String, Type> {
    [
        ("String".to_owned(), string_type()),
        ("__Type".to_owned(), introspection_type_type()),
    ]
    .into_iter()
    .collect()
}

pub fn string_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::String(
        StringType::new(),
    )))
}

pub fn introspection_type_type() -> Type {
    Type::Object(ObjectType::new(
        "__Type".to_owned(),
        vec![
            Field::new(
                "name".to_owned(),
                TypeFull::Type("String".to_owned()),
                FieldResolver::new(
                    vec![ExternalDependency::new(
                        "name".to_owned(),
                        DependencyType::String,
                    )],
                    vec![],
                    CarverOrPopulator::Carver(Box::new(StringCarver::new("name".to_owned()))),
                ),
                vec![],
            ),
            Field::new(
                "interfaces".to_owned(),
                TypeFull::List("__Type".to_owned()),
                FieldResolver::new(
                    vec![ExternalDependency::new(
                        "name".to_owned(),
                        DependencyType::String,
                    )],
                    vec![InternalDependency::new(
                        "names".to_owned(),
                        DependencyType::ListOfStrings,
                        InternalDependencyResolver::IntrospectionTypeInterfaces,
                    )],
                    CarverOrPopulator::PopulatorList(Box::new(ValuePopulatorList::new(
                        "name".to_owned(),
                    ))),
                ),
                vec![],
            ),
        ],
        None,
        vec![],
    ))
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

pub struct Param {
    pub name: String,
    pub type_: TypeFull,
}

impl Param {
    pub fn new(name: String, type_: TypeFull) -> Self {
        Self { name, type_ }
    }
}
