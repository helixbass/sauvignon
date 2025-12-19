use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use smol_str::SmolStr;
use squalid::{OptionExt, _d};
use tracing::instrument;

use crate::{
    request, types, Argument, Database, OperationType, Request, ResponseValue, Schema, Selection,
};

pub struct SyncQueryPlan<'a> {
    pub field_plans: IndexMap<SmolStr, SyncFieldPlan<'a>>,
}

impl<'a> SyncQueryPlan<'a> {
    #[instrument(level = "trace", skip(request, schema))]
    pub fn new(request: &'a Request, schema: &'a Schema) -> Self {
        let chosen_operation = request.chosen_operation();
        assert_eq!(chosen_operation.operation_type, OperationType::Query);

        Self {
            field_plans: create_field_plans(
                &chosen_operation.selection_set,
                &[schema.query_type_name.clone()].into_iter().collect(),
                schema,
                request,
            )
            .remove(&schema.query_type_name)
            .unwrap(),
        }
    }
}

#[instrument(level = "trace", skip(selection_set, schema, request))]
fn create_field_plans<'a>(
    selection_set: &'a [Selection],
    all_current_concrete_type_names: &HashSet<SmolStr>,
    schema: &'a Schema,
    request: &'a Request,
) -> HashMap<SmolStr, IndexMap<SmolStr, SyncFieldPlan<'a>>> {
    let mut by_concrete_type: HashMap<SmolStr, IndexMap<SmolStr, SyncFieldPlan<'a>>> = _d();

    selection_set.iter().for_each(|selection| match selection {
        Selection::Field(field) => {
            all_current_concrete_type_names
                .into_iter()
                .for_each(|concrete_type_name| {
                    by_concrete_type
                        .entry(concrete_type_name.clone())
                        .or_default()
                        .insert(
                            field.name.clone(),
                            SyncFieldPlan::new(
                                field,
                                schema
                                    .type_(concrete_type_name)
                                    .as_object()
                                    .field(&field.name),
                                schema,
                                request,
                            ),
                        )
                        .assert_none();
                });
        }
        _ => unimplemented!(),
    });

    by_concrete_type
}

pub struct SyncFieldPlan<'a> {
    pub name: SmolStr,
    pub field_type: &'a types::Field,
    pub selection_set_by_type: Option<HashMap<SmolStr, IndexMap<SmolStr, SyncFieldPlan<'a>>>>,
    pub arguments: Option<IndexMap<SmolStr, Argument>>,
}

impl<'a> SyncFieldPlan<'a> {
    #[instrument(level = "trace", skip(request_field, field_type, schema, request))]
    pub fn new(
        request_field: &'a request::Field,
        field_type: &'a types::Field,
        schema: &'a Schema,
        request: &'a Request,
    ) -> Self {
        Self {
            name: request_field.name.clone(),
            field_type,
            selection_set_by_type: request_field.selection_set.as_ref().map(|selection_set| {
                create_field_plans(
                    selection_set,
                    &schema.all_concrete_type_names_for_type_or_union_or_interface(
                        field_type.type_.name(),
                    ),
                    schema,
                    request,
                )
            }),
            arguments: request_field.arguments.as_ref().map(|arguments| {
                arguments
                    .into_iter()
                    .map(|argument| (argument.name.clone(), argument.clone()))
                    .collect()
            }),
        }
    }
}

#[instrument(level = "trace", skip(schema, request, database))]
fn compute_sync_response(
    schema: &Schema,
    request: &Request,
    database: &dyn Database,
) -> ResponseValue {
    let query_plan = SyncQueryPlan::new(request, schema);

    ResponseValue::Map(
        query_plan
            .field_plans
            .into_iter()
            .map(|(name, field_plan)| (name,))
            .collect(),
    )
}
