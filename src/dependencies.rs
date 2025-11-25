use std::collections::HashMap;

use squalid::OptionExt;

use crate::{AnyHashMap, Error};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DependencyType {
    Id,
    String,
    ListOfIds,
    ListOfStrings,
}

pub struct ExternalDependency {
    pub name: String,
    pub type_: DependencyType,
}

impl ExternalDependency {
    pub fn new(name: String, type_: DependencyType) -> Self {
        Self { name, type_ }
    }
}

pub struct InternalDependency {
    pub name: String,
    pub type_: DependencyType,
    pub resolver: InternalDependencyResolver,
}

impl InternalDependency {
    pub fn new(name: String, type_: DependencyType, resolver: InternalDependencyResolver) -> Self {
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
}

pub struct ColumnGetter {
    pub table_name: String,
    pub column_name: String,
}

impl ColumnGetter {
    pub fn new(table_name: String, column_name: String) -> Self {
        Self {
            table_name,
            column_name,
        }
    }
}

pub struct ArgumentInternalDependencyResolver {
    pub name: String,
}

impl ArgumentInternalDependencyResolver {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub struct ColumnGetterList {
    pub table_name: String,
    pub column_name: String,
}

impl ColumnGetterList {
    pub fn new(table_name: String, column_name: String) -> Self {
        Self {
            table_name,
            column_name,
        }
    }
}

pub struct LiteralValueInternalDependencyResolver(pub DependencyValue);

pub struct ExternalDependencyValue {
    pub name: String,
    pub value: DependencyValue,
}

pub type Id = i32;

#[derive(Clone)]
pub enum DependencyValue {
    Id(Id),
    String(String),
    List(Vec<DependencyValue>),
}

impl DependencyValue {
    pub fn as_id(&self) -> &Id {
        match self {
            Self::Id(id) => id,
            _ => panic!("Expected id"),
        }
    }

    pub fn as_string(&self) -> &String {
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
}

pub type InternalDependencyValue = ExternalDependencyValue;

#[derive(Clone, Default)]
pub struct ExternalDependencyValues {
    knowns: HashMap<String, DependencyValue>,
    anys: AnyHashMap,
}

impl ExternalDependencyValues {
    pub fn new() -> Self {
        Self {
            knowns: Default::default(),
            anys: Default::default(),
        }
    }

    pub fn insert(&mut self, name: String, value: DependencyValue) -> Result<(), Error> {
        if self.knowns.contains_key(&name) {
            return Err(Error::DependencyAlreadyPopulated(name));
        }
        self.knowns.insert(name, value).assert_none();
        Ok(())
    }

    pub fn insert_any<TValue: Clone + Send + Sync + 'static>(
        &mut self,
        name: String,
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
