use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;
use squalid::{OptionExt, _d};
use tracing::instrument;

use crate::{
    request, types, Argument, ColumnToken, ColumnTokens, Directive, IndexMap,
    InternalDependencyResolver, OperationType, Request, Schema, Selection, Value,
};

pub struct QueryPlan<'a> {
    pub field_plans: IndexMap<SmolStr, FieldPlan<'a>>,
}

impl<'a> QueryPlan<'a> {
    #[instrument(level = "trace", skip(request, schema, column_tokens))]
    pub fn new(
        request: &'a Request,
        schema: &'a Schema,
        column_tokens: Option<&ColumnTokens>,
    ) -> Self {
        let chosen_operation = request.chosen_operation();
        assert_eq!(chosen_operation.operation_type, OperationType::Query);

        Self {
            field_plans: create_field_plans(
                &chosen_operation.selection_set,
                &[schema.query_type_name.clone()].into_iter().collect(),
                schema,
                request,
                column_tokens,
            )
            .remove(&schema.query_type_name)
            .unwrap(),
        }
    }
}

pub struct FieldPlan<'a> {
    pub name: SmolStr,
    pub field_type: &'a types::Field,
    pub selection_set_by_type: Option<HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>>>,
    pub arguments: Option<IndexMap<SmolStr, Argument>>,
    pub column_token: Option<ColumnToken>,
}

impl<'a> FieldPlan<'a> {
    #[instrument(
        level = "trace",
        skip(request_field, field_type, schema, request, column_tokens)
    )]
    pub fn new(
        request_field: &'a request::Field,
        field_type: &'a types::Field,
        schema: &'a Schema,
        request: &'a Request,
        column_tokens: Option<&ColumnTokens>,
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
                    column_tokens,
                )
            }),
            arguments: request_field.arguments.as_ref().map(|arguments| {
                arguments
                    .into_iter()
                    .map(|argument| (argument.name.clone(), argument.clone()))
                    .collect()
            }),
            column_token: column_tokens.map(|column_tokens| {
                match &field_type.resolver.internal_dependencies[0].resolver {
                    InternalDependencyResolver::ColumnGetter(column_getter) => {
                        column_tokens[&column_getter.table_name][&column_getter.column_name]
                    }
                    InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
                        column_tokens[&column_getter_list.table_name]
                            [&column_getter_list.column_name]
                    }
                    _ => unimplemented!(),
                }
            }),
        }
    }
}

#[instrument(level = "trace", skip(selection_set, schema, request, column_tokens))]
fn create_field_plans<'a>(
    selection_set: &'a [Selection],
    all_current_concrete_type_names: &HashSet<SmolStr>,
    schema: &'a Schema,
    request: &'a Request,
    column_tokens: Option<&ColumnTokens>,
) -> HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>> {
    merge_hash_maps(
        selection_set
            .iter()
            .filter(|selection| !match selection {
                Selection::Field(field) => should_skip(&field.directives),
                Selection::InlineFragment(inline_fragment) => {
                    should_skip(&inline_fragment.directives)
                }
                Selection::FragmentSpread(fragment_spread) => {
                    should_skip(&fragment_spread.directives)
                }
            })
            .map(|selection| match selection {
                Selection::Field(field) => all_current_concrete_type_names
                    .iter()
                    .map(|concrete_type_name| {
                        let concrete_type = schema.type_(concrete_type_name);
                        (
                            concrete_type_name.clone(),
                            [(
                                field.name.clone(),
                                FieldPlan::new(
                                    field,
                                    concrete_type.as_object().field(&field.name),
                                    schema,
                                    request,
                                    column_tokens,
                                ),
                            )]
                            .into_iter()
                            .collect(),
                        )
                    })
                    .collect(),
                Selection::FragmentSpread(fragment_spread) => {
                    let fragment = request.fragment(&fragment_spread.name);

                    get_overlapping_fragment_types(
                        &all_current_concrete_type_names,
                        &fragment.selection_set,
                        &schema
                            .all_concrete_type_names_for_type_or_union_or_interface(&fragment.on),
                        schema,
                        request,
                        column_tokens,
                    )
                }
                Selection::InlineFragment(inline_fragment) => get_overlapping_fragment_types(
                    &all_current_concrete_type_names,
                    &inline_fragment.selection_set,
                    &match inline_fragment.on.as_ref() {
                        Some(on) => {
                            schema.all_concrete_type_names_for_type_or_union_or_interface(on)
                        }
                        None => all_current_concrete_type_names.clone(),
                    },
                    schema,
                    request,
                    column_tokens,
                ),
            }),
    )
}

fn should_skip(directives: &[Directive]) -> bool {
    if directives.into_iter().any(|directive| {
        directive.name == "skip"
            && directive.arguments.as_ref().unwrap()[0].value == Value::Bool(true)
    }) {
        return true;
    }
    if directives.into_iter().any(|directive| {
        directive.name == "include"
            && directive.arguments.as_ref().unwrap()[0].value == Value::Bool(false)
    }) {
        return true;
    }
    false
}

#[instrument(
    level = "trace",
    skip(fragment_selection_set, schema, request, column_tokens)
)]
fn get_overlapping_fragment_types<'a>(
    all_current_concrete_type_names: &HashSet<SmolStr>,
    fragment_selection_set: &'a [Selection],
    all_concrete_type_names_for_fragment: &HashSet<SmolStr>,
    schema: &'a Schema,
    request: &'a Request,
    column_tokens: Option<&ColumnTokens>,
) -> HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>> {
    create_field_plans(
        fragment_selection_set,
        &all_current_concrete_type_names
            .intersection(all_concrete_type_names_for_fragment)
            .cloned()
            .collect(),
        schema,
        request,
        column_tokens,
    )
}

fn merge_hash_maps<'a>(
    hash_maps: impl Iterator<Item = HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>>>,
) -> HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>> {
    let mut ret: HashMap<SmolStr, IndexMap<SmolStr, FieldPlan<'a>>> = _d();

    for hash_map in hash_maps {
        for (type_name, mut field_plans) in hash_map {
            if !ret.contains_key(&type_name) {
                ret.insert(type_name, field_plans).assert_none();
            } else {
                let existing_field_plans = ret.remove(&type_name).unwrap();
                let mut updated_field_plans = existing_field_plans
                    .into_iter()
                    .map(|(existing_field_name, mut existing_field_plan)| {
                        if !field_plans.contains_key(&existing_field_name) {
                            (existing_field_name, existing_field_plan)
                        } else {
                            let field_plan =
                                field_plans.shift_remove(&existing_field_name).unwrap();
                            existing_field_plan.selection_set_by_type = match (
                                existing_field_plan.selection_set_by_type,
                                field_plan.selection_set_by_type,
                            ) {
                                (None, None) => None,
                                (
                                    Some(existing_field_plan_selection_set_by_type),
                                    Some(field_plan_selection_set_by_type),
                                ) => Some(merge_hash_maps(
                                    [
                                        existing_field_plan_selection_set_by_type,
                                        field_plan_selection_set_by_type,
                                    ]
                                    .into_iter(),
                                )),
                                _ => unreachable!(),
                            };
                            (existing_field_name, existing_field_plan)
                        }
                    })
                    .collect::<IndexMap<_, _>>();
                for (field_name, field_plan) in field_plans {
                    updated_field_plans.insert(field_name, field_plan);
                }
                ret.insert(type_name, updated_field_plans);
            }
        }
    }

    ret
}
