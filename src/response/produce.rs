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
        parent_index: Option<usize>,
        field_name: SmolStr,
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
    },
    FieldNewList {
        parent_index: usize,
        field_name: SmolStr,
    },
    ListItemNewObject {
        parent_list_index: usize,
        index_in_list: usize,
        type_: SmolStr,
        external_dependency_values_for_its_fields: ExternalDependencyValues,
    },
}

impl From<Vec<Produced>> for ResponseValue {
    fn from(value: Vec<Produced>) -> Self {
        Self::Map()
    }
}
