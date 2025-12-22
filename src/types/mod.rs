use std::collections::HashMap;

use derive_builder::Builder;
use smol_str::SmolStr;
use squalid::{OptionExt, _d};

use crate::{
    ArgumentInternalDependencyResolver, CarverOrPopulator, DependencyType, DependencyValue,
    ExternalDependency, FieldResolver, IndexMap, IndexSet, InternalDependency,
    InternalDependencyResolver, LiteralValueInternalDependencyResolver, OperationType,
    StringCarver, ValuePopulator, ValuePopulatorList,
};

pub enum TypeFull {
    Type(SmolStr),
    List(Box<TypeFull>),
    NonNull(Box<TypeFull>),
}

impl TypeFull {
    pub fn name(&self) -> &str {
        match self {
            Self::Type(name) => name,
            Self::List(type_full) => type_full.name(),
            Self::NonNull(type_full) => type_full.name(),
        }
    }
}

pub enum Type {
    Object(ObjectType),
    Scalar(ScalarType),
    Enum(Enum),
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
            Self::Enum(type_) => type_.name(),
        }
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct ObjectType {
    #[builder(setter(skip), default = "self.default_typename_field()")]
    pub typename_field: Field,
    #[builder(setter(skip), default = "self.default_introspection_fields()")]
    pub introspection_fields: Option<HashMap<SmolStr, Field>>,
    #[builder(setter(into))]
    pub name: SmolStr,
    #[builder(setter(strip_option), default)]
    pub is_top_level_type: Option<OperationType>,
    // TODO: are the fields on a type ordered?
    #[builder(setter(custom))]
    pub fields: IndexMap<SmolStr, Field>,
    #[builder(default)]
    pub implements: Vec<SmolStr>,
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

    fn default_introspection_fields(&self) -> Option<HashMap<SmolStr, Field>> {
        self.is_top_level_type
            .flatten()
            .if_is(OperationType::Query)
            .map(|_| {
                [("__type".into(), Field::new_introspection_type())]
                    .into_iter()
                    .collect()
            })
    }
}

impl ObjectType {
    pub fn is_query_type(&self) -> bool {
        self.is_top_level_type.is(OperationType::Query)
    }

    pub fn maybe_field(&self, name: &str) -> Option<&Field> {
        match name {
            "__typename" => Some(&self.typename_field),
            "__type" if self.introspection_fields.is_some() => {
                Some(&self.introspection_fields.as_ref().unwrap()["__type"])
            }
            name => self.fields.get(name),
        }
    }

    pub fn field(&self, name: &str) -> &Field {
        self.maybe_field(name).unwrap()
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
    Int(IntType),
    Float(FloatType),
    Id(IdType),
}

impl TypeInterface for BuiltInScalarType {
    fn name(&self) -> &str {
        match self {
            Self::String(type_) => type_.name(),
            Self::Int(type_) => type_.name(),
            Self::Float(type_) => type_.name(),
            Self::Id(type_) => type_.name(),
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

pub struct IntType {}

impl IntType {
    pub fn new() -> Self {
        Self {}
    }
}

impl TypeInterface for IntType {
    fn name(&self) -> &str {
        "Int"
    }
}

pub struct FloatType {}

impl FloatType {
    pub fn new() -> Self {
        Self {}
    }
}

impl TypeInterface for FloatType {
    fn name(&self) -> &str {
        "Float"
    }
}

pub struct IdType {}

impl IdType {
    pub fn new() -> Self {
        Self {}
    }
}

impl TypeInterface for IdType {
    fn name(&self) -> &str {
        "ID"
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Field {
    #[builder(setter(into))]
    pub name: SmolStr,
    pub type_: TypeFull,
    pub resolver: FieldResolver,
    #[builder(setter(custom), default)]
    pub params: IndexMap<SmolStr, Param>,
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
    pub fn new_typename(type_name: SmolStr) -> Self {
        FieldBuilder::default()
            .name("__typename")
            .type_(TypeFull::Type("String".into()))
            .resolver(FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "__typename".into(),
                    DependencyType::String,
                    InternalDependencyResolver::LiteralValue(
                        LiteralValueInternalDependencyResolver(DependencyValue::String(type_name)),
                    ),
                )],
                CarverOrPopulator::Carver(Box::new(StringCarver::new("__typename".into()))),
            ))
            .build()
            .unwrap()
    }

    pub fn new_introspection_type() -> Self {
        FieldBuilder::default()
            .name("__type")
            .type_(TypeFull::Type("__Type".into()))
            .resolver(FieldResolver::new(
                vec![],
                vec![InternalDependency::new(
                    "name".into(),
                    DependencyType::String,
                    InternalDependencyResolver::Argument(ArgumentInternalDependencyResolver::new(
                        "name".into(),
                    )),
                )],
                CarverOrPopulator::Populator(ValuePopulator::new("name".into()).into()),
            ))
            .params([Param::new(
                "name".into(),
                // TODO: presumably non-null?
                TypeFull::Type("String".into()),
            )])
            .build()
            .unwrap()
    }
}

impl FieldInterface for Field {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> &TypeFull {
        &self.type_
    }

    fn params(&self) -> &IndexMap<SmolStr, Param> {
        &self.params
    }
}

pub trait FieldInterface {
    fn name(&self) -> &str;
    fn type_(&self) -> &TypeFull;
    fn params(&self) -> &IndexMap<SmolStr, Param>;
}

pub fn builtin_types() -> HashMap<SmolStr, Type> {
    [
        ("String".into(), string_type()),
        ("Int".into(), int_type()),
        ("Float".into(), float_type()),
        ("__Type".into(), introspection_type_type()),
        ("ID".into(), id_type()),
    ]
    .into_iter()
    .collect()
}

pub fn string_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::String(
        StringType::new(),
    )))
}

pub fn int_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::Int(IntType::new())))
}

pub fn float_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::Float(
        FloatType::new(),
    )))
}

pub fn id_type() -> Type {
    Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::Id(IdType::new())))
}

pub fn introspection_type_type() -> Type {
    Type::Object(
        ObjectTypeBuilder::default()
            .name("__Type")
            .fields([
                FieldBuilder::default()
                    .name("name")
                    .type_(TypeFull::Type("String".into()))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new(
                            "name".into(),
                            DependencyType::String,
                        )],
                        vec![],
                        CarverOrPopulator::Carver(Box::new(StringCarver::new("name".into()))),
                    ))
                    .build()
                    .unwrap(),
                FieldBuilder::default()
                    .name("interfaces")
                    .type_(TypeFull::List(Box::new(TypeFull::Type("__Type".into()))))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new(
                            "name".into(),
                            DependencyType::String,
                        )],
                        vec![InternalDependency::new(
                            "names".into(),
                            DependencyType::List(Box::new(DependencyType::String)),
                            InternalDependencyResolver::IntrospectionTypeInterfaces,
                        )],
                        CarverOrPopulator::PopulatorList(
                            ValuePopulatorList::new("name".into()).into(),
                        ),
                    ))
                    .build()
                    .unwrap(),
                FieldBuilder::default()
                    .name("possibleTypes")
                    .type_(TypeFull::List(Box::new(TypeFull::Type("__Type".into()))))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new(
                            "name".into(),
                            DependencyType::String,
                        )],
                        vec![InternalDependency::new(
                            "names".into(),
                            DependencyType::List(Box::new(DependencyType::String)),
                            InternalDependencyResolver::IntrospectionTypePossibleTypes,
                        )],
                        CarverOrPopulator::PopulatorList(
                            ValuePopulatorList::new("name".into()).into(),
                        ),
                    ))
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap(),
    )
}

pub struct Union {
    pub name: SmolStr,
    pub types: Vec<SmolStr>,
}

impl Union {
    pub fn new(name: SmolStr, types: Vec<SmolStr>) -> Self {
        Self { name, types }
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Interface {
    #[builder(setter(skip), default = "self.default_typename_field()")]
    pub typename_field: InterfaceField,
    #[builder(setter(into))]
    pub name: SmolStr,
    // TODO: are the fields on a type/interface ordered?
    #[builder(setter(custom))]
    pub fields: IndexMap<SmolStr, InterfaceField>,
    #[builder(default)]
    pub implements: Vec<SmolStr>,
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

    fn default_typename_field(&self) -> InterfaceField {
        InterfaceField::new_typename()
    }
}

impl Interface {
    pub fn maybe_field(&self, name: &str) -> Option<&InterfaceField> {
        match name {
            "__typename" => Some(&self.typename_field),
            name => self.fields.get(name),
        }
    }

    pub fn field(&self, name: &str) -> &InterfaceField {
        self.maybe_field(name).unwrap()
    }
}

pub struct InterfaceField {
    pub name: SmolStr,
    pub type_: TypeFull,
    pub params: IndexMap<SmolStr, Param>,
}

impl InterfaceField {
    pub fn new(name: SmolStr, type_: TypeFull, params: impl IntoIterator<Item = Param>) -> Self {
        Self {
            name,
            type_,
            params: params
                .into_iter()
                .map(|param| (param.name.clone(), param))
                .collect(),
        }
    }

    pub fn new_typename() -> Self {
        Self {
            name: "__typename".into(),
            type_: TypeFull::Type("String".into()),
            params: _d(),
        }
    }
}

impl FieldInterface for InterfaceField {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> &TypeFull {
        &self.type_
    }

    fn params(&self) -> &IndexMap<SmolStr, Param> {
        &self.params
    }
}

#[derive(Copy, Clone)]
pub enum TypeOrInterfaceField<'a> {
    Type(&'a Field),
    Interface(&'a InterfaceField),
    Union(&'a DummyUnionTypenameField),
}

impl<'a> FieldInterface for TypeOrInterfaceField<'a> {
    fn name(&self) -> &str {
        match self {
            Self::Type(type_) => type_.name(),
            Self::Interface(interface) => interface.name(),
            Self::Union(union) => &union.name,
        }
    }

    fn type_(&self) -> &TypeFull {
        match self {
            Self::Type(type_) => type_.type_(),
            Self::Interface(interface) => interface.type_(),
            Self::Union(union) => &union.type_,
        }
    }

    fn params(&self) -> &IndexMap<SmolStr, Param> {
        match self {
            Self::Type(type_) => type_.params(),
            Self::Interface(interface) => interface.params(),
            Self::Union(union) => &union.params,
        }
    }
}

impl<'a> From<&'a Field> for TypeOrInterfaceField<'a> {
    fn from(value: &'a Field) -> Self {
        Self::Type(value)
    }
}

impl<'a> From<&'a InterfaceField> for TypeOrInterfaceField<'a> {
    fn from(value: &'a InterfaceField) -> Self {
        Self::Interface(value)
    }
}

impl<'a> From<&'a DummyUnionTypenameField> for TypeOrInterfaceField<'a> {
    fn from(value: &'a DummyUnionTypenameField) -> Self {
        Self::Union(value)
    }
}

pub struct Param {
    pub name: SmolStr,
    pub type_: TypeFull,
}

impl Param {
    pub fn new(name: SmolStr, type_: TypeFull) -> Self {
        Self { name, type_ }
    }
}

pub struct DummyUnionTypenameField {
    pub name: SmolStr,
    pub type_: TypeFull,
    pub params: IndexMap<SmolStr, Param>,
}

impl Default for DummyUnionTypenameField {
    fn default() -> Self {
        Self {
            name: "__typename".into(),
            type_: TypeFull::Type("String".into()),
            params: _d(),
        }
    }
}

pub struct Enum {
    pub name: SmolStr,
    pub variants: IndexSet<SmolStr>,
}

impl Enum {
    pub fn new(name: SmolStr, variants: impl IntoIterator<Item = SmolStr>) -> Self {
        Self {
            name,
            variants: variants.into_iter().collect(),
        }
    }
}

impl TypeInterface for Enum {
    fn name(&self) -> &str {
        &self.name
    }
}
