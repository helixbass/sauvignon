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

enum Produced {
    FieldNewObject {
        parent_object_index: Option<usize>,
        index_of_field_in_object: usize,
        field_name: SmolStr,
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
    },
    FieldNewListOfObjects {
        parent_object_index: usize,
        index_of_field_in_object: usize,
        field_name: SmolStr,
    },
    // FieldNewListOfScalars { ... },
    ListItemNewObject {
        parent_list_index: usize,
        index_in_list: usize,
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
    },
    FieldScalar {
        parent_object_index: usize,
        index_of_field_in_object: usize,
        field_name: SmolStr,
        value: ResponseValue,
    },
    // ListItemScalar { ... },
}

type IndexInProduced = usize;

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

impl From<Vec<Produced>> for ResponseValue {
    fn from(produced: Vec<Produced>) -> Self {
        let mut objects_by_index: HashMap<IndexInProduced, Vec<ObjectFieldStuff>> = _d();
        let mut lists_of_objects_by_index: HashMap<IndexInProduced, Vec<ListItemStuff>> = _d();

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
            }
        }

        let mut completed_objects_by_index: HashMap<usize, Vec<(SmolStr, ResponseValue)>> = _d();
        Self::Map(construct_object(
            completed_objects_by_index.remove(&0).unwrap(),
        ))
    }
}

fn construct_object(
    fields: impl IntoIterator<Item = (SmolStr, ResponseValue)>,
) -> IndexMap<SmolStr, ResponseValue> {
    fields.into_iter().collect()
}
