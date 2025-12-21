use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use smol_str::SmolStr;
use sqlx::{postgres::PgValueRef, Decode, Pool, Postgres, QueryBuilder, Row};
use squalid::_d;
use tracing::{instrument, trace_span, Instrument};

use crate::{
    ColumnSpec, ColumnValueMassager, DependencyType, DependencyValue, Id, IndexMap, SmolStrSqlx,
    WhereResolved,
};

pub enum Database {
    Postgres(PostgresDatabase),
    Dyn(Box<dyn DatabaseInterface>),
}

impl Database {
    pub fn as_postgres(&self) -> &PostgresDatabase {
        match self {
            Self::Postgres(database) => database,
            _ => panic!("expected postgres"),
        }
    }
}

impl From<PostgresDatabase> for Database {
    fn from(value: PostgresDatabase) -> Self {
        Self::Postgres(value)
    }
}

#[async_trait]
impl DatabaseInterface for Database {
    async fn get_column(
        &self,
        table_name: &str,
        column_name: &str,
        id: &Id,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue {
        match self {
            Self::Postgres(database) => {
                database
                    .get_column(table_name, column_name, id, id_column_name, dependency_type)
                    .await
            }
            Self::Dyn(database) => {
                database
                    .get_column(table_name, column_name, id, id_column_name, dependency_type)
                    .await
            }
        }
    }

    async fn get_column_list(
        &self,
        table_name: &str,
        column_name: &str,
        dependency_type: DependencyType,
        wheres: &[WhereResolved],
    ) -> Vec<DependencyValue> {
        match self {
            Self::Postgres(database) => {
                database
                    .get_column_list(table_name, column_name, dependency_type, wheres)
                    .await
            }
            Self::Dyn(database) => {
                database
                    .get_column_list(table_name, column_name, dependency_type, wheres)
                    .await
            }
        }
    }

    fn get_column_sync(
        &self,
        column_token: ColumnToken,
        id: &Id,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue {
        match self {
            Self::Postgres(database) => {
                database.get_column_sync(column_token, id, id_column_name, dependency_type)
            }
            Self::Dyn(database) => {
                database.get_column_sync(column_token, id, id_column_name, dependency_type)
            }
        }
    }

    fn get_column_list_sync(
        &self,
        column_token: ColumnToken,
        dependency_type: DependencyType,
        wheres: &[WhereResolved],
    ) -> Vec<DependencyValue> {
        match self {
            Self::Postgres(database) => {
                database.get_column_list_sync(column_token, dependency_type, wheres)
            }
            Self::Dyn(database) => {
                database.get_column_list_sync(column_token, dependency_type, wheres)
            }
        }
    }

    fn is_sync(&self) -> bool {
        match self {
            Self::Postgres(database) => database.is_sync(),
            Self::Dyn(database) => database.is_sync(),
        }
    }

    fn column_tokens(&self) -> Option<&'static ColumnTokens> {
        match self {
            Self::Postgres(database) => database.column_tokens(),
            Self::Dyn(database) => database.column_tokens(),
        }
    }
}

#[async_trait]
pub trait DatabaseInterface: Send + Sync {
    async fn get_column(
        &self,
        table_name: &str,
        column_name: &str,
        id: &Id,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue;

    async fn get_column_list(
        &self,
        table_name: &str,
        column_name: &str,
        dependency_type: DependencyType,
        wheres: &[WhereResolved],
    ) -> Vec<DependencyValue>;

    fn get_column_sync(
        &self,
        column_token: ColumnToken,
        id: &Id,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue;

    fn get_column_list_sync(
        &self,
        column_token: ColumnToken,
        dependency_type: DependencyType,
        wheres: &[WhereResolved],
    ) -> Vec<DependencyValue>;

    fn is_sync(&self) -> bool;

    fn column_tokens(&self) -> Option<&'static ColumnTokens>;
}

pub struct PostgresDatabase {
    pub pool: Pool<Postgres>,
    pub massagers: IndexMap<SmolStr, IndexMap<SmolStr, ColumnValueMassager>>,
}

impl PostgresDatabase {
    pub fn new(pool: Pool<Postgres>, massagers: Vec<PostgresColumnMassager>) -> Self {
        Self {
            pool,
            massagers: {
                let mut ret: IndexMap<SmolStr, IndexMap<SmolStr, ColumnValueMassager>> = _d();
                for massager in massagers {
                    let for_this_table = ret.entry(massager.table_name).or_default();
                    if for_this_table.contains_key(&massager.column_name) {
                        panic!("Already saw column");
                    }
                    for_this_table.insert(massager.column_name, massager.massager);
                }
                ret
            },
        }
    }

    pub fn to_dependency_value(
        &self,
        column_value: PgValueRef<'_>,
        dependency_type: DependencyType,
        massager: Option<&ColumnValueMassager>,
    ) -> DependencyValue {
        match dependency_type {
            DependencyType::Id => {
                assert!(massager.is_none());
                DependencyValue::Id(Id::Int(
                    <i32 as Decode<Postgres>>::decode(column_value).unwrap(),
                ))
            }
            DependencyType::String => match massager {
                None => DependencyValue::String(
                    <SmolStrSqlx as Decode<Postgres>>::decode(column_value)
                        .unwrap()
                        .0,
                ),
                Some(massager) => {
                    DependencyValue::String(massager.as_string().massage(column_value).unwrap())
                }
            },
            DependencyType::OptionalInt => {
                assert!(massager.is_none());
                DependencyValue::OptionalInt(
                    <Option<i32> as Decode<Postgres>>::decode(column_value).unwrap(),
                )
            }
            DependencyType::OptionalFloat => {
                assert!(massager.is_none());
                DependencyValue::OptionalFloat(
                    <Option<f64> as Decode<Postgres>>::decode(column_value).unwrap(),
                )
            }
            DependencyType::OptionalString => match massager {
                None => DependencyValue::OptionalString(
                    <Option<SmolStrSqlx> as Decode<Postgres>>::decode(column_value)
                        .unwrap()
                        .map(|column_value| column_value.0),
                ),
                Some(massager) => DependencyValue::OptionalString(
                    massager.as_optional_string().massage(column_value).unwrap(),
                ),
            },
            DependencyType::Timestamp => {
                assert!(massager.is_none());
                DependencyValue::Timestamp(
                    <jiff_sqlx::Timestamp as Decode<Postgres>>::decode(column_value)
                        .unwrap()
                        .to_jiff(),
                )
            }
            DependencyType::OptionalId => {
                assert!(massager.is_none());
                DependencyValue::OptionalId(
                    <Option<i32> as Decode<Postgres>>::decode(column_value)
                        .unwrap()
                        .map(|column_value| Id::Int(column_value)),
                )
            }
            DependencyType::Int => {
                assert!(massager.is_none());
                DependencyValue::Int(<i32 as Decode<Postgres>>::decode(column_value).unwrap())
            }
            DependencyType::Date => {
                assert!(massager.is_none());
                DependencyValue::Date(
                    <NaiveDate as Decode<Postgres>>::decode(column_value).unwrap(),
                )
            }
            _ => unimplemented!(),
        }
    }

    pub async fn get_columns(
        &self,
        table_name: &str,
        columns: &[ColumnSpec],
        id: &Id,
        id_column_name: &str,
    ) -> HashMap<SmolStr, DependencyValue> {
        let mut query_builder = QueryBuilder::default();
        query_builder.push("SELECT ");
        columns.into_iter().enumerate().for_each(|(index, column)| {
            query_builder.push(&column.name);
            if index != columns.len() - 1 {
                query_builder.push(", ");
            }
        });
        query_builder.push(" FROM ");
        query_builder.push(table_name);
        query_builder.push(" WHERE ");
        query_builder.push(id_column_name);
        query_builder.push(" = ");
        query_builder.push_bind(id.as_int());
        let row = query_builder.build().fetch_one(&self.pool).await.unwrap();
        // TODO: column massagers?
        columns
            .into_iter()
            .map(|column| {
                let column_value = row.try_get_raw(&*column.name).unwrap();
                (
                    column.name.clone(),
                    self.to_dependency_value(
                        column_value,
                        column.dependency_type,
                        self.massagers
                            .get(table_name)
                            .and_then(|table| table.get(&column.name)),
                    ),
                )
            })
            .collect()
    }
}

#[async_trait]
impl DatabaseInterface for PostgresDatabase {
    #[instrument(level = "trace", skip(self))]
    async fn get_column(
        &self,
        table_name: &str,
        column_name: &str,
        id: &Id,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue {
        // TODO: should check that table names and column names can never be SQL injection?
        let query = format!(
            "SELECT {} FROM {} WHERE {} = $1",
            column_name, table_name, id_column_name,
        );
        let row = sqlx::query(&query)
            .bind(id.as_int())
            .fetch_one(&self.pool)
            .instrument(trace_span!("fetch column"))
            .await
            .unwrap();
        // TODO: add tests (in this repo vs in swapi-sauvignon)
        // for id_column(), enum_column(), optional int column,
        // optional float column, optional string column
        // (including for optional_enum_column() and optional_string_column()),
        // timestamp column, int_column(), date column
        self.to_dependency_value(
            row.try_get_raw(column_name).unwrap(),
            dependency_type,
            self.massagers
                .get(table_name)
                .and_then(|table| table.get(column_name)),
        )
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_column_list(
        &self,
        table_name: &str,
        column_name: &str,
        dependency_type: DependencyType,
        wheres: &[WhereResolved],
    ) -> Vec<DependencyValue> {
        match dependency_type {
            DependencyType::ListOfIds => {
                // TODO: same as above, sql injection?
                let query = format!(
                    "SELECT {} FROM {}{}",
                    column_name,
                    table_name,
                    if wheres.is_empty() {
                        "".to_owned()
                    } else {
                        format!(
                            " WHERE {}",
                            wheres
                                .iter()
                                .enumerate()
                                .map(|(index, where_)| {
                                    format!("{} = ${}", where_.column_name, index + 1)
                                })
                                .collect::<String>()
                        )
                    }
                );
                let mut query = sqlx::query_as::<_, (i32,)>(&query);
                for where_ in wheres {
                    // TODO: this is punting on where's specifying
                    // values
                    query = match &where_.value {
                        DependencyValue::Id(id) => query.bind(id.as_int()),
                        DependencyValue::String(str) => query.bind(SmolStrSqlx(str.clone())),
                        _ => unimplemented!(),
                    };
                }
                let rows = query
                    .fetch_all(&self.pool)
                    .instrument(trace_span!("fetch id column list"))
                    .await
                    .unwrap();
                rows.into_iter()
                    .map(|(column_value,)| DependencyValue::Id(Id::Int(column_value)))
                    .collect()
            }
            DependencyType::ListOfStrings => {
                // TODO: same as above, sql injection?
                let query = format!(
                    "SELECT {} FROM {}{}",
                    column_name,
                    table_name,
                    if wheres.is_empty() {
                        "".to_owned()
                    } else {
                        format!(
                            " WHERE {}",
                            wheres
                                .iter()
                                .enumerate()
                                .map(|(index, where_)| {
                                    format!("{} = ${}", where_.column_name, index + 1)
                                })
                                .collect::<String>()
                        )
                    }
                );
                match self
                    .massagers
                    .get(table_name)
                    .and_then(|table| table.get(column_name))
                {
                    None => {
                        let mut query = sqlx::query_as::<_, (SmolStrSqlx,)>(&query);
                        for where_ in wheres {
                            query = match &where_.value {
                                DependencyValue::Id(id) => query.bind(id.as_int()),
                                DependencyValue::String(str) => {
                                    query.bind(SmolStrSqlx(str.clone()))
                                }
                                _ => unimplemented!(),
                            };
                        }
                        let rows = query
                            .fetch_all(&self.pool)
                            .instrument(trace_span!("fetch string column list"))
                            .await
                            .unwrap();
                        rows.into_iter()
                            .map(|(column_value,)| DependencyValue::String(column_value.0))
                            .collect()
                    }
                    Some(massager) => {
                        let massager = massager.as_string();
                        let mut query = sqlx::query(&query);
                        for where_ in wheres {
                            query = match &where_.value {
                                DependencyValue::Id(id) => query.bind(id.as_int()),
                                DependencyValue::String(str) => {
                                    query.bind(SmolStrSqlx(str.clone()))
                                }
                                _ => unimplemented!(),
                            };
                        }
                        let rows = query
                            .fetch_all(&self.pool)
                            .instrument(trace_span!("fetch string column list"))
                            .await
                            .unwrap();
                        rows.into_iter()
                            .map(|row| {
                                DependencyValue::String(
                                    massager
                                        .massage(row.try_get_raw(column_name).unwrap())
                                        .unwrap(),
                                )
                            })
                            .collect()
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn get_column_sync(
        &self,
        _column_token: ColumnToken,
        _id: &Id,
        _id_column_name: &str,
        _dependency_type: DependencyType,
    ) -> DependencyValue {
        unreachable!()
    }

    fn get_column_list_sync(
        &self,
        _column_token: ColumnToken,
        _dependency_type: DependencyType,
        _wheres: &[WhereResolved],
    ) -> Vec<DependencyValue> {
        unreachable!()
    }

    fn is_sync(&self) -> bool {
        false
    }

    fn column_tokens(&self) -> Option<&'static ColumnTokens> {
        None
    }
}

pub struct PostgresColumnMassager {
    pub table_name: SmolStr,
    pub column_name: SmolStr,
    pub massager: ColumnValueMassager,
}

impl PostgresColumnMassager {
    pub fn new(table_name: SmolStr, column_name: SmolStr, massager: ColumnValueMassager) -> Self {
        Self {
            table_name,
            column_name,
            massager,
        }
    }
}

#[derive(Copy, Clone)]
pub struct ColumnToken {
    pub table: u32,
    pub column: u32,
}

pub type ColumnTokens = HashMap<SmolStr, HashMap<SmolStr, ColumnToken>>;
