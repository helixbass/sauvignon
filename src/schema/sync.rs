use indexmap::IndexMap;
use smol_str::SmolStr;
use tracing::instrument;

use crate::{
    get_internal_dependency_values_synchronous, CarverOrPopulator, ColumnGetter, ColumnGetterList,
    Database, DatabaseInterface, ExternalDependencyValues, FieldPlan, FieldResolver,
    InternalDependency, InternalDependencyResolver, InternalDependencyValues,
    PopulatorListInterface, QueryPlan, ResponseValue, Schema, WhereResolved, WheresResolved,
};

#[instrument(level = "trace", skip(schema, database, query_plan))]
pub fn compute_sync_response(
    schema: &Schema,
    database: &Database,
    query_plan: &QueryPlan,
) -> ResponseValue {
    ResponseValue::Map(compute_sync_response_fields(
        &query_plan.field_plans,
        schema,
        database,
        ExternalDependencyValues::Empty,
    ))
}

#[instrument(
    level = "trace",
    skip(field_plans, schema, database, external_dependency_values)
)]
fn compute_sync_response_fields(
    field_plans: &IndexMap<SmolStr, FieldPlan<'_>>,
    schema: &Schema,
    database: &Database,
    external_dependency_values: ExternalDependencyValues,
) -> IndexMap<SmolStr, ResponseValue> {
    field_plans
        .into_iter()
        .map(|(name, field_plan)| {
            (name.clone(), {
                let resolver = &field_plan.field_type.resolver;
                if let Some((internal_dependency, column_getter)) =
                    is_simple_column_getter(resolver)
                {
                    let value = database.get_column_sync(
                        field_plan.column_token.unwrap(),
                        external_dependency_values.get("id").unwrap().as_id(),
                        &column_getter.id_column_name,
                        &internal_dependency.type_,
                    );
                    match &resolver.carver_or_populator {
                        CarverOrPopulator::Carver(carver) => carver.carve(
                            &ExternalDependencyValues::Empty,
                            &InternalDependencyValues::Single(
                                internal_dependency.name.clone(),
                                value,
                            ),
                        ),
                        CarverOrPopulator::Populator(populator) => {
                            let populator = populator.as_values();
                            let keys = &populator.keys;
                            assert_eq!(keys.len(), 1);
                            let first_key = keys.iter().next().unwrap();
                            assert_eq!(first_key.0, &internal_dependency.name);
                            assert_eq!(first_key.1, "id");
                            ResponseValue::Map(compute_sync_response_fields(
                                &field_plan.selection_set_by_type.as_ref().unwrap()
                                    [field_plan.field_type.type_.name()],
                                schema,
                                database,
                                ExternalDependencyValues::Single("id".into(), value),
                            ))
                        }
                        _ => unimplemented!(),
                    }
                } else if let Some((internal_dependency, column_getter_list)) =
                    is_simple_column_getter_list(resolver)
                {
                    // TODO: could return iterator instead of Vec
                    // from .get_column_list_sync() to avoid
                    // allocating giant Vec?
                    let list = database.get_column_list_sync(
                        field_plan.column_token.unwrap(),
                        &internal_dependency.type_,
                        // TODO: optimize this to not allocate Vec
                        // for single-where case
                        &column_getter_list
                            .wheres
                            .iter()
                            .map(|where_| {
                                WhereResolved::new(
                                    where_.column_name.clone(),
                                    // TODO: this is punting on where's specifying
                                    // values
                                    external_dependency_values.get("id").unwrap().clone(),
                                )
                            })
                            .collect::<WheresResolved>(),
                    );
                    match &resolver.carver_or_populator {
                        CarverOrPopulator::PopulatorList(populator) => {
                            let populator = populator.as_value();
                            ResponseValue::List(
                                list.into_iter()
                                    .map(|value| {
                                        ResponseValue::Map(compute_sync_response_fields(
                                            &field_plan.selection_set_by_type.as_ref().unwrap()
                                                [field_plan.field_type.type_.name()],
                                            schema,
                                            database,
                                            ExternalDependencyValues::Single(
                                                populator.singular.clone(),
                                                value,
                                            ),
                                        ))
                                    })
                                    .collect(),
                            )
                        }
                        _ => unimplemented!(),
                    }
                } else {
                    let internal_dependency_values = get_internal_dependency_values_synchronous(
                        field_plan,
                        &external_dependency_values,
                        schema,
                        database,
                    );
                    match &resolver.carver_or_populator {
                        CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                            type_populator,
                            populator,
                        ) => {
                            let type_names = type_populator
                                .populate(&external_dependency_values, &internal_dependency_values);
                            let populated = populator
                                .populate(&external_dependency_values, &internal_dependency_values);

                            ResponseValue::List(
                                type_names
                                    .into_iter()
                                    .zip(populated)
                                    .map(|(type_name, populated)| {
                                        ResponseValue::Map(compute_sync_response_fields(
                                            &field_plan.selection_set_by_type.as_ref().unwrap()
                                                [&type_name],
                                            schema,
                                            database,
                                            populated,
                                        ))
                                    })
                                    .collect(),
                            )
                        }
                        _ => unimplemented!(),
                    }
                }
            })
        })
        .collect()
}

fn is_simple_column_getter(
    resolver: &FieldResolver,
) -> Option<(&InternalDependency, &ColumnGetter)> {
    if resolver.internal_dependencies.len() != 1 {
        return None;
    }
    let internal_dependency = &resolver.internal_dependencies[0];
    let InternalDependencyResolver::ColumnGetter(column_getter) = &internal_dependency.resolver
    else {
        return None;
    };
    Some((internal_dependency, column_getter))
}

fn is_simple_column_getter_list(
    resolver: &FieldResolver,
) -> Option<(&InternalDependency, &ColumnGetterList)> {
    if resolver.internal_dependencies.len() != 1 {
        return None;
    }
    let internal_dependency = &resolver.internal_dependencies[0];
    let InternalDependencyResolver::ColumnGetterList(column_getter_list) =
        &internal_dependency.resolver
    else {
        return None;
    };
    Some((internal_dependency, column_getter_list))
}
