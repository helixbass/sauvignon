use std::collections::HashMap;

use inflector::Inflector;

use crate::{
    ExternalDependency, ExternalDependencyValues, InternalDependency, InternalDependencyValues,
    ResponseValue,
};

pub struct FieldResolver {
    pub external_dependencies: Vec<ExternalDependency>,
    pub internal_dependencies: Vec<InternalDependency>,
    pub carver_or_populator: CarverOrPopulator,
}

impl FieldResolver {
    pub fn new(
        external_dependencies: Vec<ExternalDependency>,
        internal_dependencies: Vec<InternalDependency>,
        carver_or_populator: CarverOrPopulator,
    ) -> Self {
        Self {
            external_dependencies,
            internal_dependencies,
            carver_or_populator,
        }
    }
}

pub trait Carver {
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue;
}

pub struct StringCarver {
    pub name: String,
}

impl StringCarver {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Carver for StringCarver {
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::String(
            internal_dependencies
                .get(&self.name)
                .or_else(|| external_dependencies.get(&self.name))
                .unwrap()
                .as_string()
                .clone(),
        )
    }
}

pub enum CarverOrPopulator {
    Carver(Box<dyn Carver>),
    Populator(Box<dyn Populator>),
    PopulatorList(Box<dyn PopulatorList>),
    UnionOrInterfaceTypePopulator(Box<dyn UnionOrInterfaceTypePopulator>, Box<dyn Populator>),
    UnionOrInterfaceTypePopulatorList(
        Box<dyn UnionOrInterfaceTypePopulatorList>,
        Box<dyn PopulatorList>,
    ),
}

pub trait Populator {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues;
}

pub struct ValuePopulator {
    pub key: String,
}

impl ValuePopulator {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}

impl Populator for ValuePopulator {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues {
        let mut ret = ExternalDependencyValues::default();
        ret.insert(
            self.key.clone(),
            internal_dependencies.get(&self.key).unwrap().clone(),
        )
        .unwrap();
        ret
    }
}

pub struct ValuesPopulator {
    pub keys: HashMap<String, String>,
}

impl ValuesPopulator {
    pub fn new(keys: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

impl Populator for ValuesPopulator {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues {
        let mut ret = ExternalDependencyValues::default();
        for (internal_dependency_key, populated_key) in &self.keys {
            ret.insert(
                populated_key.clone(),
                internal_dependencies
                    .get(internal_dependency_key)
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        }
        ret
    }
}

pub trait PopulatorList {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues>;
}

pub struct ValuePopulatorList {
    pub singular: String,
}

impl ValuePopulatorList {
    pub fn new(singular: String) -> Self {
        Self { singular }
    }
}

impl PopulatorList for ValuePopulatorList {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues> {
        internal_dependencies
            .get(&self.singular.to_plural())
            .unwrap()
            .as_list()
            .into_iter()
            .map(|value| {
                let mut ret = ExternalDependencyValues::default();
                ret.insert(self.singular.clone(), value.clone()).unwrap();
                ret
            })
            .collect()
    }
}

pub trait UnionOrInterfaceTypePopulator {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> String;
}

pub struct TypeDepluralizer {}

impl TypeDepluralizer {
    pub fn new() -> Self {
        Self {}
    }
}

impl UnionOrInterfaceTypePopulator for TypeDepluralizer {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> String {
        internal_dependencies
            .get("type")
            .unwrap()
            .as_string()
            .to_singular()
            .to_pascal_case()
    }
}

pub trait UnionOrInterfaceTypePopulatorList {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<String>;
}
