mod any_hash_map;
mod database;
mod dependencies;
mod error;
mod hash;
mod inscribe;
mod operation;
mod parser;
mod plan;
mod positions;
mod request;
mod resolve;
mod response;
mod schema;
mod smolstr;
mod string;
mod types;

pub use heck;
pub use heck_smol_str;
pub use indexmap::{IndexMap, IndexSet};
pub use smol_str;
pub use strum;

pub use shared::pluralize;

pub use crate::any_hash_map::AnyHashMap;
pub use crate::database::{
    ColumnToken, ColumnTokens, Database, DatabaseInterface, PostgresColumnMassager,
    PostgresDatabase,
};
pub use crate::dependencies::{
    ArgumentInternalDependencyResolver, ColumnGetter, ColumnGetterList, ColumnValueMassager,
    ColumnValueMassagerInterface, DependencyType, DependencyValue, ExternalDependency,
    ExternalDependencyValue, ExternalDependencyValues, Id, InternalDependency,
    InternalDependencyResolver, InternalDependencyValue, InternalDependencyValues,
    LiteralValueInternalDependencyResolver, ResolveInternalDependency, Where, WhereResolved,
    WheresResolved,
};
pub use crate::error::{Error, Result};
pub use crate::hash::get_hash;
pub use crate::inscribe::json_from_response;
pub use crate::operation::OperationType;
pub use crate::parser::{lex, parse, LexError, ParseError, ParseOrLexError, Token};
pub use crate::plan::{FieldPlan, QueryPlan};
pub use crate::positions::{CharsEmitter, Location, PositionsTracker};
pub use crate::request::{
    Argument, Directive, Document, ExecutableDefinition, Field as SelectionField,
    FieldBuilder as SelectionFieldBuilder, FragmentDefinition, FragmentSpread, InlineFragment,
    OperationDefinition, OperationDefinitionBuilder, Request, Selection, Value,
};
pub use crate::resolve::{
    Carver, CarverList, CarverOrPopulator, DateCarver, EnumValueCarver, EnumValueCarverList,
    FieldResolver, IdCarver, IntCarver, OptionalEnumValueCarver, OptionalFloatCarver,
    OptionalIntCarver, OptionalPopulator, OptionalPopulatorInterface, OptionalStringCarver,
    OptionalUnionOrInterfaceTypePopulator, OptionalValuePopulator, OptionalValuesPopulator,
    Populator, PopulatorInterface, PopulatorList, PopulatorListInterface, StringCarver,
    TimestampCarver, TypeDepluralizer, UnionOrInterfaceTypePopulator,
    UnionOrInterfaceTypePopulatorList, ValuePopulator, ValuePopulatorList, ValuesPopulator,
};
pub use crate::response::{produce_response, ColumnSpec, Response, ResponseValue};
pub use crate::schema::{Schema, TypeOrUnionOrInterface, ValidationError};
pub use crate::smolstr::SmolStrSqlx;
pub use crate::string::singularize;
pub use crate::types::{
    builtin_types, float_type, id_type, string_type, BuiltInScalarType, DummyUnionTypenameField,
    Enum, Field as TypeField, FieldBuilder as TypeFieldBuilder, FieldInterface, Interface,
    InterfaceBuilder, InterfaceField, ObjectType, ObjectTypeBuilder, Param, ScalarType, StringType,
    Type, TypeFull, TypeInterface, TypeOrInterfaceField, Union,
};

pub use proc_macros::{enum_optional_string_massager, enum_string_massager, schema};
