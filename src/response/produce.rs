use std::collections::HashMap;

use indexmap::IndexMap;
use itertools::Itertools;
use smallvec::SmallVec;
use smol_str::SmolStr;
use squalid::_d;
use tracing::{instrument, trace_span};

use crate::{
    Argument, Carver, CarverOrPopulator, Database, DependencyType, DependencyValue,
    ExternalDependencyValues, FieldPlan, Id, InternalDependency, InternalDependencyResolver,
    InternalDependencyValues, Populator, PopulatorInterface, PopulatorListInterface, QueryPlan,
    ResponseValue, Schema, Type, Value,
};

type IndexInProduced = usize;

enum AsyncStep {
    ListOfIds {
        table_name: SmolStr,
    },
    ListOfIdsAndOtherColumns {
        table_name: SmolStr,
        other_columns: SmallVec<[SmolStr; 8]>,
    },
}

struct AsyncInstruction<'a> {
    pub step: AsyncStep,
    pub is_internal_dependency_of: IsInternalDependencyOf<'a>,
}

struct IsInternalDependencyOf<'a> {
    pub dependency_name: SmolStr,
    pub is_internal_dependency_of: IsInternalDependencyOfInner<'a>,
}

enum IsInternalDependencyOfInner<'a> {
    ObjectFieldScalar {
        parent_object_index: IndexInProduced,
        carver: &'a Box<dyn Carver>,
        external_dependency_values: &'a ExternalDependencyValues,
    },
    ObjectFieldObject {
        new_object_index: IndexInProduced,
        populator: &'a Populator,
        external_dependency_values: &'a ExternalDependencyValues,
    },
}

#[instrument(level = "trace", skip(schema, database, query_plan))]
pub async fn produce_response(
    schema: &Schema,
    database: &dyn Database,
    query_plan: &QueryPlan<'_>,
) -> ResponseValue {
    let mut produced: Vec<Produced> = _d();
    produced.push(Produced::NewRootObject);

    let mut next_async_instructions: Vec<AsyncInstruction> = _d();

    make_progress_selection_set(
        &query_plan.field_plans,
        0,
        &ExternalDependencyValues::Empty,
        &mut produced,
        &mut next_async_instructions,
        schema,
    );

    unimplemented!();

    produced.into()
}

#[instrument(
    level = "trace",
    skip(
        field_plans,
        external_dependency_values,
        produced,
        next_async_instructions,
        schema,
    )
)]
fn make_progress_selection_set(
    field_plans: &IndexMap<SmolStr, FieldPlan<'_>>,
    parent_object_index: usize,
    external_dependency_values: &ExternalDependencyValues,
    produced: &mut Vec<Produced>,
    next_async_instructions: &mut Vec<AsyncInstruction>,
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
                            get_internal_dependency_value_synchronous(
                                field_plan.arguments.as_ref(),
                                &external_dependency_values,
                                internal_dependency,
                                schema,
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
                            let populated = populator
                                .populate(&external_dependency_values, &internal_dependency_values);
                            produced.push(Produced::FieldNewObject {
                                parent_object_index,
                                index_of_field_in_object,
                                field_name: field_name.clone(),
                            });
                            // TODO: presumably queue up object fields?
                            unimplemented!()
                        }
                        CarverOrPopulator::PopulatorList(populator) => {
                            let populated = populator
                                .populate(&external_dependency_values, &internal_dependency_values);
                            // TODO: this presumably needs to maybe also be
                            // eg FieldNewListOfScalars?
                            produced.push(Produced::FieldNewListOfObjects {
                                parent_object_index,
                                index_of_field_in_object,
                                field_name: field_name.clone(),
                            });
                            let parent_list_index = produced.len() - 1;

                            let type_name = field_plan.field_type.type_.name();
                            let selection_set =
                                &field_plan.selection_set_by_type.as_ref().unwrap()[type_name];
                            populated.into_iter().enumerate().for_each(
                                |(index_in_list, external_dependency_values)| {
                                    produced.push(Produced::ListItemNewObject {
                                        parent_list_index,
                                        index_in_list,
                                    });
                                    let parent_object_index = produced.len() - 1;

                                    make_progress_selection_set(
                                        selection_set,
                                        parent_object_index,
                                        &external_dependency_values,
                                        produced,
                                        next_async_instructions,
                                        schema,
                                    );
                                },
                            );
                        }
                        _ => unimplemented!(),
                    }
                }
                false => {
                    unimplemented!()
                }
            }
        },
    );
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

enum Produced {
    NewRootObject,
    FieldNewObject {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        field_name: SmolStr,
    },
    FieldNewListOfObjects {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        field_name: SmolStr,
    },
    // FieldNewListOfScalars { ... },
    ListItemNewObject {
        parent_list_index: IndexInProduced,
        index_in_list: usize,
    },
    FieldScalar {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        field_name: SmolStr,
        value: ResponseValue,
    },
    // ListItemScalar { ... },
}

struct ObjectFieldStuff {
    pub field_name: SmolStr,
    pub value_stub: FieldValueStub,
    pub index_of_field_in_object: usize,
}

enum FieldValueStub {
    Value(ResponseValue),
    ObjectIndexInProduced(IndexInProduced),
    ListIndexInProduced(IndexInProduced),
}

struct ListOfObjectsItemStuff {
    pub index_in_list: usize,
    pub object_index_in_produced: IndexInProduced,
}

impl From<Vec<Produced>> for ResponseValue {
    fn from(produced: Vec<Produced>) -> Self {
        let mut objects_by_index: HashMap<IndexInProduced, Vec<ObjectFieldStuff>> = _d();
        let mut lists_of_objects_by_index: HashMap<IndexInProduced, Vec<ListOfObjectsItemStuff>> =
            _d();

        for (index, step) in produced.into_iter().enumerate() {
            match step {
                Produced::NewRootObject => {
                    assert_eq!(index, 0);

                    objects_by_index.insert(index, _d());
                }
                Produced::FieldNewObject {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                } => {
                    objects_by_index.insert(index, _d());

                    objects_by_index
                        .get_mut(&parent_object_index)
                        .unwrap()
                        .push(ObjectFieldStuff {
                            field_name,
                            index_of_field_in_object,
                            value_stub: FieldValueStub::ObjectIndexInProduced(index),
                        });
                }
                Produced::FieldNewListOfObjects {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                } => {
                    lists_of_objects_by_index.insert(index, _d());

                    objects_by_index
                        .get_mut(&parent_object_index)
                        .unwrap()
                        .push(ObjectFieldStuff {
                            field_name,
                            index_of_field_in_object,
                            value_stub: FieldValueStub::ListIndexInProduced(index),
                        });
                }
                Produced::ListItemNewObject {
                    parent_list_index,
                    index_in_list,
                } => {
                    objects_by_index.insert(index, _d());

                    lists_of_objects_by_index
                        .get_mut(&parent_list_index)
                        .unwrap()
                        .push(ListOfObjectsItemStuff {
                            index_in_list,
                            object_index_in_produced: index,
                        });
                }
                Produced::FieldScalar {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                    value,
                } => {
                    objects_by_index
                        .get_mut(&parent_object_index)
                        .unwrap()
                        .push(ObjectFieldStuff {
                            field_name,
                            index_of_field_in_object,
                            value_stub: FieldValueStub::Value(value),
                        });
                }
            }
        }

        ResponseValue::Map(construct_object(
            0,
            &mut objects_by_index,
            &mut lists_of_objects_by_index,
        ))
    }
}

#[instrument(level = "trace", skip(objects_by_index, lists_of_objects_by_index))]
fn construct_object(
    object_index: usize,
    objects_by_index: &mut HashMap<IndexInProduced, Vec<ObjectFieldStuff>>,
    lists_of_objects_by_index: &mut HashMap<IndexInProduced, Vec<ListOfObjectsItemStuff>>,
) -> IndexMap<SmolStr, ResponseValue> {
    let mut fields = objects_by_index.remove(&object_index).unwrap();
    // TODO: simultaneously check that we have consecutive expected
    // index_of_field_in_object's?
    fields.sort_by_key(|field| field.index_of_field_in_object);
    fields
        .into_iter()
        .map(|object_field_stuff| {
            (
                object_field_stuff.field_name,
                match object_field_stuff.value_stub {
                    FieldValueStub::Value(value) => value,
                    FieldValueStub::ObjectIndexInProduced(index_in_produced) => {
                        ResponseValue::Map(construct_object(
                            index_in_produced,
                            objects_by_index,
                            lists_of_objects_by_index,
                        ))
                    }
                    FieldValueStub::ListIndexInProduced(index_in_produced) => {
                        // TODO: in reality I assume here you'd know
                        // list-of-objects vs list-of-scalars?
                        ResponseValue::List({
                            let mut items = lists_of_objects_by_index
                                .remove(&index_in_produced)
                                .unwrap();
                            // TODO: like above also here simultaneously check
                            // that we have consecutive expected
                            // index_of_field_in_object's?
                            items.sort_by_key(|list_of_objects_item_stuff| {
                                list_of_objects_item_stuff.index_in_list
                            });
                            items
                                .into_iter()
                                .map(|list_of_objects_item_stuff| {
                                    ResponseValue::Map(construct_object(
                                        list_of_objects_item_stuff.object_index_in_produced,
                                        objects_by_index,
                                        lists_of_objects_by_index,
                                    ))
                                })
                                .collect()
                        })
                    }
                },
            )
        })
        .collect()
}
