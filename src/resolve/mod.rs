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
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        unimplemented!()
    }
}

pub enum CarverOrPopulator {
    Carver(Box<dyn Carver>),
    Populator(Box<dyn Populator>),
}

pub trait Populator {
    fn populate(
        &self,
        into: &mut ExternalDependencyValues,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    );
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
        into: &mut ExternalDependencyValues,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) {
        unimplemented!()
    }
}
