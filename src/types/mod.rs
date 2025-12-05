use std::collections::HashMap;

use derive_builder::Builder;
use squalid::OptionExt;

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

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct ObjectType {
    #[builder(setter(skip), default = "self.default_typename_field()")]
    pub typename_field: Field,
    #[builder(setter(skip), default = "self.default_introspection_fields()")]
    pub introspection_fields: Option<HashMap<String, Field>>,
    #[builder(setter(into))]
    pub name: String,
    #[builder(setter(strip_option), default)]
    pub is_top_level_type: Option<OperationType>,
    // TODO: are the fields on a type ordered?
    #[builder(setter(custom))]
    pub fields: IndexMap<String, Field>,
    #[builder(default)]
    pub implements: Vec<String>,
}

impl ObjectTypeBuilder {
    pub fn fields(self, fields: impl IntoIterator<Item = Field>) -> Self {
        let mut new = self;
        new.fields = Some(
            fields
                .into_iter()
                .map(|field| (field.name.clone(), field))
                .collect(),
        );
        new
    }

    fn default_typename_field(&self) -> Field {
        Field::new_typename(self.name.clone().unwrap())
    }

    fn default_introspection_fields(&self) -> Option<HashMap<String, Field>> {
        self.is_top_level_type
            .flatten()
            .if_is(OperationType::Query)
            .map(|_| {
                [("__type".to_owned(), Field::new_introspection_type())]
                    .into_iter()
                    .collect()
            })
    }
}

impl ObjectType {
    pub fn is_query_type(&self) -> bool {
        self.is_top_level_type.is(OperationType::Query)
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

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Field {
    #[builder(setter(into))]
    pub name: String,
    pub type_: TypeFull,
    pub resolver: FieldResolver,
    #[builder(setter(custom), default)]
    pub params: IndexMap<String, Param>,
}

impl FieldBuilder {
    pub fn params(self, params: impl IntoIterator<Item = Param>) -> Self {
        let mut new = self;
        new.params = Some(
            params
                .into_iter()
                .map(|param| (param.name.clone(), param))
                .collect(),
        );
        new
    }
}

impl Field {
    pub fn new_typename(type_name: String) -> Self {
        FieldBuilder::default()
            .name("__typename")
            .type_(TypeFull::Type("String".to_owned()))
            .resolver(FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "__typename".to_owned(),
                    DependencyType::String,
                    InternalDependencyResolver::LiteralValue(
                        LiteralValueInternalDependencyResolver(DependencyValue::String(type_name)),
                    ),
                )],
                CarverOrPopulator::Carver(Box::new(StringCarver::new("__typename".to_owned()))),
            ))
            .build()
            .unwrap()
    }

    pub fn new_introspection_type() -> Self {
        FieldBuilder::default()
            .name("__type")
            .type_(TypeFull::Type("__Type".to_owned()))
            .resolver(FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "name".to_owned(),
                    DependencyType::String,
                    InternalDependencyResolver::Argument(ArgumentInternalDependencyResolver::new(
                        "name".to_owned(),
                    )),
                )],
                CarverOrPopulator::Populator(Box::new(ValuePopulator::new("name".to_owned()))),
            ))
            .params([Param::new(
                "name".to_owned(),
                // TODO: presumably non-null?
                TypeFull::Type("String".to_owned()),
            )])
            .build()
            .unwrap()
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
    Type::Object(
        ObjectTypeBuilder::default()
            .name("__Type")
            .fields([
                FieldBuilder::default()
                    .name("name")
                    .type_(TypeFull::Type("String".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new(
                            "name".to_owned(),
                            DependencyType::String,
                        )],
                        vec![],
                        CarverOrPopulator::Carver(Box::new(StringCarver::new("name".to_owned()))),
                    ))
                    .build()
                    .unwrap(),
                FieldBuilder::default()
                    .name("interfaces")
                    .type_(TypeFull::List("__Type".to_owned()))
                    .resolver(FieldResolver::new(
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
                    ))
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap(),
    )
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

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Interface {
    #[builder(setter(into))]
    pub name: String,
    // TODO: are the fields on a type/interface ordered?
    #[builder(setter(custom))]
    pub fields: IndexMap<String, InterfaceField>,
    #[builder(default)]
    pub implements: Vec<String>,
}

impl InterfaceBuilder {
    pub fn fields(self, fields: Vec<InterfaceField>) -> Self {
        let mut new = self;
        new.fields = Some(
            fields
                .into_iter()
                .map(|field| (field.name.clone(), field))
                .collect(),
        );
        new
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
