use std::collections::HashMap;
use std::error;
use std::mem;

use chrono::NaiveDate;
use jiff::Timestamp;
use smallvec::SmallVec;
use smol_str::{SmolStr, ToSmolStr};
use sqlx::postgres::PgValueRef;
use squalid::{OptionExt, _d};
use uuid::Uuid;

use crate::{AnyHashMap, Error};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DependencyType {
    Id,
    String,
    ListOfIds,
    ListOfStrings,
    OptionalInt,
    OptionalFloat,
    OptionalString,
    Timestamp,
    OptionalId,
    Int,
    Date,
}

pub struct ExternalDependency {
    pub name: SmolStr,
    pub type_: DependencyType,
}

impl ExternalDependency {
    pub fn new(name: SmolStr, type_: DependencyType) -> Self {
        Self { name, type_ }
    }
}

pub struct InternalDependency {
    pub name: SmolStr,
    pub type_: DependencyType,
    pub resolver: InternalDependencyResolver,
}

impl InternalDependency {
    pub fn new(name: SmolStr, type_: DependencyType, resolver: InternalDependencyResolver) -> Self {
        Self {
            name,
            type_,
            resolver,
        }
    }
}

pub enum InternalDependencyResolver {
    ColumnGetter(ColumnGetter),
    Argument(ArgumentInternalDependencyResolver),
    ColumnGetterList(ColumnGetterList),
    LiteralValue(LiteralValueInternalDependencyResolver),
    IntrospectionTypeInterfaces,
    IntrospectionTypePossibleTypes,
}

impl InternalDependencyResolver {
    pub fn can_be_resolved_synchronously(&self) -> bool {
        match self {
            Self::ColumnGetter(_) => false,
            Self::Argument(_) => true,
            Self::ColumnGetterList(_) => false,
            Self::LiteralValue(_) => true,
            Self::IntrospectionTypeInterfaces => true,
            Self::IntrospectionTypePossibleTypes => true,
        }
    }
}

pub struct ColumnGetter {
    pub table_name: SmolStr,
    pub column_name: SmolStr,
    pub id_column_name: SmolStr,
}

impl ColumnGetter {
    pub fn new(table_name: SmolStr, column_name: SmolStr, id_column_name: SmolStr) -> Self {
        Self {
            table_name,
            column_name,
            id_column_name,
        }
    }
}

pub enum ColumnValueMassager {
    OptionalString(Box<dyn ColumnValueMassagerInterface<Option<SmolStr>>>),
    String(Box<dyn ColumnValueMassagerInterface<SmolStr>>),
}

impl ColumnValueMassager {
    pub fn as_optional_string(&self) -> &Box<dyn ColumnValueMassagerInterface<Option<SmolStr>>> {
        match self {
            Self::OptionalString(value) => value,
            _ => panic!("Expected optional string"),
        }
    }

    pub fn as_string(&self) -> &Box<dyn ColumnValueMassagerInterface<SmolStr>> {
        match self {
            Self::String(value) => value,
            _ => panic!("Expected string"),
        }
    }
}

pub trait ColumnValueMassagerInterface<TMassaged>: Send + Sync {
    fn massage(
        &self,
        value: PgValueRef<'_>,
    ) -> Result<TMassaged, Box<dyn error::Error + Sync + Send>>;
}

pub struct ArgumentInternalDependencyResolver {
    pub name: SmolStr,
}

impl ArgumentInternalDependencyResolver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

pub struct ColumnGetterList {
    pub table_name: SmolStr,
    pub column_name: SmolStr,
    pub wheres: Vec<Where>,
}

impl ColumnGetterList {
    pub fn new(table_name: SmolStr, column_name: SmolStr, wheres: Vec<Where>) -> Self {
        Self {
            table_name,
            column_name,
            wheres,
        }
    }
}

pub struct Where {
    pub column_name: SmolStr,
}

impl Where {
    pub fn new(column_name: SmolStr) -> Self {
        Self { column_name }
    }
}

#[derive(Debug)]
pub struct WhereResolved {
    pub column_name: SmolStr,
    pub value: DependencyValue,
}

impl WhereResolved {
    pub fn new(column_name: SmolStr, value: DependencyValue) -> Self {
        Self { column_name, value }
    }
}

pub type WheresResolved = SmallVec<[WhereResolved; 2]>;

pub struct LiteralValueInternalDependencyResolver(pub DependencyValue);

pub struct ExternalDependencyValue {
    pub name: SmolStr,
    pub value: DependencyValue,
}

#[derive(Clone, Debug)]
pub enum Id {
    Int(i32),
    String(SmolStr),
    Uuid(Uuid),
}

impl Id {
    pub fn get_string(&self) -> SmolStr {
        match self {
            Self::Int(value) => value.to_smolstr(),
            Self::String(value) => value.clone(),
            Self::Uuid(value) => value.to_smolstr(),
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(value) => *value,
            _ => panic!("Expected int"),
        }
    }

    pub fn as_string(&self) -> &SmolStr {
        match self {
            Self::String(value) => value,
            _ => panic!("Expected string"),
        }
    }

    pub fn as_uuid(&self) -> &Uuid {
        match self {
            Self::Uuid(value) => value,
            _ => panic!("Expected uuid"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum DependencyValue {
    Id(Id),
    String(SmolStr),
    List(Vec<DependencyValue>),
    Float(f64),
    OptionalInt(Option<i32>),
    OptionalFloat(Option<f64>),
    OptionalString(Option<SmolStr>),
    OptionalId(Option<Id>),
    Timestamp(Timestamp),
    Int(i32),
    Date(NaiveDate),
}

impl DependencyValue {
    pub fn as_id(&self) -> &Id {
        match self {
            Self::Id(id) => id,
            _ => panic!("Expected id"),
        }
    }

    pub fn as_string(&self) -> &SmolStr {
        match self {
            Self::String(string) => string,
            _ => panic!("Expected string"),
        }
    }

    pub fn as_list(&self) -> &Vec<DependencyValue> {
        match self {
            Self::List(values) => values,
            _ => panic!("Expected list"),
        }
    }

    pub fn as_float(&self) -> f64 {
        match self {
            Self::Float(value) => *value,
            _ => panic!("Expected float"),
        }
    }

    pub fn as_optional_int(&self) -> Option<i32> {
        match self {
            Self::OptionalInt(value) => *value,
            _ => panic!("Expected optional int"),
        }
    }

    pub fn as_optional_float(&self) -> Option<f64> {
        match self {
            Self::OptionalFloat(value) => *value,
            _ => panic!("Expected optional float"),
        }
    }

    pub fn as_optional_string(&self) -> Option<&str> {
        match self {
            Self::OptionalString(value) => value.as_deref(),
            _ => panic!("Expected optional string"),
        }
    }

    pub fn as_timestamp(&self) -> Timestamp {
        match self {
            Self::Timestamp(value) => *value,
            _ => panic!("Expected timestamp"),
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(value) => *value,
            _ => panic!("Expected int"),
        }
    }

    pub fn as_date(&self) -> &NaiveDate {
        match self {
            Self::Date(value) => value,
            _ => panic!("Expected date"),
        }
    }

    pub fn maybe_non_optional(&self) -> Option<Self> {
        match self {
            Self::OptionalId(value) => value.as_ref().map(|value| Self::Id(value.clone())),
            Self::OptionalString(value) => value.as_ref().map(|value| Self::String(value.clone())),
            Self::OptionalFloat(value) => value.map(|value| Self::Float(value)),
            Self::OptionalInt(value) => value.map(|value| Self::Int(value)),
            _ => panic!("Expected optional type"),
        }
    }
}

pub type InternalDependencyValue = ExternalDependencyValue;

#[derive(Clone, Default)]
pub enum ExternalDependencyValues {
    #[default]
    Empty,
    Single(SmolStr, DependencyValue),
    Full(ExternalDependencyValuesFull),
}

impl ExternalDependencyValues {
    pub fn as_full_mut(&mut self) -> &mut ExternalDependencyValuesFull {
        match self {
            Self::Full(value) => value,
            _ => panic!("Expected full"),
        }
    }

    pub fn into_single(self) -> (SmolStr, DependencyValue) {
        match self {
            Self::Single(name, value) => (name, value),
            _ => panic!("Expected single"),
        }
    }

    fn promote_self_single_to_full(&mut self) {
        let mut other = Self::Full(_d());
        mem::swap(self, &mut other);
        let (name, value) = other.into_single();
        self.as_full_mut().insert(name, value).unwrap();
    }

    fn promote_self_empty_to_single(&mut self, name: SmolStr, value: DependencyValue) {
        *self = Self::Single(name, value);
    }

    fn promote_self_empty_to_full(&mut self) {
        *self = Self::Full(_d());
    }

    pub fn insert(&mut self, name: SmolStr, value: DependencyValue) -> Result<(), Error> {
        if matches!(self, Self::Empty) {
            self.promote_self_empty_to_single(name, value);
            return Ok(());
        }
        if matches!(self, Self::Single(_, _)) {
            self.promote_self_single_to_full();
        }
        self.as_full_mut().insert(name, value)
    }

    pub fn insert_any<TValue: Clone + Send + Sync + 'static>(
        &mut self,
        name: SmolStr,
        value: TValue,
    ) -> Result<(), Error> {
        if matches!(self, Self::Single(_, _)) {
            self.promote_self_single_to_full();
        } else if matches!(self, Self::Empty) {
            self.promote_self_empty_to_full();
        }
        self.as_full_mut().insert_any(name, value)
    }

    pub fn get(&self, name: &str) -> Option<&DependencyValue> {
        match self {
            Self::Empty => None,
            Self::Single(single_name, value) => (name == single_name).then_some(value),
            Self::Full(full) => full.get(name),
        }
    }

    pub fn get_any<TValue: Send + Sync + 'static>(&self, name: &str) -> Option<&TValue> {
        match self {
            Self::Empty => None,
            Self::Single(_, _) => None,
            Self::Full(full) => full.get_any(name),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Single(_, _) => 1,
            Self::Full(full) => full.len(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ExternalDependencyValuesFull {
    knowns: HashMap<SmolStr, DependencyValue>,
    anys: AnyHashMap,
}

impl ExternalDependencyValuesFull {
    pub fn insert(&mut self, name: SmolStr, value: DependencyValue) -> Result<(), Error> {
        if self.knowns.contains_key(&name) {
            return Err(Error::DependencyAlreadyPopulated(name));
        }
        self.knowns.insert(name, value).assert_none();
        Ok(())
    }

    pub fn insert_any<TValue: Clone + Send + Sync + 'static>(
        &mut self,
        name: SmolStr,
        value: TValue,
    ) -> Result<(), Error> {
        if self.anys.contains_key(&name) {
            return Err(Error::DependencyAlreadyPopulated(name));
        }
        self.anys.insert(name, value).assert_none();
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&DependencyValue> {
        self.knowns.get(name)
    }

    pub fn get_any<TValue: Send + Sync + 'static>(&self, name: &str) -> Option<&TValue> {
        self.anys.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.knowns.is_empty() && self.anys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.knowns.len() + self.anys.len()
    }
}

pub type InternalDependencyValues = ExternalDependencyValues;
