use std::collections::HashMap;

use indexmap::IndexMap;
use smol_str::SmolStr;
use squalid::_d;
use tracing::instrument;

use crate::ResponseValue;

use super::IndexInProduced;

pub enum Produced {
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
    FieldNewListOfScalars {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        field_name: SmolStr,
    },
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
    ListItemScalar {
        parent_list_index: IndexInProduced,
        index_in_list: usize,
        value: ResponseValue,
    },
    FieldNewNull {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        field_name: SmolStr,
    },
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

struct ListOfScalarsItemStuff {
    pub index_in_list: usize,
    pub value: ResponseValue,
}

impl From<Vec<Produced>> for ResponseValue {
    fn from(produced: Vec<Produced>) -> Self {
        let mut objects_by_index: HashMap<IndexInProduced, Vec<ObjectFieldStuff>> = _d();
        let mut lists_of_objects_by_index: HashMap<IndexInProduced, Vec<ListOfObjectsItemStuff>> =
            _d();
        let mut lists_of_scalars_by_index: HashMap<IndexInProduced, Vec<ListOfScalarsItemStuff>> =
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
                Produced::FieldNewListOfScalars {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                } => {
                    lists_of_scalars_by_index.insert(index, _d());

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
                Produced::ListItemScalar {
                    parent_list_index,
                    index_in_list,
                    value,
                } => {
                    lists_of_scalars_by_index
                        .get_mut(&parent_list_index)
                        .unwrap()
                        .push(ListOfScalarsItemStuff {
                            index_in_list,
                            value,
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
                Produced::FieldNewNull {
                    parent_object_index,
                    index_of_field_in_object,
                    field_name,
                } => {
                    objects_by_index
                        .get_mut(&parent_object_index)
                        .unwrap()
                        .push(ObjectFieldStuff {
                            field_name,
                            index_of_field_in_object,
                            value_stub: FieldValueStub::Value(ResponseValue::Null),
                        });
                }
            }
        }

        ResponseValue::Map(construct_object(
            0,
            &mut objects_by_index,
            &mut lists_of_objects_by_index,
            &mut lists_of_scalars_by_index,
        ))
    }
}

#[instrument(
    level = "trace",
    skip(objects_by_index, lists_of_objects_by_index, lists_of_scalars_by_index)
)]
fn construct_object(
    object_index: usize,
    objects_by_index: &mut HashMap<IndexInProduced, Vec<ObjectFieldStuff>>,
    lists_of_objects_by_index: &mut HashMap<IndexInProduced, Vec<ListOfObjectsItemStuff>>,
    lists_of_scalars_by_index: &mut HashMap<IndexInProduced, Vec<ListOfScalarsItemStuff>>,
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
                            lists_of_scalars_by_index,
                        ))
                    }
                    FieldValueStub::ListIndexInProduced(index_in_produced) => {
                        ResponseValue::List(
                            match lists_of_objects_by_index.contains_key(&index_in_produced) {
                                true => {
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
                                                lists_of_scalars_by_index,
                                            ))
                                        })
                                        .collect()
                                }
                                false => {
                                    let mut items = lists_of_scalars_by_index
                                        .remove(&index_in_produced)
                                        .unwrap();
                                    // TODO: like above also here simultaneously check
                                    // that we have consecutive expected
                                    // index_of_field_in_object's?
                                    items.sort_by_key(|list_of_scalars_item_stuff| {
                                        list_of_scalars_item_stuff.index_in_list
                                    });
                                    items
                                        .into_iter()
                                        .map(|list_of_scalars_item_stuff| {
                                            list_of_scalars_item_stuff.value
                                        })
                                        .collect()
                                }
                            },
                        )
                    }
                },
            )
        })
        .collect()
}
