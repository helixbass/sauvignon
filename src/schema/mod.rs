use std::collections::HashMap;
use std::pin::Pin;

use sqlx::{Pool, Postgres};

use crate::{
    builtin_types, fields_in_progress_new, CarverOrPopulator, DependencyValue, Error,
    ExternalDependencyValues, FieldPlan, FieldsInProgress, InProgress, InProgressRecursing,
    IndexMap, InternalDependencyResolver, InternalDependencyValues, QueryPlan, Request, Response,
    ResponseValue, ResponseValueOrInProgress, Result as SauvignonResult, Type, TypeInterface,
};

pub struct Schema {
    pub types: HashMap<String, Type>,
    pub query_type_name: String,
    builtin_types: HashMap<String, Type>,
}

impl Schema {
    pub fn try_new(types: Vec<Type>) -> SauvignonResult<Self> {
        let query_type_index = types
            .iter()
            .position(|type_| type_.is_query_type())
            .ok_or_else(|| Error::NoQueryTypeSpecified)?;
        let query_type_name = types[query_type_index].name().to_owned();

        Ok(Self {
            types: HashMap::from_iter(
                types
                    .into_iter()
                    .map(|type_| (type_.name().to_owned(), type_)),
            ),
            query_type_name,
            builtin_types: builtin_types(),
        })
    }

    pub async fn request(&self, request: Request, db_pool: &Pool<Postgres>) -> Response {
        let response = compute_response(self, &request, db_pool).await;
        Response { data: response }
    }

    pub fn query_type(&self) -> &Type {
        &self.types[&self.query_type_name]
    }

    pub fn get_type(&self, name: &str) -> &Type {
        self.types
            .get(name)
            .or_else(|| self.builtin_types.get(name))
            .unwrap()
    }
}

async fn compute_response(
    schema: &Schema,
    request: &Request,
    db_pool: &Pool<Postgres>,
) -> ResponseValue {
    let query_plan = QueryPlan::new(&request, schema);
    let response_in_progress = query_plan.initial_response_in_progress();
    let mut fields_in_progress = response_in_progress.fields;
    loop {
        let ret = progress_fields(fields_in_progress, db_pool).await;
        let is_done = ret.0;
        fields_in_progress = ret.1;
        if is_done {
            return fields_in_progress.into();
        }
    }
}

fn progress_fields<'a>(
    fields_in_progress: FieldsInProgress<'a>,
    db_pool: &'a Pool<Postgres>,
) -> Pin<Box<dyn Future<Output = (bool, FieldsInProgress<'a>)> + 'a>> {
    Box::pin(async move {
        let is_done = fields_in_progress
            .values()
            .all(|field| matches!(field, ResponseValueOrInProgress::ResponseValue(_)));
        if is_done {
            return (true, fields_in_progress);
        }

        let mut progressed = IndexMap::new();
        for (field_name, response_value_or_in_progress) in fields_in_progress {
            progressed.insert(
                field_name,
                match response_value_or_in_progress {
                    ResponseValueOrInProgress::ResponseValue(response_value) => {
                        ResponseValueOrInProgress::ResponseValue(response_value)
                    }
                    ResponseValueOrInProgress::InProgress(InProgress {
                        field_plan,
                        external_dependency_values,
                    }) => {
                        let internal_dependency_values = populate_internal_dependencies(
                            field_plan,
                            &external_dependency_values,
                            db_pool,
                        )
                        .await;
                        match &field_plan.field_type.resolver.carver_or_populator {
                            CarverOrPopulator::Populator(populator) => {
                                let mut populated = ExternalDependencyValues::default();
                                populator.populate(
                                    &mut populated,
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                let fields_in_progress = fields_in_progress_new(
                                    field_plan.selection_set.as_ref().unwrap(),
                                    &populated,
                                );
                                ResponseValueOrInProgress::InProgressRecursing(
                                    InProgressRecursing::new(
                                        field_plan,
                                        populated,
                                        fields_in_progress,
                                    ),
                                )
                            }
                            CarverOrPopulator::Carver(carver) => {
                                ResponseValueOrInProgress::ResponseValue(carver.carve(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                ))
                            }
                        }
                    }
                    ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                        field_plan,
                        populated,
                        selection,
                    }) => {
                        let (is_done, fields_in_progress) =
                            progress_fields(selection, db_pool).await;

                        if is_done {
                            ResponseValueOrInProgress::ResponseValue(fields_in_progress.into())
                        } else {
                            ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                                field_plan,
                                populated,
                                selection: fields_in_progress,
                            })
                        }
                    }
                },
            );
        }

        (false, progressed)
    })
}

async fn populate_internal_dependencies(
    field_plan: &FieldPlan<'_>,
    external_dependency_values: &ExternalDependencyValues,
    db_pool: &Pool<Postgres>,
) -> InternalDependencyValues {
    let mut ret = InternalDependencyValues::new();
    for internal_dependency in field_plan.field_type.resolver.internal_dependencies.iter() {
        ret.insert(
            internal_dependency.name.clone(),
            match &internal_dependency.resolver {
                InternalDependencyResolver::ColumnGetter(column_getter) => {
                    let row_id = match external_dependency_values.get("id").unwrap() {
                        DependencyValue::Id(id) => id,
                        _ => unreachable!(),
                    };
                    // TODO: should check that table names and column names can never be SQL injection?
                    let query = format!(
                        "SELECT {} FROM {} WHERE id = $1",
                        column_getter.column_name, column_getter.table_name
                    );
                    let (column_value,): (String,) = sqlx::query_as(&query)
                        .bind(row_id)
                        .fetch_one(db_pool)
                        .await
                        .unwrap();
                    DependencyValue::String(column_value)
                }
                InternalDependencyResolver::LiteralValue(literal_value) => literal_value.0.clone(),
                _ => unimplemented!(),
            },
        )
        .unwrap();
    }

    ret
}
