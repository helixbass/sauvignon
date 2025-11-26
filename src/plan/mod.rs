use std::collections::{HashMap, HashSet};

use squalid::OptionExt;

use crate::{
    fields_in_progress_new, request, types, Argument, IndexMap, OperationType, Request,
    ResponseInProgress, Schema, Selection,
};

pub struct QueryPlan<'a> {
    field_plans: IndexMap<String, FieldPlan<'a>>,
}

impl<'a> QueryPlan<'a> {
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

    pub fn initial_response_in_progress(&self) -> ResponseInProgress<'_> {
        ResponseInProgress::new(fields_in_progress_new(
            &self.field_plans,
            &Default::default(),
        ))
    }
}

pub struct FieldPlan<'a> {
    pub name: String,
    pub field_type: &'a types::Field,
    pub selection_set_by_type: Option<HashMap<String, IndexMap<String, FieldPlan<'a>>>>,
    pub arguments: &'a Option<HashMap<String, Argument>>,
}

impl<'a> FieldPlan<'a> {
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
            arguments: &request_field.arguments,
        }
    }
}

fn create_field_plans<'a>(
    selection_set: &'a [Selection],
    all_current_concrete_type_names: &HashSet<String>,
    schema: &'a Schema,
    request: &'a Request,
) -> HashMap<String, IndexMap<String, FieldPlan<'a>>> {
    merge_hash_maps(selection_set.iter().map(|selection| {
        match selection {
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
                    &schema.all_concrete_type_names_for_type_or_union_or_interface(&fragment.on),
                    schema,
                    request,
                )
            }
            Selection::InlineFragment(inline_fragment) => get_overlapping_fragment_types(
                &all_current_concrete_type_names,
                &inline_fragment.selection_set,
                &match inline_fragment.on.as_ref() {
                    Some(on) => schema.all_concrete_type_names_for_type_or_union_or_interface(on),
                    None => all_current_concrete_type_names.clone(),
                },
                schema,
                request,
            ),
        }
    }))
}

fn get_overlapping_fragment_types<'a>(
    all_current_concrete_type_names: &HashSet<String>,
    fragment_selection_set: &'a [Selection],
    all_concrete_type_names_for_fragment: &HashSet<String>,
    schema: &'a Schema,
    request: &'a Request,
) -> HashMap<String, IndexMap<String, FieldPlan<'a>>> {
    create_field_plans(
        fragment_selection_set,
        &all_current_concrete_type_names
            .intersection(all_concrete_type_names_for_fragment)
            .cloned()
            .collect::<HashSet<_>>(),
        schema,
        request,
    )
}

fn merge_hash_maps<'a>(
    hash_maps: impl Iterator<Item = HashMap<String, IndexMap<String, FieldPlan<'a>>>>,
) -> HashMap<String, IndexMap<String, FieldPlan<'a>>> {
    let mut ret: HashMap<String, IndexMap<String, FieldPlan<'a>>> = Default::default();

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
