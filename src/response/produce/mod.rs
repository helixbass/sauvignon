use futures::future;
use indexmap::IndexMap;
use itertools::{Either, Itertools};
use smol_str::{SmolStr, ToSmolStr};
use squalid::_d;
use tracing::{instrument, trace_span};

use crate::{
    Argument, CarverList, CarverOrPopulator, ColumnGetter, ColumnGetterList, Database,
    DependencyType, DependencyValue, ExternalDependencyValues, FieldPlan, Id, InternalDependency,
    InternalDependencyResolver, InternalDependencyValues, OptionalPopulator,
    OptionalPopulatorInterface, OptionalUnionOrInterfaceTypePopulator, Populator,
    PopulatorInterface, PopulatorList, PopulatorListInterface, QueryPlan, ResponseValue, Schema,
    Type, UnionOrInterfaceTypePopulator, UnionOrInterfaceTypePopulatorList, Value, WhereResolved,
    WheresResolved,
};

mod async_step;
mod chunk;

pub use async_step::ColumnSpec;
use async_step::{
    AsyncInstruction, AsyncInstructionSimple, AsyncInstructions, AsyncStep, AsyncStepColumn,
    AsyncSteps, DependencyNames, IsInternalDependenciesOf,
};
use chunk::Produced;

type IndexInProduced = usize;

#[instrument(level = "trace", skip(schema, database, query_plan))]
pub async fn produce_response(
    schema: &Schema,
    database: &Database,
    query_plan: &QueryPlan<'_>,
) -> ResponseValue {
    let mut produced: Vec<Produced> = _d();
    produced.push(Produced::NewRootObject);

    let mut current_async_instructions: AsyncInstructions<'_> = _d();
    make_progress_selection_set(
        &query_plan.field_plans,
        0,
        ExternalDependencyValues::Empty,
        &mut produced,
        &mut current_async_instructions,
        schema,
    );
    loop {
        if current_async_instructions.is_empty() {
            break;
        }

        let responses = future::join_all(current_async_instructions.iter().flat_map(
            |async_instruction| {
                match async_instruction {
                    AsyncInstruction::Simple(async_instruction) => Either::Left(
                        async_instruction
                            .steps
                            .iter()
                            .map(|step| step.run(database)),
                    ),
                    AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                        step,
                        ..
                    } => Either::Right([step.run(database)].into_iter()),
                }
            },
        ))
        .await;

        let mut next_async_instructions: AsyncInstructions<'_> = _d();
        let mut responses = responses.into_iter();
        current_async_instructions.into_iter().for_each(
            |async_instruction| match async_instruction {
                AsyncInstruction::Simple(async_instruction) => {
                    let mut internal_dependency_values = InternalDependencyValues::default();
                    for internal_dependency_index in 0..async_instruction.steps.len() {
                        internal_dependency_values
                            .insert(
                                async_instruction.internal_dependency_names
                                    [internal_dependency_index]
                                    .clone(),
                                responses.next().unwrap().into_dependency_value(),
                            )
                            .unwrap();
                    }
                    do_simple_async_instruction_follow(
                        async_instruction.is_internal_dependencies_of,
                        internal_dependency_values,
                        &mut produced,
                        &mut next_async_instructions,
                        schema,
                    );
                }
                AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                    step: _,
                    is_internal_dependencies_of,
                } => {
                    let mut column_values = responses.next().unwrap().into_dependency_value_map();
                    is_internal_dependencies_of.into_iter().for_each(
                        |(column_name, is_internal_dependencies_of)| {
                            let column_value = column_values.remove(&column_name).unwrap();
                            let internal_dependency_values: InternalDependencyValues =
                                [(column_name, column_value)].into_iter().collect();
                            is_internal_dependencies_of.into_iter().for_each(
                                |is_internal_dependencies_of| {
                                    do_simple_async_instruction_follow(
                                        is_internal_dependencies_of,
                                        internal_dependency_values.clone(),
                                        &mut produced,
                                        &mut next_async_instructions,
                                        schema,
                                    );
                                },
                            );
                        },
                    );
                }
            },
        );

        current_async_instructions = next_async_instructions;
    }

    produced.into()
}

#[instrument(
    level = "trace",
    skip(
        is_internal_dependencies_of,
        internal_dependency_values,
        produced,
        current_async_instructions,
        schema,
    )
)]
fn do_simple_async_instruction_follow<'a: 'b, 'b>(
    is_internal_dependencies_of: IsInternalDependenciesOf<'a>,
    internal_dependency_values: InternalDependencyValues,
    produced: &mut Vec<Produced>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    match is_internal_dependencies_of {
        IsInternalDependenciesOf::ObjectFieldListOfObjects {
            parent_object_index,
            index_of_field_in_object,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            populate_list(
                &external_dependency_values,
                &internal_dependency_values,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
        IsInternalDependenciesOf::ObjectFieldScalar {
            parent_object_index,
            index_of_field_in_object,
            carver,
            external_dependency_values,
            field_name,
        } => {
            produced.push(Produced::FieldScalar {
                parent_object_index,
                index_of_field_in_object,
                field_name: field_name.clone(),
                value: carver.carve(&external_dependency_values, &internal_dependency_values),
            });
        }
        IsInternalDependenciesOf::ObjectFieldObject {
            parent_object_index,
            index_of_field_in_object,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            populate_object(
                &external_dependency_values,
                &internal_dependency_values,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
        IsInternalDependenciesOf::ObjectFieldUnionOrInterfaceObject {
            parent_object_index,
            index_of_field_in_object,
            type_populator,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            populate_union_or_interface_object(
                &external_dependency_values,
                &internal_dependency_values,
                type_populator,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
        IsInternalDependenciesOf::ObjectFieldListOfScalars {
            parent_object_index,
            index_of_field_in_object,
            carver,
            external_dependency_values,
            field_name,
        } => {
            carve_list(
                &external_dependency_values,
                &internal_dependency_values,
                carver,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
            );
        }
        IsInternalDependenciesOf::ObjectFieldOptionalObject {
            parent_object_index,
            index_of_field_in_object,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            optionally_populate_object(
                &external_dependency_values,
                &internal_dependency_values,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
        IsInternalDependenciesOf::ObjectFieldOptionalUnionOrInterfaceObject {
            parent_object_index,
            index_of_field_in_object,
            type_populator,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            optionally_populate_union_or_interface_object(
                &external_dependency_values,
                &internal_dependency_values,
                type_populator,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
        IsInternalDependenciesOf::ObjectFieldListOfUnionOrInterfaceObjects {
            parent_object_index,
            index_of_field_in_object,
            type_populator,
            populator,
            external_dependency_values,
            field_name,
            field_plan,
        } => {
            populate_union_or_interface_list(
                &external_dependency_values,
                &internal_dependency_values,
                type_populator,
                populator,
                produced,
                parent_object_index,
                index_of_field_in_object,
                &field_name,
                field_plan,
                current_async_instructions,
                schema,
            );
        }
    }
}

#[instrument(
    level = "trace",
    skip(
        field_plans,
        external_dependency_values,
        produced,
        current_async_instructions,
        schema,
    )
)]
fn make_progress_selection_set<'a: 'b, 'b>(
    field_plans: &'a IndexMap<SmolStr, FieldPlan<'a>>,
    parent_object_index: usize,
    external_dependency_values: ExternalDependencyValues,
    produced: &mut Vec<Produced>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    field_plans.into_iter().enumerate().for_each(
        |(index_of_field_in_object, (field_name, field_plan))| {
            let can_resolve_all_internal_dependencies_synchronously = field_plan
                .field_type
                .resolver
                .internal_dependencies
                .iter()
                .all(|internal_dependency| {
                    internal_dependency.resolver.can_be_resolved_synchronously()
                });
            match can_resolve_all_internal_dependencies_synchronously {
                true => {
                    let internal_dependency_values: InternalDependencyValues = field_plan
                        .field_type
                        .resolver
                        .internal_dependencies
                        .iter()
                        .map(|internal_dependency| {
                            (
                                internal_dependency.name.clone(),
                                get_internal_dependency_value_synchronous(
                                    field_plan.arguments.as_ref(),
                                    &external_dependency_values,
                                    internal_dependency,
                                    schema,
                                ),
                            )
                        })
                        .collect();
                    match &field_plan.field_type.resolver.carver_or_populator {
                        CarverOrPopulator::Carver(carver) => {
                            produced.push(Produced::FieldScalar {
                                parent_object_index,
                                index_of_field_in_object,
                                field_name: field_name.clone(),
                                value: carver.carve(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                ),
                            });
                        }
                        CarverOrPopulator::Populator(populator) => {
                            populate_object(
                                &external_dependency_values,
                                &internal_dependency_values,
                                populator,
                                produced,
                                parent_object_index,
                                index_of_field_in_object,
                                field_name,
                                field_plan,
                                current_async_instructions,
                                schema,
                            )
                        }
                        CarverOrPopulator::PopulatorList(populator) => {
                            populate_list(
                                &external_dependency_values,
                                &internal_dependency_values,
                                populator,
                                produced,
                                parent_object_index,
                                index_of_field_in_object,
                                field_name,
                                field_plan,
                                current_async_instructions,
                                schema,
                            );
                        }
                        CarverOrPopulator::UnionOrInterfaceTypePopulator(type_populator, populator) => {
                            populate_union_or_interface_object(
                                &external_dependency_values,
                                &internal_dependency_values,
                                type_populator,
                                populator,
                                produced,
                                parent_object_index,
                                index_of_field_in_object,
                                field_name,
                                field_plan,
                                current_async_instructions,
                                schema,
                            )
                        }
                        CarverOrPopulator::CarverList(carver) => {
                            carve_list(
                                &external_dependency_values,
                                &internal_dependency_values,
                                carver,
                                produced,
                                parent_object_index,
                                index_of_field_in_object,
                                field_name,
                            )
                        }
                        _ => unimplemented!(),
                    }
                }
                false => {
                    match &field_plan
                            .field_type
                            .resolver
                            .carver_or_populator {
                        CarverOrPopulator::Carver(carver) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of: IsInternalDependenciesOf::ObjectFieldScalar {
                                    parent_object_index,
                                    carver,
                                    external_dependency_values: external_dependency_values
                                        .clone(),
                                    index_of_field_in_object,
                                    field_name: field_name.clone(),
                                },
                            }));
                        }
                        CarverOrPopulator::Populator(populator) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of: IsInternalDependenciesOf::ObjectFieldObject {
                                    parent_object_index,
                                    populator,
                                    external_dependency_values: external_dependency_values
                                        .clone(),
                                    index_of_field_in_object,
                                    field_name: field_name.clone(),
                                    field_plan,
                                },
                            }));
                        }
                        CarverOrPopulator::OptionalPopulator(populator) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of: IsInternalDependenciesOf::ObjectFieldOptionalObject {
                                    parent_object_index,
                                    populator,
                                    external_dependency_values: external_dependency_values
                                        .clone(),
                                    index_of_field_in_object,
                                    field_name: field_name.clone(),
                                    field_plan,
                                },
                            }));
                        }
                        CarverOrPopulator::UnionOrInterfaceTypePopulator(type_populator, populator) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of: IsInternalDependenciesOf::ObjectFieldUnionOrInterfaceObject {
                                    parent_object_index,
                                    type_populator,
                                    populator,
                                    external_dependency_values: external_dependency_values
                                        .clone(),
                                    index_of_field_in_object,
                                    field_name: field_name.clone(),
                                    field_plan,
                                },
                            }));
                        }
                        CarverOrPopulator::OptionalUnionOrInterfaceTypePopulator(type_populator, populator) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of: IsInternalDependenciesOf::ObjectFieldOptionalUnionOrInterfaceObject {
                                    parent_object_index,
                                    type_populator,
                                    populator,
                                    external_dependency_values: external_dependency_values
                                        .clone(),
                                    index_of_field_in_object,
                                    field_name: field_name.clone(),
                                    field_plan,
                                },
                            }));
                        }
                        CarverOrPopulator::PopulatorList(populator) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of:
                                    IsInternalDependenciesOf::ObjectFieldListOfObjects {
                                        parent_object_index,
                                        populator,
                                        external_dependency_values: external_dependency_values
                                            .clone(),
                                        index_of_field_in_object,
                                        field_name: field_name.clone(),
                                        field_plan,
                                    },
                            }));
                        }
                        CarverOrPopulator::CarverList(carver) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of:
                                    IsInternalDependenciesOf::ObjectFieldListOfScalars {
                                        parent_object_index,
                                        carver,
                                        external_dependency_values: external_dependency_values
                                            .clone(),
                                        index_of_field_in_object,
                                        field_name: field_name.clone(),
                                    },
                            }));
                        }
                        CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                            type_populator,
                            populator,
                        ) => {
                            let (steps, internal_dependency_names) = extract_dependency_steps(field_plan, &external_dependency_values);
                            current_async_instructions.push(AsyncInstruction::Simple(AsyncInstructionSimple {
                                steps,
                                internal_dependency_names,
                                is_internal_dependencies_of:
                                    IsInternalDependenciesOf::ObjectFieldListOfUnionOrInterfaceObjects {
                                        parent_object_index,
                                        index_of_field_in_object,
                                        type_populator,
                                        populator,
                                        external_dependency_values: external_dependency_values.clone(),
                                        field_name: field_name.clone(),
                                        field_plan,
                                    },
                            }));
                        }
                    }
                }
            }
        },
    );
}

fn extract_dependency_steps(
    field_plan: &FieldPlan<'_>,
    external_dependency_values: &ExternalDependencyValues,
) -> (AsyncSteps, DependencyNames) {
    field_plan
        .field_type
        .resolver
        .internal_dependencies
        .iter()
        .map(|internal_dependency| {
            (
                match &internal_dependency.resolver {
                    InternalDependencyResolver::ColumnGetter(column_getter) => column_getter_step(
                        column_getter,
                        internal_dependency,
                        &external_dependency_values,
                    ),
                    InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
                        column_getter_list_step(
                            column_getter_list,
                            internal_dependency,
                            &external_dependency_values,
                        )
                    }
                    _ => unreachable!(),
                },
                internal_dependency.name.clone(),
            )
        })
        .unzip()
}

fn column_getter_step(
    column_getter: &ColumnGetter,
    internal_dependency: &InternalDependency,
    external_dependency_values: &ExternalDependencyValues,
) -> AsyncStep {
    AsyncStep::Column(AsyncStepColumn {
        table_name: column_getter.table_name.clone(),
        column: ColumnSpec {
            name: column_getter.column_name.clone(),
            dependency_type: internal_dependency.type_,
        },
        id_column_name: column_getter.id_column_name.clone(),
        id: external_dependency_values
            .get("id")
            .unwrap()
            .as_id()
            .clone(),
    })
}

fn column_getter_list_step(
    column_getter_list: &ColumnGetterList,
    internal_dependency: &InternalDependency,
    external_dependency_values: &ExternalDependencyValues,
) -> AsyncStep {
    AsyncStep::ListOfColumn {
        table_name: column_getter_list.table_name.clone(),
        column: ColumnSpec {
            name: column_getter_list.column_name.clone(),
            dependency_type: internal_dependency.type_,
        },
        wheres: column_getter_list
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
    }
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_list<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &PopulatorList,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    populate_concrete_or_union_or_interface_list(
        external_dependency_values,
        internal_dependency_values,
        SingleOrVec::Single(field_plan.field_type.type_.name().to_smolstr()),
        populator,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        type_populator,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_union_or_interface_list<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    type_populator: &Box<dyn UnionOrInterfaceTypePopulatorList>,
    populator: &PopulatorList,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    let type_names =
        type_populator.populate(external_dependency_values, internal_dependency_values);
    populate_concrete_or_union_or_interface_list(
        external_dependency_values,
        internal_dependency_values,
        SingleOrVec::Vec(type_names),
        populator,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

enum SingleOrVec<TValue> {
    Single(TValue),
    Vec(Vec<TValue>),
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        type_names,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_concrete_or_union_or_interface_list<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    type_names: SingleOrVec<SmolStr>,
    populator: &PopulatorList,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    let populated = populator.populate(external_dependency_values, &internal_dependency_values);
    produced.push(Produced::FieldNewListOfObjects {
        parent_object_index,
        index_of_field_in_object,
        field_name: field_name.clone(),
    });
    let parent_list_index = produced.len() - 1;

    let selection_set_by_type = field_plan.selection_set_by_type.as_ref().unwrap();
    enum SingleOrIterator<'a, TIterator: Iterator<Item = &'a IndexMap<SmolStr, FieldPlan<'a>>>> {
        Single(&'a IndexMap<SmolStr, FieldPlan<'a>>),
        Iterator(TIterator),
    }
    let mut selection_sets = match type_names {
        SingleOrVec::Single(type_name) => {
            SingleOrIterator::Single(&selection_set_by_type[&type_name])
        }
        SingleOrVec::Vec(type_names) => SingleOrIterator::Iterator(
            type_names
                .into_iter()
                .map(|type_name| &selection_set_by_type[&type_name]),
        ),
    };
    populated
        .into_iter()
        .enumerate()
        .for_each(|(index_in_list, external_dependency_values)| {
            produced.push(Produced::ListItemNewObject {
                parent_list_index,
                index_in_list,
            });
            let parent_object_index = produced.len() - 1;

            make_progress_selection_set(
                match &mut selection_sets {
                    SingleOrIterator::Single(type_name) => type_name,
                    SingleOrIterator::Iterator(type_names) => type_names.next().unwrap(),
                },
                parent_object_index,
                external_dependency_values,
                produced,
                current_async_instructions,
                schema,
            );
        });
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_object<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &Populator,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    populate_concrete_or_union_or_interface_object(
        external_dependency_values,
        internal_dependency_values,
        field_plan.field_type.type_.name(),
        populator,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        type_populator,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_union_or_interface_object<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    type_populator: &Box<dyn UnionOrInterfaceTypePopulator>,
    populator: &Populator,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    let type_name = type_populator.populate(external_dependency_values, internal_dependency_values);
    populate_concrete_or_union_or_interface_object(
        external_dependency_values,
        internal_dependency_values,
        &type_name,
        populator,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn populate_concrete_or_union_or_interface_object<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    type_name: &str,
    populator: &Populator,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    post_populate_concrete_or_union_or_interface_object(
        type_name,
        populator.populate(external_dependency_values, internal_dependency_values),
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(populated, produced, field_plan, current_async_instructions, schema,)
)]
fn post_populate_concrete_or_union_or_interface_object<'a: 'b, 'b>(
    type_name: &str,
    populated: ExternalDependencyValues,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    produced.push(Produced::FieldNewObject {
        parent_object_index,
        index_of_field_in_object,
        field_name: field_name.clone(),
    });
    let parent_object_index = produced.len() - 1;

    let selection_set = &field_plan.selection_set_by_type.as_ref().unwrap()[type_name];

    make_progress_selection_set(
        selection_set,
        parent_object_index,
        populated,
        produced,
        current_async_instructions,
        schema,
    );
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        type_populator,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn optionally_populate_union_or_interface_object<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    type_populator: &Box<dyn OptionalUnionOrInterfaceTypePopulator>,
    populator: &Populator,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    let Some(type_name) =
        type_populator.populate(external_dependency_values, internal_dependency_values)
    else {
        produced.push(Produced::FieldNewNull {
            parent_object_index,
            index_of_field_in_object,
            field_name: field_name.clone(),
        });
        return;
    };
    populate_concrete_or_union_or_interface_object(
        external_dependency_values,
        internal_dependency_values,
        &type_name,
        populator,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        populator,
        produced,
        field_plan,
        current_async_instructions,
        schema,
    )
)]
fn optionally_populate_object<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &OptionalPopulator,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
    field_plan: &'a FieldPlan<'a>,
    current_async_instructions: &'b mut AsyncInstructions<'a>,
    schema: &Schema,
) {
    let Some(populated) =
        populator.populate(external_dependency_values, internal_dependency_values)
    else {
        produced.push(Produced::FieldNewNull {
            parent_object_index,
            index_of_field_in_object,
            field_name: field_name.clone(),
        });
        return;
    };
    post_populate_concrete_or_union_or_interface_object(
        field_plan.field_type.type_.name(),
        populated,
        produced,
        parent_object_index,
        index_of_field_in_object,
        field_name,
        field_plan,
        current_async_instructions,
        schema,
    )
}

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        carver,
        produced,
    )
)]
fn carve_list<'a: 'b, 'b>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    carver: &Box<dyn CarverList>,
    produced: &mut Vec<Produced>,
    parent_object_index: IndexInProduced,
    index_of_field_in_object: usize,
    field_name: &SmolStr,
) {
    produced.push(Produced::FieldNewListOfScalars {
        parent_object_index,
        index_of_field_in_object,
        field_name: field_name.clone(),
    });
    let parent_list_index = produced.len() - 1;
    let item_values = carver.carve(external_dependency_values, internal_dependency_values);
    item_values
        .into_iter()
        .enumerate()
        .for_each(|(index_in_list, item_value)| {
            produced.push(Produced::ListItemScalar {
                parent_list_index,
                index_in_list,
                value: item_value,
            });
        });
}

#[instrument(
    level = "trace",
    skip(arguments, external_dependency_values, internal_dependency, schema)
)]
fn get_internal_dependency_value_synchronous(
    arguments: Option<&IndexMap<SmolStr, Argument>>,
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency: &InternalDependency,
    schema: &Schema,
) -> DependencyValue {
    match &internal_dependency.resolver {
        InternalDependencyResolver::LiteralValue(literal_value) => literal_value.0.clone(),
        InternalDependencyResolver::IntrospectionTypeInterfaces => {
            let _ = trace_span!("resolve introspection type interfaces").entered();
            let type_name = external_dependency_values.get("name").unwrap().as_string();
            DependencyValue::List(
                schema
                    .maybe_type(type_name)
                    .filter(|type_| matches!(type_, Type::Object(_)))
                    .map(|type_| {
                        type_
                            .as_object()
                            .implements
                            .iter()
                            .map(|implement| DependencyValue::String(implement.clone()))
                            .collect()
                    })
                    .or_else(|| {
                        schema.interfaces.get(type_name).map(|interface| {
                            interface
                                .implements
                                .iter()
                                .map(|implement| DependencyValue::String(implement.clone()))
                                .collect()
                        })
                    })
                    // TODO: this needs to be optional for
                    // things other than object types and interfaces
                    .unwrap(),
            )
        }
        InternalDependencyResolver::IntrospectionTypePossibleTypes => {
            let _ = trace_span!("resolve introspection type possible types").entered();
            let type_name = external_dependency_values.get("name").unwrap().as_string();
            DependencyValue::List(
                schema
                    .interface_all_concrete_types
                    .get(type_name)
                    .map(|all_concrete_type_names| {
                        all_concrete_type_names
                            .into_iter()
                            .sorted()
                            .map(|concrete_type_name| {
                                DependencyValue::String(concrete_type_name.clone())
                            })
                            .collect()
                    })
                    .or_else(|| {
                        schema.unions.get(type_name).map(|union| {
                            union
                                .types
                                .iter()
                                .map(|concrete_type_name| {
                                    DependencyValue::String(concrete_type_name.clone())
                                })
                                .collect()
                        })
                    })
                    // TODO: this needs to be optional for
                    // things other than interfaces and unions
                    .unwrap(),
            )
        }
        InternalDependencyResolver::Argument(argument_resolver) => {
            let argument = arguments.unwrap().get(&argument_resolver.name).unwrap();
            match (internal_dependency.type_, &argument.value) {
                (DependencyType::Id, Value::Int(argument_value)) => {
                    DependencyValue::Id(Id::Int(*argument_value))
                }
                (DependencyType::String, Value::String(argument_value)) => {
                    DependencyValue::String(argument_value.clone())
                }
                (DependencyType::String, Value::EnumVariant(argument_value)) => {
                    DependencyValue::String(argument_value.clone())
                }
                // TODO: truly unreachable?
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}
