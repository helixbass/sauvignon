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

pub struct StringColumnCarver {
    pub column_name: String,
}

impl StringColumnCarver {
    pub fn new(column_name: String) -> Self {
        Self { column_name }
    }
}

impl Carver for StringColumnCarver {
    fn carve(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::String(
            internal_dependencies
                .get(&self.column_name)
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
    UnionOrInterfaceTypePopulator(Box<dyn UnionOrInterfaceTypePopulator>),
}

pub trait Populator {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues;
}

pub struct IdPopulator {}

impl IdPopulator {
    pub fn new() -> Self {
        Self {}
    }
}

impl Populator for IdPopulator {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues {
        let mut ret = ExternalDependencyValues::default();
        ret.insert(
            "id".to_owned(),
            internal_dependencies.get("id").unwrap().clone(),
        )
        .unwrap();
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

pub struct IdPopulatorList {}

impl IdPopulatorList {
    pub fn new() -> Self {
        Self {}
    }
}

impl PopulatorList for IdPopulatorList {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues> {
        internal_dependencies
            .get("ids")
            .unwrap()
            .as_list()
            .into_iter()
            .map(|id| {
                let mut ret = ExternalDependencyValues::default();
                ret.insert("id".to_owned(), id.clone()).unwrap();
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
