use std::collections::HashMap;

use indexmap::IndexMap;
use smol_str::SmolStr;
use squalid::_d;

use crate::{Database, ExternalDependencyValues, QueryPlan, ResponseValue, Schema};

pub async fn produce_response(
    schema: &Schema,
    database: &dyn Database,
    query_plan: &QueryPlan<'_>,
) -> ResponseValue {
    let mut produced: Vec<Produced> = _d();
    unimplemented!()
}

type IndexInProduced = usize;

enum Produced {
    FieldNewObject {
        parent_object_index: Option<IndexInProduced>,
        index_of_field_in_object: usize,
        field_name: SmolStr,
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
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
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
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
                Produced::FieldNewObject {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                    type_,
                    external_dependency_values_for_its_fields,
                } => {
                    objects_by_index.insert(index, _d());

                    match parent_object_index {
                        None => {
                            assert_eq!(index, 0);
                        }
                        Some(parent_object_index) => objects_by_index
                            .get_mut(&parent_object_index)
                            .unwrap()
                            .push(ObjectFieldStuff {
                                field_name,
                                index_of_field_in_object,
                                value_stub: FieldValueStub::ObjectIndexInProduced(index),
                            }),
                    }
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
                    type_,
                    external_dependency_values_for_its_fields,
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

        ResponseValue::Map(construct_object(0, &mut objects_by_index))

        // let mut completed_objects_by_index: HashMap<usize, Vec<(SmolStr, ResponseValue)>> = _d();
        // Self::Map(construct_object(
        //     completed_objects_by_index.remove(&0).unwrap(),
        // ))
    }
}

fn construct_object(
    object_index: usize,
    objects_by_index: &mut HashMap<IndexInProduced, Vec<ObjectFieldStuff>>,
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
                        ResponseValue::Map(construct_object(index_in_produced, objects_by_index))
                    }
                    FieldValueStub::ListIndexInProduced(index_in_produced) => {
                        ResponseValue::List(unimplemented!())
                    }
                },
            )
        })
        .collect()
}

// fn construct_object(
//     fields: impl IntoIterator<Item = (SmolStr, ResponseValue)>,
// ) -> IndexMap<SmolStr, ResponseValue> {
//     fields.into_iter().collect()
// }
