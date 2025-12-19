use std::collections::HashMap;

use heck_smol_str::ToPascalCase;
use smol_str::SmolStr;
use tracing::instrument;

use crate::{
    pluralize, singularize, ExternalDependency, ExternalDependencyValues, InternalDependency,
    InternalDependencyValues, ResponseValue,
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

pub enum CarverOrPopulator {
    Carver(Box<dyn Carver>),
    CarverList(Box<dyn CarverList>),
    Populator(Populator),
    PopulatorList(PopulatorList),
    UnionOrInterfaceTypePopulator(Box<dyn UnionOrInterfaceTypePopulator>, Populator),
    UnionOrInterfaceTypePopulatorList(
        Box<dyn UnionOrInterfaceTypePopulatorList>,
        Box<dyn PopulatorListInterface>,
    ),
    OptionalPopulator(OptionalPopulator),
    OptionalUnionOrInterfaceTypePopulator(
        Box<dyn OptionalUnionOrInterfaceTypePopulator>,
        Populator,
    ),
}

pub trait Carver: Send + Sync {
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue;
}

pub struct StringCarver {
    pub name: SmolStr,
}

impl StringCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for StringCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
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

pub struct OptionalIntCarver {
    pub name: SmolStr,
}

impl OptionalIntCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for OptionalIntCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_optional_int()
            .into()
    }
}

pub struct OptionalFloatCarver {
    pub name: SmolStr,
}

impl OptionalFloatCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for OptionalFloatCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_optional_float()
            .into()
    }
}

pub struct OptionalEnumValueCarver {
    pub name: SmolStr,
}

impl OptionalEnumValueCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for OptionalEnumValueCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        match internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_optional_string()
        {
            None => ResponseValue::Null,
            Some(value) => ResponseValue::EnumValue(value.into()),
        }
    }
}

pub struct EnumValueCarver {
    pub name: SmolStr,
}

impl EnumValueCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for EnumValueCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::EnumValue(
            internal_dependencies
                .get(&self.name)
                .or_else(|| external_dependencies.get(&self.name))
                .unwrap()
                .as_string()
                .clone(),
        )
    }
}

pub struct TimestampCarver {
    pub name: SmolStr,
}

impl TimestampCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for TimestampCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_timestamp()
            .into()
    }
}

pub struct IdCarver {
    pub name: SmolStr,
}

impl IdCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for IdCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
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
                .as_id()
                .get_string(),
        )
    }
}

pub struct OptionalStringCarver {
    pub name: SmolStr,
}

impl OptionalStringCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for OptionalStringCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_optional_string()
            .map(|str| ResponseValue::String(str.into()))
            .unwrap_or_else(|| ResponseValue::Null)
    }
}

pub struct IntCarver {
    pub name: SmolStr,
}

impl IntCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for IntCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::Int(
            internal_dependencies
                .get(&self.name)
                .or_else(|| external_dependencies.get(&self.name))
                .unwrap()
                .as_int()
                .clone(),
        )
    }
}

pub struct DateCarver {
    pub name: SmolStr,
}

impl DateCarver {
    pub fn new(name: SmolStr) -> Self {
        Self { name }
    }
}

impl Carver for DateCarver {
    #[instrument(
        level = "trace",
        skip(self, external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        internal_dependencies
            .get(&self.name)
            .or_else(|| external_dependencies.get(&self.name))
            .unwrap()
            .as_date()
            .into()
    }
}

pub trait CarverList: Send + Sync {
    fn carve(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ResponseValue>;
}

pub struct EnumValueCarverList {
    pub singular: SmolStr,
}

impl EnumValueCarverList {
    pub fn new(singular: SmolStr) -> Self {
        Self { singular }
    }
}

impl CarverList for EnumValueCarverList {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
    fn carve(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ResponseValue> {
        internal_dependencies
            .get(&pluralize(&self.singular))
            .unwrap()
            .as_list()
            .into_iter()
            .map(|value| ResponseValue::EnumValue(value.as_string().clone()))
            .collect()
    }
}

pub enum Populator {
    Value(ValuePopulator),
    Values(ValuesPopulator),
    Dyn(Box<dyn PopulatorInterface>),
}

impl Populator {
    pub fn as_values(&self) -> &ValuesPopulator {
        match self {
            Self::Values(populator) => populator,
            _ => panic!("expected values populator"),
        }
    }
}

impl PopulatorInterface for Populator {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues {
        match self {
            Self::Value(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
            Self::Values(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
            Self::Dyn(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
        }
    }
}

impl From<ValuePopulator> for Populator {
    fn from(value: ValuePopulator) -> Self {
        Self::Value(value)
    }
}

impl From<ValuesPopulator> for Populator {
    fn from(value: ValuesPopulator) -> Self {
        Self::Values(value)
    }
}

pub trait PopulatorInterface: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ExternalDependencyValues;
}

pub struct ValuePopulator {
    pub key: SmolStr,
}

impl ValuePopulator {
    pub fn new(key: SmolStr) -> Self {
        Self { key }
    }
}

impl PopulatorInterface for ValuePopulator {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
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
    pub keys: HashMap<SmolStr, SmolStr>,
}

impl ValuesPopulator {
    pub fn new(keys: impl IntoIterator<Item = (SmolStr, SmolStr)>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

impl PopulatorInterface for ValuesPopulator {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
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

pub enum OptionalPopulator {
    Value(OptionalValuePopulator),
    Values(OptionalValuesPopulator),
    Dyn(Box<dyn OptionalPopulatorInterface>),
}

impl OptionalPopulatorInterface for OptionalPopulator {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Option<ExternalDependencyValues> {
        match self {
            Self::Value(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
            Self::Values(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
            Self::Dyn(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
        }
    }
}

impl From<OptionalValuePopulator> for OptionalPopulator {
    fn from(value: OptionalValuePopulator) -> Self {
        Self::Value(value)
    }
}

impl From<OptionalValuesPopulator> for OptionalPopulator {
    fn from(value: OptionalValuesPopulator) -> Self {
        Self::Values(value)
    }
}

pub trait OptionalPopulatorInterface: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Option<ExternalDependencyValues>;
}

pub struct OptionalValuePopulator {
    pub key: SmolStr,
}

impl OptionalValuePopulator {
    pub fn new(key: SmolStr) -> Self {
        Self { key }
    }
}

impl OptionalPopulatorInterface for OptionalValuePopulator {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Option<ExternalDependencyValues> {
        internal_dependencies
            .get(&self.key)
            .unwrap()
            .maybe_non_optional()
            .map(|internal_dependency_value| {
                let mut ret = ExternalDependencyValues::default();
                ret.insert(self.key.clone(), internal_dependency_value)
                    .unwrap();
                ret
            })
    }
}

pub struct OptionalValuesPopulator {
    pub keys: HashMap<SmolStr, SmolStr>,
}

impl OptionalValuesPopulator {
    pub fn new(keys: impl IntoIterator<Item = (SmolStr, SmolStr)>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

impl OptionalPopulatorInterface for OptionalValuesPopulator {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Option<ExternalDependencyValues> {
        // TODO: this is an opinionated algorithm in that it says
        // "all internal dependency values must be optional", could
        // imagine (if there are multiple internal dependencies in
        // an instance of this type) eg only wanting to make the
        // population conditional on the optional-ness of one of them?
        // In that case could extend the signature/fields of
        // OptionalValuesPopulator to support that?
        let internal_dependency_values = self
            .keys
            .iter()
            .map(|(internal_dependency_key, populated_key)| {
                (
                    populated_key,
                    internal_dependencies
                        .get(internal_dependency_key)
                        .unwrap()
                        .maybe_non_optional(),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            internal_dependency_values
                .iter()
                .all(|(_, value)| value.is_none())
                || internal_dependency_values
                    .iter()
                    .all(|(_, value)| value.is_some()),
            "Currently expecting all present or all missing"
        );
        if internal_dependency_values[0].1.is_none() {
            return None;
        }
        let internal_dependency_values = internal_dependency_values.into_iter().map(
            |(populated_key, internal_dependency_value)| {
                (populated_key, internal_dependency_value.unwrap())
            },
        );
        let mut ret = ExternalDependencyValues::default();
        for (populated_key, internal_dependency_value) in internal_dependency_values {
            ret.insert(populated_key.clone(), internal_dependency_value)
                .unwrap();
        }
        Some(ret)
    }
}

pub enum PopulatorList {
    Value(ValuePopulatorList),
    Dyn(Box<dyn PopulatorListInterface>),
}

impl PopulatorList {
    pub fn as_value(&self) -> &ValuePopulatorList {
        match self {
            Self::Value(populator) => populator,
            _ => panic!("Expected value"),
        }
    }
}

impl PopulatorListInterface for PopulatorList {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues> {
        match self {
            Self::Value(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
            Self::Dyn(populator) => {
                populator.populate(external_dependencies, internal_dependencies)
            }
        }
    }
}

impl From<ValuePopulatorList> for PopulatorList {
    fn from(value: ValuePopulatorList) -> Self {
        Self::Value(value)
    }
}

pub trait PopulatorListInterface: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues>;
}

pub struct ValuePopulatorList {
    pub singular: SmolStr,
    pub plural: SmolStr,
}

impl ValuePopulatorList {
    pub fn new(singular: SmolStr) -> Self {
        Self {
            plural: pluralize(&singular),
            singular,
        }
    }
}

impl PopulatorListInterface for ValuePopulatorList {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues> {
        internal_dependencies
            .get(&self.plural)
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

pub trait UnionOrInterfaceTypePopulator: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> SmolStr;
}

pub struct TypeDepluralizer {}

impl TypeDepluralizer {
    pub fn new() -> Self {
        Self {}
    }
}

impl UnionOrInterfaceTypePopulator for TypeDepluralizer {
    #[instrument(
        level = "trace",
        skip(self, _external_dependencies, internal_dependencies)
    )]
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> SmolStr {
        singularize(internal_dependencies.get("type").unwrap().as_string()).to_pascal_case()
    }
}

pub trait OptionalUnionOrInterfaceTypePopulator: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Option<SmolStr>;
}

pub trait UnionOrInterfaceTypePopulatorList: Send + Sync {
    fn populate(
        &self,
        external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<SmolStr>;
}
