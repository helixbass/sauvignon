use async_trait::async_trait;
use sqlx::{Pool, Postgres};
use tracing::{trace_span, Instrument};

use crate::{DependencyType, DependencyValue, Id, WhereResolved};

#[async_trait]
pub trait Database: Send + Sync {
    async fn get_column(
        &self,
        table_name: &str,
        column_name: &str,
        id: &str,
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
}

pub struct PostgresDatabase {
    pub pool: Pool<Postgres>,
}

impl PostgresDatabase {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Database for PostgresDatabase {
    async fn get_column(
        &self,
        table_name: &str,
        column_name: &str,
        id: &str,
        id_column_name: &str,
        dependency_type: DependencyType,
    ) -> DependencyValue {
        match dependency_type {
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for id_column()
            DependencyType::Id => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (Id,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch ID column"))
                    .await
                    .unwrap();
                DependencyValue::Id(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for enum_column()
            DependencyType::String => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (String,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch string column"))
                    .await
                    .unwrap();
                DependencyValue::String(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for optional int column
            DependencyType::OptionalInt => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (Option<i32>,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch optional int column"))
                    .await
                    .unwrap();
                DependencyValue::OptionalInt(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for optional float column
            DependencyType::OptionalFloat => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (Option<f64>,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch optional float column"))
                    .await
                    .unwrap();
                DependencyValue::OptionalFloat(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for optional string column (including for optional_enum_column()
            // and optional_string_column())
            DependencyType::OptionalString => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (Option<String>,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch optional string column"))
                    .await
                    .unwrap();
                DependencyValue::OptionalString(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for timestamp column
            DependencyType::Timestamp => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (jiff_sqlx::Timestamp,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch timestamp column"))
                    .await
                    .unwrap();
                DependencyValue::Timestamp(column_value.to_jiff())
            }
            DependencyType::OptionalId => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (Option<Id>,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch optional ID column"))
                    .await
                    .unwrap();
                DependencyValue::OptionalId(column_value)
            }
            // TODO: add test (in this repo vs in swapi-sauvignon)
            // for int_column()
            DependencyType::Int => {
                // TODO: should check that table names and column names can never be SQL injection?
                let query = format!(
                    "SELECT {} FROM {} WHERE {} = $1",
                    column_name, table_name, id_column_name,
                );
                let (column_value,): (i32,) = sqlx::query_as(&query)
                    .bind(id)
                    .fetch_one(&self.pool)
                    .instrument(trace_span!("fetch ID column"))
                    .await
                    .unwrap();
                DependencyValue::Int(column_value)
            }
            _ => unimplemented!(),
        }
    }

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
                let mut query = sqlx::query_as::<_, (Id,)>(&query);
                for where_ in wheres {
                    // TODO: this is punting on where's specifying
                    // values
                    query = match &where_.value {
                        DependencyValue::Id(id) => query.bind(id),
                        DependencyValue::String(str) => query.bind(str),
                        _ => unimplemented!(),
                    };
                }
                let rows = query
                    .fetch_all(&self.pool)
                    .instrument(trace_span!("fetch id column list"))
                    .await
                    .unwrap();
                rows.into_iter()
                    .map(|(column_value,)| DependencyValue::Id(column_value))
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
                let mut query = sqlx::query_as::<_, (String,)>(&query);
                for where_ in wheres {
                    // TODO: this is punting on where's specifying
                    // values
                    query = match &where_.value {
                        DependencyValue::Id(id) => query.bind(id),
                        DependencyValue::String(str) => query.bind(str),
                        _ => unimplemented!(),
                    };
                }
                let rows = query
                    .fetch_all(&self.pool)
                    .instrument(trace_span!("fetch string column list"))
                    .await
                    .unwrap();
                rows.into_iter()
                    .map(|(column_value,)| DependencyValue::String(column_value))
                    .collect()
            }
            _ => unreachable!(),
        }
    }
}
