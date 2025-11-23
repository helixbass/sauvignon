use std::collections::{HashMap, HashSet};

use crate::{
    fields_in_progress_new, request, types, OperationType, Request, ResponseInProgress, Schema,
    Selection, SelectionSet, TypeInterface, TypeOrUnionOrInterface,
};

pub struct QueryPlan<'a> {
    field_plans: Vec<FieldPlan<'a>>,
}

impl<'a> QueryPlan<'a> {
    pub fn new(request: &'a Request, schema: &'a Schema) -> Self {
        let chosen_operation = request.chosen_operation();
        assert_eq!(chosen_operation.operation_type, OperationType::Query);

        Self {
            field_plans: create_field_plans(
                &chosen_operation.selection_set,
                &schema.query_type_name,
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
    pub request_field: &'a request::Field,
    pub field_type: &'a types::Field,
    pub selection_set_by_type: Option<HashMap<String, Vec<FieldPlan<'a>>>>,
}

impl<'a> FieldPlan<'a> {
    pub fn new(
        request_field: &'a request::Field,
        field_type: &'a types::Field,
        schema: &'a Schema,
        request: &'a Request,
    ) -> Self {
        Self {
            request_field,
            field_type,
            selection_set_by_type: request_field.selection_set.as_ref().map(|selection_set| {
                create_field_plans(selection_set, field_type.type_.name(), schema, request)
            }),
        }
    }
}

fn create_field_plans<'a>(
    selection_set: &'a SelectionSet,
    type_name: &str,
    schema: &'a Schema,
    request: &'a Request,
) -> HashMap<String, Vec<FieldPlan<'a>>> {
    let type_or_union_or_interface = schema.type_or_union_or_interface(type_name);

    let all_concrete_type_names_for_type_or_union_or_interface =
        get_all_concrete_type_names_for_type_or_union_or_interface(&type_or_union_or_interface);

    let mut ret: HashMap<String, Vec<FieldPlan<'a>>> = Default::default();

    for selection in selection_set.selections.iter() {
        match selection {
            Selection::Field(field) => match type_or_union_or_interface {
                TypeOrUnionOrInterface::Type(type_) => {
                    let type_fields = &type_.as_object().fields;
                    let field_type = &type_fields[&field.name];
                    ret.entry(type_name.to_owned())
                        .or_default()
                        .push(FieldPlan::new(field, field_type, schema, request));
                }
                TypeOrUnionOrInterface::Union(_) => unreachable!(),
            },
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = request.fragment(&fragment_spread.name);
                let all_concrete_type_names_for_fragment =
                    get_all_concrete_type_names_for_type_or_union_or_interface(
                        &schema.type_or_union_or_interface(&fragment.on),
                    );
                add_overlapping_fragment_types(
                    &mut ret,
                    &type_or_union_or_interface,
                    type_name,
                    &fragment.selection_set,
                    &all_concrete_type_names_for_fragment,
                    schema,
                    request,
                );
            }
            Selection::InlineFragment(inline_fragment) => {
                let all_concrete_type_names_for_fragment = match inline_fragment.on.as_ref() {
                    Some(on) => get_all_concrete_type_names_for_type_or_union_or_interface(
                        &schema.type_or_union_or_interface(on),
                    ),
                    None => all_concrete_type_names_for_type_or_union_or_interface.clone(),
                };
                add_overlapping_fragment_types(
                    &mut ret,
                    &type_or_union_or_interface,
                    type_name,
                    &inline_fragment.selection_set,
                    &all_concrete_type_names_for_fragment,
                    schema,
                    request,
                );
            }
        }
    }

    ret
}

fn add_overlapping_fragment_types<'a>(
    ret: &mut HashMap<String, Vec<FieldPlan<'a>>>,
    type_or_union_or_interface: &TypeOrUnionOrInterface,
    type_name: &str,
    fragment_selection_set: &'a SelectionSet,
    all_concrete_type_names_for_fragment: &[String],
    schema: &'a Schema,
    request: &'a Request,
) {
    match type_or_union_or_interface {
        TypeOrUnionOrInterface::Type(_) => {
            ret.entry(type_name.to_owned()).or_default().extend(
                create_field_plans(fragment_selection_set, type_name, schema, request)
                    .remove(type_name)
                    .unwrap(),
            );
        }
        TypeOrUnionOrInterface::Union(union) => {
            let all_concrete_type_names_for_fragment_set: HashSet<String> =
                HashSet::from_iter(all_concrete_type_names_for_fragment.into_iter().cloned());
            let union_types_set = HashSet::from_iter(union.types.iter().cloned());
            all_concrete_type_names_for_fragment_set
                .intersection(&union_types_set)
                .for_each(|matching_type_name| {
                    ret.entry(matching_type_name.to_owned())
                        .or_default()
                        .extend(
                            create_field_plans(
                                &fragment_selection_set,
                                // TODO: is this right recursively vs eg `type_name`?
                                matching_type_name,
                                schema,
                                request,
                            )
                            .remove(matching_type_name)
                            .unwrap(),
                        );
                });
        }
    }
}

fn get_all_concrete_type_names_for_type_or_union_or_interface(
    type_or_union_or_interface: &TypeOrUnionOrInterface,
) -> Vec<String> {
    match type_or_union_or_interface {
        TypeOrUnionOrInterface::Type(type_) => vec![type_.name().to_owned()],
        TypeOrUnionOrInterface::Union(union) => union.types.clone(),
    }
}
