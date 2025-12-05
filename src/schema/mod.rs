use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use sqlx::{Pool, Postgres};

use crate::{
    builtin_types, fields_in_progress_new, CarverOrPopulator, DependencyType, DependencyValue,
    Error, ExternalDependencyValues, FieldPlan, FieldsInProgress, Id, InProgress,
    InProgressRecursing, InProgressRecursingList, IndexMap, InternalDependencyResolver,
    InternalDependencyValues, Populator, QueryPlan, Request, Response, ResponseValue,
    ResponseValueOrInProgress, Result as SauvignonResult, Type, TypeInterface, Union,
};

pub struct Schema {
    pub types: HashMap<String, Type>,
    pub query_type_name: String,
    builtin_types: HashMap<String, Type>,
    pub unions: HashMap<String, Union>,
}

impl Schema {
    pub fn try_new(types: Vec<Type>, unions: Vec<Union>) -> SauvignonResult<Self> {
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
            unions: HashMap::from_iter(
                unions
                    .into_iter()
                    .map(|union| (union.name.to_owned(), union)),
            ),
        })
    }

    pub async fn request(&self, request: Request, db_pool: &Pool<Postgres>) -> Response {
        let response = compute_response(self, &request, db_pool).await;
        Response { data: response }
    }

    pub fn query_type(&self) -> &Type {
        &self.types[&self.query_type_name]
    }

    pub fn maybe_type(&self, name: &str) -> Option<&Type> {
        self.types
            .get(name)
            .or_else(|| self.builtin_types.get(name))
    }

    pub fn type_(&self, name: &str) -> &Type {
        self.maybe_type(name).unwrap()
    }

    pub fn type_or_union_or_interface<'a>(&'a self, name: &str) -> TypeOrUnionOrInterface<'a> {
        if let Some(type_) = self.maybe_type(name) {
            return TypeOrUnionOrInterface::Type(type_);
        }
        if let Some(union) = self.unions.get(name) {
            return TypeOrUnionOrInterface::Union(union);
        }
        panic!()
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
        let ret = progress_fields(fields_in_progress, db_pool, schema).await;
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
    schema: &'a Schema,
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
                                to_recursing_after_populating(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                    populator,
                                    field_plan.field_type.type_.name(),
                                    field_plan,
                                )
                            }
                            CarverOrPopulator::PopulatorList(populator) => {
                                let populated = populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                let type_name = field_plan.field_type.type_.name();
                                let fields_in_progress = populated
                                    .iter()
                                    .map(|populated| {
                                        fields_in_progress_new(
                                            &field_plan.selection_set_by_type.as_ref().unwrap()
                                                [type_name],
                                            &populated,
                                        )
                                    })
                                    .collect();
                                ResponseValueOrInProgress::InProgressRecursingList(
                                    InProgressRecursingList::new(
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
                            CarverOrPopulator::UnionOrInterfaceTypePopulator(
                                type_populator,
                                populator,
                            ) => {
                                let type_name = type_populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                to_recursing_after_populating(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                    populator,
                                    &type_name,
                                    field_plan,
                                )
                            }
                            CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                                type_populator,
                                populator,
                            ) => unimplemented!(),
                        }
                    }
                    ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                        field_plan,
                        populated,
                        selection,
                    }) => {
                        let (is_done, fields_in_progress) =
                            progress_fields(selection, db_pool, schema).await;

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
                    ResponseValueOrInProgress::InProgressRecursingList(
                        InProgressRecursingList {
                            field_plan,
                            populated,
                            selections,
                        },
                    ) => {
                        let mut progressed = vec![];
                        let mut are_all_done = true;
                        for selection in selections {
                            let (is_done, fields_in_progress) =
                                progress_fields(selection, db_pool, schema).await;
                            if !is_done {
                                are_all_done = false;
                            }
                            progressed.push(fields_in_progress);
                        }
                        if are_all_done {
                            ResponseValueOrInProgress::ResponseValue(progressed.into())
                        } else {
                            ResponseValueOrInProgress::InProgressRecursingList(
                                InProgressRecursingList {
                                    field_plan,
                                    populated,
                                    selections: progressed,
                                },
                            )
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
                    match internal_dependency.type_ {
                        DependencyType::Id => {
                            // TODO: should check that table names and column names can never be SQL injection?
                            let query = format!(
                                "SELECT {} FROM {} WHERE id = $1",
                                column_getter.column_name, column_getter.table_name
                            );
                            let (column_value,): (Id,) = sqlx::query_as(&query)
                                .bind(row_id)
                                .fetch_one(db_pool)
                                .await
                                .unwrap();
                            DependencyValue::Id(column_value)
                        }
                        DependencyType::String => {
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
                        _ => unimplemented!(),
                    }
                }
                InternalDependencyResolver::LiteralValue(literal_value) => literal_value.0.clone(),
                InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
                    // TODO: same as above, sql injection?
                    let query = format!(
                        "SELECT {} FROM {}",
                        column_getter_list.column_name, column_getter_list.table_name
                    );
                    let rows = sqlx::query_as::<_, (Id,)>(&query)
                        .fetch_all(db_pool)
                        .await
                        .unwrap();
                    DependencyValue::List(
                        rows.into_iter()
                            .map(|(column_value,)| DependencyValue::Id(column_value))
                            .collect(),
                    )
                }
                _ => unimplemented!(),
            },
        )
        .unwrap();
    }

    ret
}

fn to_recursing_after_populating<'a>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &Box<dyn Populator>,
    resolved_concrete_type_name: &str,
    field_plan: &'a FieldPlan<'a>,
) -> ResponseValueOrInProgress<'a> {
    let populated = populator.populate(&external_dependency_values, &internal_dependency_values);
    let fields_in_progress = fields_in_progress_new(
        &field_plan.selection_set_by_type.as_ref().unwrap()[resolved_concrete_type_name],
        &populated,
    );
    ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing::new(
        field_plan,
        populated,
        fields_in_progress,
    ))
}

pub enum TypeOrUnionOrInterface<'a> {
    Type(&'a Type),
    Union(&'a Union),
    // Interface,
}

impl<'a> TypeOrUnionOrInterface<'a> {
    pub fn all_concrete_type_names(&self) -> HashSet<String> {
        match self {
            Self::Type(type_) => HashSet::from_iter([type_.name().to_owned()]),
            Self::Union(union) => HashSet::from_iter(union.types.iter().cloned()),
        }
    }
}
