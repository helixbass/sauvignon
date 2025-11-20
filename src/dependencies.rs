pub enum DependencyType {
    Id,
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

pub struct ExternalDependencyValue {
    pub name: String,
    pub value: DependencyValue,
}

pub enum DependencyValue {
    DbValue(Omg),
    VariableValue(Omg),
}

pub struct InternalDependencyValue {
    pub name: String,
    pub value: DependencyValue,
}
