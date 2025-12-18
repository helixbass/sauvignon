use std::collections::HashSet;

use itertools::Itertools;
use squalid::{OptionExt, _d};
use tracing::instrument;

use crate::{
    Directive, ExecutableDefinition, FieldInterface, FragmentDefinition, FragmentSpread,
    InlineFragment, Location, OperationDefinition, PositionsTracker, Request, Schema, Selection,
    SelectionField, Type, TypeFull, TypeOrInterfaceField, TypeOrUnionOrInterface, Value,
};

impl Schema {
    #[instrument(level = "trace", skip(self, request))]
    pub fn validate(&self, request: &Request) -> ValidationRequestOrErrors {
        if let Some(error) = validate_operation_name_uniqueness(request) {
            return vec![error].into();
        }
        if let Some(error) = validate_lone_anonymous_operation(request) {
            return vec![error].into();
        }
        let errors = validate_type_names_exist(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_selection_fields_exist(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_directives_exist(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_directives_place(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_directives_duplicate(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_no_duplicate_arguments(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_argument_names_exist(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_required_arguments(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        if let Some(error) = validate_fragment_name_uniqueness(request) {
            return vec![error].into();
        }
        let errors = validate_unused_fragments(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_fragment_spreads_exist(request, self);
        if !errors.is_empty() {
            return errors.into();
        }
        let errors = validate_fragment_spreads_relevant_type(request, self);
        if !errors.is_empty() {
            return errors.into();
        }

        ValidatedRequest::new().into()
    }
}

#[instrument(level = "trace", skip(request))]
fn validate_operation_name_uniqueness(request: &Request) -> Option<ValidationError> {
    let mut duplicates = request
        .document
        .definitions
        .iter()
        .filter_map(|definition| {
            definition
                .maybe_as_operation_definition()
                .and_then(|operation_definition| operation_definition.name.as_ref())
        })
        // TODO: could make my own eg `.duplicates_all()`
        // (returning a sub-list of each group of duplicates, not just
        // one of each duplicate group)
        .duplicates();

    duplicates.next().map(|duplicate| {
        let mut locations: Vec<Location> = _d();
        add_all_operation_name_locations(&mut locations, duplicate, request);
        let mut message = format!("Non-unique operation names: `{}`", duplicate);
        while let Some(duplicate) = duplicates.next() {
            add_all_operation_name_locations(&mut locations, duplicate, request);
            message.push_str(&format!(", `{duplicate}`"));
        }

        ValidationError::new(message, locations)
    })
}

#[instrument(level = "trace", skip(locations, request))]
fn add_all_operation_name_locations(locations: &mut Vec<Location>, name: &str, request: &Request) {
    let Some(positions_tracker) = PositionsTracker::current() else {
        return;
    };

    request
        .document
        .definitions
        .iter()
        .filter_map(|definition| definition.maybe_as_operation_definition())
        .enumerate()
        .filter_map(|(index, operation_definition)| {
            operation_definition
                .name
                .as_ref()
                .if_is(name)
                .map(|_| index)
        })
        .for_each(|index| {
            locations.push(positions_tracker.nth_operation_location(index));
        });
}

#[instrument(level = "trace", skip(request))]
fn validate_lone_anonymous_operation(request: &Request) -> Option<ValidationError> {
    if !request
        .document
        .definitions
        .iter()
        .filter(|definition| definition.maybe_as_operation_definition().is_some())
        .nth(1)
        .is_some()
    {
        return None;
    }

    if !request
        .document
        .definitions
        .iter()
        .filter(|definition| {
            definition
                .maybe_as_operation_definition()
                .is_some_and(|operation_definition| operation_definition.name.is_none())
        })
        .next()
        .is_some()
    {
        return None;
    }

    Some(ValidationError::new(
        "Anonymous operation must be only operation".into(),
        PositionsTracker::current()
            .map(|positions_tracker| {
                vec![positions_tracker.nth_operation_location(
                    request
                        .document
                        .definitions
                        .iter()
                        .filter_map(|definition| definition.maybe_as_operation_definition())
                        .enumerate()
                        .find_map(|(index, operation_definition)| {
                            match operation_definition.name.as_ref() {
                                None => Some(index),
                                Some(_) => None,
                            }
                        })
                        .unwrap(),
                )]
            })
            .unwrap_or_default(),
    ))
}

trait Collector<TItem, TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default> {
    fn visit_operation(
        &self,
        _operation: &OperationDefinition,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_fragment_definition(
        &self,
        _fragment_definition: &FragmentDefinition,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_field(
        &self,
        _field: &SelectionField,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_fragment_spread(
        &self,
        _fragment_spread: &FragmentSpread,
        _schema: &Schema,
        _request: &Request,
    ) -> TCollection {
        _d()
    }

    fn visit_inline_fragment(
        &self,
        _inline_fragment: &InlineFragment,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }
}

#[instrument(level = "trace", skip(collector, request, schema))]
fn collect<
    TItem,
    TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default,
    TCollector: Collector<TItem, TCollection>,
>(
    collector: &TCollector,
    request: &Request,
    schema: &Schema,
) -> TCollection {
    request
        .document
        .definitions
        .iter()
        .flat_map(|definition| match definition {
            ExecutableDefinition::Operation(operation_definition) => {
                let (errors, should_recurse) =
                    collector.visit_operation(operation_definition, schema, request);
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_selection_set(
                        collector,
                        &operation_definition.selection_set,
                        request,
                        schema,
                    ))
                    .collect()
            }
            ExecutableDefinition::Fragment(fragment_definition) => {
                let (errors, should_recurse) =
                    collector.visit_fragment_definition(fragment_definition, schema, request);
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_selection_set(
                        collector,
                        &fragment_definition.selection_set,
                        request,
                        schema,
                    ))
                    .collect()
            }
        })
        .collect()
}

#[instrument(level = "trace", skip(collector, selection_set, request, schema))]
fn collect_selection_set<
    TItem,
    TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default,
    TCollector: Collector<TItem, TCollection>,
>(
    collector: &TCollector,
    selection_set: &[Selection],
    request: &Request,
    schema: &Schema,
) -> TCollection {
    selection_set
        .into_iter()
        .flat_map(|selection| match selection {
            Selection::Field(field) => {
                let (errors, should_recurse) = collector.visit_field(field, schema, request);
                if !should_recurse {
                    return errors;
                }
                if let Some(selection_set) = field.selection_set.as_ref() {
                    errors
                        .into_iter()
                        .chain(collect_selection_set(
                            collector,
                            selection_set,
                            request,
                            schema,
                        ))
                        .collect()
                } else {
                    errors
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                let (errors, should_recurse) =
                    collector.visit_inline_fragment(inline_fragment, schema, request);
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_selection_set(
                        collector,
                        &inline_fragment.selection_set,
                        request,
                        schema,
                    ))
                    .collect()
            }
            Selection::FragmentSpread(fragment_spread) => {
                collector.visit_fragment_spread(fragment_spread, schema, request)
            }
        })
        .collect()
}

trait CollectorTyped<TItem, TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default>
{
    fn visit_operation(
        &self,
        _operation: &OperationDefinition,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_fragment_definition(
        &self,
        _fragment_definition: &FragmentDefinition,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_field(
        &self,
        _field: &SelectionField,
        _type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }

    fn visit_fragment_spread(
        &self,
        _fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        _request: &Request,
    ) -> TCollection {
        _d()
    }

    fn visit_inline_fragment(
        &self,
        _inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        _request: &Request,
    ) -> (TCollection, bool) {
        (_d(), true)
    }
}

#[instrument(level = "trace", skip(collector, request, schema))]
fn collect_typed<
    TItem,
    TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default,
    TCollector: CollectorTyped<TItem, TCollection>,
>(
    collector: &TCollector,
    request: &Request,
    schema: &Schema,
) -> TCollection {
    request
        .document
        .definitions
        .iter()
        .flat_map(|definition| match definition {
            ExecutableDefinition::Operation(operation_definition) => {
                let (errors, should_recurse) =
                    collector.visit_operation(operation_definition, schema, request);
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_typed_selection_set(
                        collector,
                        &operation_definition.selection_set,
                        schema.type_name_for_operation_type(operation_definition.operation_type),
                        request,
                        schema,
                    ))
                    .collect()
            }
            ExecutableDefinition::Fragment(fragment_definition) => {
                let (errors, should_recurse) =
                    collector.visit_fragment_definition(fragment_definition, schema, request);
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_typed_selection_set(
                        collector,
                        &fragment_definition.selection_set,
                        &fragment_definition.on,
                        request,
                        schema,
                    ))
                    .collect()
            }
        })
        .collect()
}

#[instrument(level = "trace", skip(collector, selection_set, request, schema))]
fn collect_typed_selection_set<
    TItem,
    TCollection: FromIterator<TItem> + IntoIterator<Item = TItem> + Default,
    TCollector: CollectorTyped<TItem, TCollection>,
>(
    collector: &TCollector,
    selection_set: &[Selection],
    enclosing_type_name: &str,
    request: &Request,
    schema: &Schema,
) -> TCollection {
    let enclosing_type = schema.type_or_union_or_interface(enclosing_type_name);
    assert!(is_non_scalar_type(&enclosing_type));
    selection_set
        .into_iter()
        .flat_map(|selection| match selection {
            Selection::Field(field) => {
                let field_type: TypeOrInterfaceField<'_> = match enclosing_type {
                    TypeOrUnionOrInterface::Type(type_) => {
                        type_.as_object().field(&field.name).into()
                    }
                    TypeOrUnionOrInterface::Interface(interface) => {
                        interface.field(&field.name).into()
                    }
                    TypeOrUnionOrInterface::Union(_) => {
                        assert!(field.name == "__typename");
                        (&schema.dummy_union_typename_field).into()
                    }
                };
                let (errors, should_recurse) =
                    collector.visit_field(field, field_type, schema, request);
                if !should_recurse {
                    return errors;
                }
                if let Some(selection_set) = field.selection_set.as_ref() {
                    errors
                        .into_iter()
                        .chain(collect_typed_selection_set(
                            collector,
                            selection_set,
                            field_type.type_().name(),
                            request,
                            schema,
                        ))
                        .collect()
                } else {
                    errors
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                let (errors, should_recurse) = collector.visit_inline_fragment(
                    inline_fragment,
                    enclosing_type,
                    schema,
                    request,
                );
                if !should_recurse {
                    return errors;
                }
                errors
                    .into_iter()
                    .chain(collect_typed_selection_set(
                        collector,
                        &inline_fragment.selection_set,
                        inline_fragment.on.as_deref().unwrap_or(enclosing_type_name),
                        request,
                        schema,
                    ))
                    .collect()
            }
            Selection::FragmentSpread(fragment_spread) => {
                collector.visit_fragment_spread(fragment_spread, enclosing_type, schema, request)
            }
        })
        .collect()
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_type_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect(&TypeNamesExistCollector::default(), request, schema)
}

#[derive(Default)]
struct TypeNamesExistCollector {}

impl Collector<ValidationError, Vec<ValidationError>> for TypeNamesExistCollector {
    #[instrument(level = "trace", skip(self, fragment_definition, schema, request))]
    fn visit_fragment_definition(
        &self,
        fragment_definition: &FragmentDefinition,
        schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            schema
                .maybe_type_or_union_or_interface(&fragment_definition.on)
                .is_none()
                .then(|| {
                    type_names_exist_validation_error(
                        &fragment_definition.on,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.fragment_definition_location(
                                fragment_definition,
                                &request.document,
                            )
                        }),
                    )
                })
                .into_iter()
                .collect(),
            true,
        )
    }

    #[instrument(level = "trace", skip(self, inline_fragment, schema, request))]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            inline_fragment
                .on
                .as_ref()
                .filter(|on| schema.maybe_type_or_union_or_interface(on).is_none())
                .map(|on| {
                    type_names_exist_validation_error(
                        on,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker
                                .inline_fragment_location(inline_fragment, &request.document)
                        }),
                    )
                })
                .into_iter()
                .collect(),
            true,
        )
    }
}

fn type_names_exist_validation_error(
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Unknown type name: `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_selection_fields_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    let mut ret: Vec<ValidationError> = _d();

    request
        .document
        .definitions
        .iter()
        .for_each(|definition| match definition {
            ExecutableDefinition::Operation(operation_definition) => {
                validate_selection_fields_exist_selection_set(
                    &operation_definition.selection_set,
                    schema.type_name_for_operation_type(operation_definition.operation_type),
                    schema,
                    request,
                    &mut ret,
                );
            }
            ExecutableDefinition::Fragment(fragment_definition) => {
                if !is_non_scalar_type(&schema.type_or_union_or_interface(&fragment_definition.on))
                {
                    ret.push(selection_on_scalar_type_fragment_validation_error(
                        &fragment_definition.name,
                        &fragment_definition.on,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.fragment_definition_location(
                                fragment_definition,
                                &request.document,
                            )
                        }),
                    ));
                    return;
                }

                validate_selection_fields_exist_selection_set(
                    &fragment_definition.selection_set,
                    &fragment_definition.on,
                    schema,
                    request,
                    &mut ret,
                );
            }
        });

    ret
}

#[instrument(level = "trace", skip(selection_set, schema, request, ret))]
fn validate_selection_fields_exist_selection_set(
    selection_set: &[Selection],
    type_name: &str,
    schema: &Schema,
    request: &Request,
    ret: &mut Vec<ValidationError>,
) {
    let type_ = schema.type_or_union_or_interface(type_name);
    assert!(is_non_scalar_type(&type_));
    for selection in selection_set {
        match selection {
            Selection::Field(field) => match type_ {
                TypeOrUnionOrInterface::Type(type_) => {
                    validate_type_or_interface_field_exists(
                        type_.as_object().maybe_field(&field.name),
                        type_name,
                        field,
                        schema,
                        request,
                        ret,
                    );
                }
                TypeOrUnionOrInterface::Union(_) => {
                    if field.name != "__typename" {
                        ret.push(selection_field_doesnt_exist_validation_error(
                            &field.name,
                            type_name,
                            PositionsTracker::current().map(|positions_tracker| {
                                positions_tracker.field_location(field, &request.document)
                            }),
                        ));
                    }
                }
                TypeOrUnionOrInterface::Interface(interface) => {
                    validate_type_or_interface_field_exists(
                        interface.maybe_field(&field.name),
                        type_name,
                        field,
                        schema,
                        request,
                        ret,
                    );
                }
            },
            Selection::InlineFragment(inline_fragment) => {
                if let Some(on) = inline_fragment
                    .on
                    .as_ref()
                    .filter(|on| !is_non_scalar_type(&schema.type_or_union_or_interface(on)))
                {
                    ret.push(selection_on_scalar_type_inline_fragment_validation_error(
                        on,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker
                                .inline_fragment_location(inline_fragment, &request.document)
                        }),
                    ));
                    continue;
                }

                validate_selection_fields_exist_selection_set(
                    &inline_fragment.selection_set,
                    inline_fragment.on.as_deref().unwrap_or(type_name),
                    schema,
                    request,
                    ret,
                );
            }
            _ => {}
        }
    }
}

#[instrument(level = "trace", skip(type_field, field, schema, request, ret))]
fn validate_type_or_interface_field_exists<TField: FieldInterface>(
    type_field: Option<&TField>,
    type_name: &str,
    field: &SelectionField,
    schema: &Schema,
    request: &Request,
    ret: &mut Vec<ValidationError>,
) {
    match type_field {
        None => {
            ret.push(selection_field_doesnt_exist_validation_error(
                &field.name,
                type_name,
                PositionsTracker::current().map(|positions_tracker| {
                    positions_tracker.field_location(field, &request.document)
                }),
            ));
        }
        Some(field_type) => {
            match (
                is_non_scalar_type(&schema.type_or_union_or_interface(field_type.type_().name())),
                field.selection_set.as_ref(),
            ) {
                (true, Some(selection_set)) => {
                    validate_selection_fields_exist_selection_set(
                        selection_set,
                        field_type.type_().name(),
                        schema,
                        request,
                        ret,
                    );
                }
                (false, None) => {}
                (true, None) => {
                    ret.push(no_selection_on_object_type_validation_error(
                        &field.name,
                        field_type.type_().name(),
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.field_location(field, &request.document)
                        }),
                    ));
                }
                (false, Some(_)) => {
                    ret.push(selection_on_scalar_type_validation_error(
                        &field.name,
                        field_type.type_().name(),
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.field_location(field, &request.document)
                        }),
                    ));
                }
            }
        }
    }
}

fn selection_field_doesnt_exist_validation_error(
    field_name: &str,
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Field `{field_name}` doesn't exist on `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

fn selection_on_scalar_type_fragment_validation_error(
    fragment_name: &str,
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Fragment `{fragment_name}` can't be of scalar type `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

fn selection_on_scalar_type_inline_fragment_validation_error(
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Inline fragment can't be of scalar type `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

fn selection_on_scalar_type_validation_error(
    field_name: &str,
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Field `{field_name}` can't have selection set because it is of scalar type `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

fn no_selection_on_object_type_validation_error(
    field_name: &str,
    type_name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Field `{field_name}` must have selection set because it is of non-scalar type `{type_name}`"),
        match location {
            Some(location) => vec![location],
            None => _d(),
        },
    )
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_argument_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&ArgumentNamesExistCollector::default(), request, schema)
}

#[derive(Default)]
struct ArgumentNamesExistCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for ArgumentNamesExistCollector {
    #[instrument(level = "trace", skip(self, field, type_field, _schema, request))]
    fn visit_field(
        &self,
        field: &SelectionField,
        type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            argument_names_exist_directives_errors(&field.directives, request)
                .into_iter()
                .chain({
                    let params = type_field.params();
                    field
                        .arguments
                        .as_ref()
                        .map(|arguments| {
                            arguments
                                .into_iter()
                                .enumerate()
                                .unique_by(|(_, argument)| &argument.name)
                                .filter(|(_, argument)| !params.contains_key(&argument.name))
                                .map(|(index, argument)| {
                                    argument_names_exist_validation_error(
                                        &argument.name,
                                        PositionsTracker::current().map(|positions_tracker| {
                                            positions_tracker.field_nth_argument_location(
                                                field,
                                                index,
                                                &request.document,
                                            )
                                        }),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                        .into_iter()
                })
                .collect(),
            true,
        )
    }

    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        argument_names_exist_directives_errors(&fragment_spread.directives, request)
    }

    #[instrument(
        level = "trace",
        skip(self, inline_fragment, _enclosing_type, _schema, request)
    )]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            argument_names_exist_directives_errors(&inline_fragment.directives, request),
            true,
        )
    }
}

fn argument_names_exist_directives_errors(
    directives: &[Directive],
    request: &Request,
) -> Vec<ValidationError> {
    directives
        .iter()
        .flat_map(|directive| argument_names_exist_directive_errors(directive, request))
        .collect()
}

fn argument_names_exist_directive_errors(
    directive: &Directive,
    request: &Request,
) -> Vec<ValidationError> {
    directive
        .arguments
        .as_ref()
        .map(|arguments| {
            arguments
                .into_iter()
                .enumerate()
                .unique_by(|(_, argument)| &argument.name)
                .filter(|(_, argument)| argument.name != "if")
                .map(|(_, argument)| {
                    argument_names_exist_validation_error(
                        &argument.name,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.directive_location(directive, &request.document)
                        }),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn argument_names_exist_validation_error(
    name: &str,
    location: Option<Location>,
) -> ValidationError {
    ValidationError::new(
        format!("Non-existent argument: `{name}`"),
        location.into_iter().collect(),
    )
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_no_duplicate_arguments(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&NoDuplicateArgumentsCollector::default(), request, schema)
}

#[derive(Default)]
struct NoDuplicateArgumentsCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for NoDuplicateArgumentsCollector {
    #[instrument(level = "trace", skip(self, field, _type_field, _schema, request))]
    fn visit_field(
        &self,
        field: &SelectionField,
        _type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            no_duplicate_arguments_directives_errors(&field.directives, request)
                .into_iter()
                .chain(
                    field
                        .arguments
                        .as_ref()
                        .map(|arguments| {
                            arguments
                                .into_iter()
                                .enumerate()
                                .into_group_map_by(|(_, argument)| argument.name.clone())
                                .into_iter()
                                .filter(|(_, arguments)| arguments.len() > 1)
                                .map(|(name, arguments)| {
                                    duplicate_argument_validation_error(
                                        &name,
                                        PositionsTracker::current()
                                            .map(|positions_tracker| {
                                                arguments
                                                    .into_iter()
                                                    .map(|(index, _)| {
                                                        positions_tracker
                                                            .field_nth_argument_location(
                                                                field,
                                                                index,
                                                                &request.document,
                                                            )
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                        .into_iter(),
                )
                .collect(),
            true,
        )
    }

    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        no_duplicate_arguments_directives_errors(&fragment_spread.directives, request)
    }

    #[instrument(
        level = "trace",
        skip(self, inline_fragment, _enclosing_type, _schema, request)
    )]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            no_duplicate_arguments_directives_errors(&inline_fragment.directives, request),
            true,
        )
    }
}

fn no_duplicate_arguments_directives_errors(
    directives: &[Directive],
    request: &Request,
) -> Vec<ValidationError> {
    directives
        .iter()
        .flat_map(|directive| no_duplicate_arguments_directive_errors(directive, request))
        .collect()
}

fn no_duplicate_arguments_directive_errors(
    directive: &Directive,
    request: &Request,
) -> Vec<ValidationError> {
    directive
        .arguments
        .as_ref()
        .map(|arguments| {
            arguments
                .into_iter()
                .enumerate()
                .into_group_map_by(|(_, argument)| argument.name.clone())
                .into_iter()
                .filter(|(_, arguments)| arguments.len() > 1)
                .map(|(name, _)| {
                    duplicate_argument_validation_error(
                        &name,
                        PositionsTracker::current()
                            .map(|positions_tracker| {
                                positions_tracker.directive_location(directive, &request.document)
                            })
                            .into_iter()
                            .collect(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn duplicate_argument_validation_error(name: &str, locations: Vec<Location>) -> ValidationError {
    ValidationError::new(format!("Duplicate argument: `{name}`"), locations)
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_required_arguments(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&RequiredArgumentCollector::default(), request, schema)
}

#[derive(Default)]
struct RequiredArgumentCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for RequiredArgumentCollector {
    #[instrument(level = "trace", skip(self, field, type_field, _schema, request))]
    fn visit_field(
        &self,
        field: &SelectionField,
        type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            {
                field
                    .directives
                    .iter()
                    .filter(doesnt_have_if_argument)
                    .map(|directive| {
                        required_argument_validation_error(
                            "if",
                            PositionsTracker::current().map(|positions_tracker| {
                                positions_tracker.directive_location(directive, &request.document)
                            }),
                        )
                    })
                    .chain(
                        type_field
                            .params()
                            .into_iter()
                            .filter(|(_, param)| matches!(param.type_, TypeFull::NonNull(_)))
                            .filter(|(name, _)| {
                                !field.arguments.as_ref().is_some_and(|arguments| {
                                    arguments.into_iter().any(|argument| {
                                        argument.name == **name
                                            && !matches!(argument.value, Value::Null)
                                    })
                                })
                            })
                            .map(|(name, _)| {
                                required_argument_validation_error(
                                    name,
                                    PositionsTracker::current().map(|positions_tracker| {
                                        positions_tracker.field_location(field, &request.document)
                                    }),
                                )
                            }),
                    )
                    .collect()
            },
            true,
        )
    }

    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        fragment_spread
            .directives
            .iter()
            .filter(doesnt_have_if_argument)
            .map(|directive| {
                required_argument_validation_error(
                    "if",
                    PositionsTracker::current().map(|positions_tracker| {
                        positions_tracker.directive_location(directive, &request.document)
                    }),
                )
            })
            .collect()
    }

    #[instrument(
        level = "trace",
        skip(self, inline_fragment, _enclosing_type, _schema, request)
    )]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            inline_fragment
                .directives
                .iter()
                .filter(doesnt_have_if_argument)
                .map(|directive| {
                    required_argument_validation_error(
                        "if",
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker.directive_location(directive, &request.document)
                        }),
                    )
                })
                .collect(),
            true,
        )
    }
}

fn doesnt_have_if_argument(directive: &&Directive) -> bool {
    !directive.arguments.as_ref().is_some_and(|arguments| {
        arguments
            .into_iter()
            .any(|argument| argument.name == "if" && !matches!(argument.value, Value::Null))
    })
}

fn required_argument_validation_error(name: &str, location: Option<Location>) -> ValidationError {
    ValidationError::new(
        format!("Missing required argument `{}`", name),
        location.into_iter().collect(),
    )
}

#[instrument(level = "trace", skip(request))]
fn validate_fragment_name_uniqueness(request: &Request) -> Option<ValidationError> {
    let mut duplicates = request
        .document
        .definitions
        .iter()
        .filter_map(|definition| {
            definition
                .maybe_as_fragment_definition()
                .map(|fragment_definition| &fragment_definition.name)
        })
        // TODO: same as validate_operation_name_uniqueness() above
        // could make my own eg `.duplicates_all()`
        // (returning a sub-list of each group of duplicates, not just
        // one of each duplicate group)
        .duplicates();

    duplicates.next().map(|duplicate| {
        let mut locations: Vec<Location> = _d();
        add_all_fragment_name_locations(&mut locations, duplicate, request);
        let mut message = format!("Non-unique fragment names: `{}`", duplicate);
        while let Some(duplicate) = duplicates.next() {
            add_all_fragment_name_locations(&mut locations, duplicate, request);
            message.push_str(&format!(", `{duplicate}`"));
        }

        ValidationError::new(message, locations)
    })
}

#[instrument(level = "trace", skip(locations, request))]
fn add_all_fragment_name_locations(locations: &mut Vec<Location>, name: &str, request: &Request) {
    let Some(positions_tracker) = PositionsTracker::current() else {
        return;
    };

    request
        .document
        .definitions
        .iter()
        .filter_map(|definition| definition.maybe_as_fragment_definition())
        .enumerate()
        .filter_map(|(index, fragment_definition)| {
            (fragment_definition.name == name).then_some(index)
        })
        .for_each(|index| {
            locations.push(positions_tracker.nth_fragment_location(index));
        });
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_unused_fragments(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    let all_used_fragment_names = collect(&FragmentNamesUsedCollector::default(), request, schema);

    request
        .document
        .definitions
        .iter()
        .filter_map(|definition| {
            definition
                .maybe_as_fragment_definition()
                .filter(|fragment_definition| {
                    !all_used_fragment_names.contains(&fragment_definition.name)
                })
        })
        .map(|fragment_definition| {
            ValidationError::new(
                format!("Unused fragment: `{}`", fragment_definition.name),
                PositionsTracker::current()
                    .map(|positions_tracker| {
                        positions_tracker
                            .fragment_definition_location(fragment_definition, &request.document)
                    })
                    .into_iter()
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

#[derive(Default)]
struct FragmentNamesUsedCollector {}

impl Collector<SmolStr, HashSet<SmolStr>> for FragmentNamesUsedCollector {
    #[instrument(level = "trace", skip(self, fragment_spread, _schema, _request))]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _schema: &Schema,
        _request: &Request,
    ) -> HashSet<SmolStr> {
        [fragment_spread.name.clone()].into()
    }
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_fragment_spreads_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&FragmentSpreadsExistCollector::default(), request, schema)
}

#[derive(Default)]
struct FragmentSpreadsExistCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for FragmentSpreadsExistCollector {
    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        (!request.document.definitions.iter().any(|definition| {
            matches!(
                definition,
                ExecutableDefinition::Fragment(fragment_definition) if fragment_definition.name == fragment_spread.name
            )
        })).then(|| {
            vec![
                ValidationError::new(
                    format!("Non-existent fragment: `{}`", fragment_spread.name),
                    PositionsTracker::current()
                        .map(|positions_tracker| {
                            positions_tracker.fragment_spread_location(
                                fragment_spread,
                                &request.document,
                            )
                        })
                        .into_iter()
                        .collect(),
                )
            ]
        }).unwrap_or_default()
    }
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_fragment_spreads_relevant_type(
    request: &Request,
    schema: &Schema,
) -> Vec<ValidationError> {
    collect_typed(
        &FragmentSpreadsRelevantTypeCollector::default(),
        request,
        schema,
    )
}

#[derive(Default)]
struct FragmentSpreadsRelevantTypeCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>>
    for FragmentSpreadsRelevantTypeCollector
{
    #[instrument(
        level = "trace",
        skip(self, fragment_spread, enclosing_type, schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        enclosing_type: TypeOrUnionOrInterface<'_>,
        schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        let fragment_definition = request
            .document
            .definitions
            .iter()
            .find_map(|definition| {
                definition
                    .maybe_as_fragment_definition()
                    .filter(|fragment_definition| fragment_definition.name == fragment_spread.name)
            })
            .unwrap();

        schema
            .all_concrete_type_names(&enclosing_type)
            .intersection(
                &schema.all_concrete_type_names_for_type_or_union_or_interface(
                    &fragment_definition.on,
                ),
            )
            .next()
            .is_none()
            .then(|| {
                vec![ValidationError::new(
                    format!(
                        "Fragment `{}` has no overlap with parent type `{}`",
                        fragment_spread.name,
                        enclosing_type.name()
                    ),
                    PositionsTracker::current()
                        .map(|positions_tracker| {
                            positions_tracker
                                .fragment_spread_location(fragment_spread, &request.document)
                        })
                        .into_iter()
                        .collect(),
                )]
            })
            .unwrap_or_default()
    }
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_directives_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&DirectivesExistCollector::default(), request, schema)
}

#[derive(Default)]
struct DirectivesExistCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for DirectivesExistCollector {
    #[instrument(level = "trace", skip(self, operation, _schema, request))]
    fn visit_operation(
        &self,
        operation: &OperationDefinition,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            operation
                .directives
                .iter()
                .filter(|directive| directive.name != "skip" && directive.name != "include")
                .map(|directive| directive_exists_validation_error(directive, request))
                .collect(),
            true,
        )
    }

    #[instrument(level = "trace", skip(self, fragment_definition, _schema, request))]
    fn visit_fragment_definition(
        &self,
        fragment_definition: &FragmentDefinition,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            fragment_definition
                .directives
                .iter()
                .filter(|directive| directive.name != "skip" && directive.name != "include")
                .map(|directive| directive_exists_validation_error(directive, request))
                .collect(),
            true,
        )
    }

    #[instrument(level = "trace", skip(self, field, _type_field, _schema, request))]
    fn visit_field(
        &self,
        field: &SelectionField,
        _type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            field
                .directives
                .iter()
                .filter(|directive| directive.name != "skip" && directive.name != "include")
                .map(|directive| directive_exists_validation_error(directive, request))
                .collect(),
            true,
        )
    }

    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        fragment_spread
            .directives
            .iter()
            .filter(|directive| directive.name != "skip" && directive.name != "include")
            .map(|directive| directive_exists_validation_error(directive, request))
            .collect()
    }

    #[instrument(
        level = "trace",
        skip(self, inline_fragment, _enclosing_type, _schema, request)
    )]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            inline_fragment
                .directives
                .iter()
                .filter(|directive| directive.name != "skip" && directive.name != "include")
                .map(|directive| directive_exists_validation_error(directive, request))
                .collect(),
            true,
        )
    }
}

fn directive_exists_validation_error(directive: &Directive, request: &Request) -> ValidationError {
    ValidationError::new(
        format!("Non-existent directive: `@{}`", directive.name),
        PositionsTracker::current()
            .map(|positions_tracker| {
                vec![positions_tracker.directive_location(directive, &request.document)]
            })
            .unwrap_or_default(),
    )
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_directives_place(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&DirectivesPlaceCollector::default(), request, schema)
}

#[derive(Default)]
struct DirectivesPlaceCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for DirectivesPlaceCollector {
    #[instrument(level = "trace", skip(self, operation, _schema, request))]
    fn visit_operation(
        &self,
        operation: &OperationDefinition,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            operation
                .directives
                .iter()
                .map(|directive| directive_place_validation_error(directive, request))
                .collect(),
            true,
        )
    }

    #[instrument(level = "trace", skip(self, fragment_definition, _schema, request))]
    fn visit_fragment_definition(
        &self,
        fragment_definition: &FragmentDefinition,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            fragment_definition
                .directives
                .iter()
                .map(|directive| directive_place_validation_error(directive, request))
                .collect(),
            true,
        )
    }
}

fn directive_place_validation_error(directive: &Directive, request: &Request) -> ValidationError {
    ValidationError::new(
        format!(
            "Directive `@{}` can't be used in this position",
            directive.name
        ),
        PositionsTracker::current()
            .map(|positions_tracker| {
                vec![positions_tracker.directive_location(directive, &request.document)]
            })
            .unwrap_or_default(),
    )
}

#[instrument(level = "trace", skip(request, schema))]
fn validate_directives_duplicate(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&DirectivesDuplicateCollector::default(), request, schema)
}

#[derive(Default)]
struct DirectivesDuplicateCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for DirectivesDuplicateCollector {
    #[instrument(level = "trace", skip(self, field, _type_field, _schema, request))]
    fn visit_field(
        &self,
        field: &SelectionField,
        _type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            field
                .directives
                .iter()
                .into_group_map_by(|directive| directive.name.clone())
                .into_iter()
                .filter(|(_, directives)| directives.len() > 1)
                .map(|(name, directives)| {
                    directive_duplicate_validation_error(&name, &directives, request)
                })
                .collect(),
            true,
        )
    }

    #[instrument(
        level = "trace",
        skip(self, fragment_spread, _enclosing_type, _schema, request)
    )]
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> Vec<ValidationError> {
        fragment_spread
            .directives
            .iter()
            .into_group_map_by(|directive| directive.name.clone())
            .into_iter()
            .filter(|(_, directives)| directives.len() > 1)
            .map(|(name, directives)| {
                directive_duplicate_validation_error(&name, &directives, request)
            })
            .collect()
    }

    #[instrument(
        level = "trace",
        skip(self, inline_fragment, _enclosing_type, _schema, request)
    )]
    fn visit_inline_fragment(
        &self,
        inline_fragment: &InlineFragment,
        _enclosing_type: TypeOrUnionOrInterface<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            inline_fragment
                .directives
                .iter()
                .into_group_map_by(|directive| directive.name.clone())
                .into_iter()
                .filter(|(_, directives)| directives.len() > 1)
                .map(|(name, directives)| {
                    directive_duplicate_validation_error(&name, &directives, request)
                })
                .collect(),
            true,
        )
    }
}

fn directive_duplicate_validation_error(
    name: &str,
    directives: &[&Directive],
    request: &Request,
) -> ValidationError {
    ValidationError::new(
        format!("Directive `@{}` can't be used more than once", name),
        PositionsTracker::current()
            .map(|positions_tracker| {
                directives
                    .into_iter()
                    .map(|directive| {
                        positions_tracker.directive_location(directive, &request.document)
                    })
                    .collect()
            })
            .unwrap_or_default(),
    )
}

#[derive(Debug)]
pub struct ValidationError {
    pub message: SmolStr,
    pub locations: Vec<Location>,
}

impl ValidationError {
    pub fn new(message: SmolStr, locations: Vec<Location>) -> Self {
        Self { message, locations }
    }
}

pub struct ValidatedRequest {}

impl ValidatedRequest {
    pub fn new() -> Self {
        Self {}
    }
}

pub enum ValidationRequestOrErrors {
    Request(ValidatedRequest),
    Errors(Vec<ValidationError>),
}

impl ValidationRequestOrErrors {
    pub fn into_errors(self) -> Vec<ValidationError> {
        match self {
            Self::Errors(errors) => errors,
            _ => panic!("Expected errors"),
        }
    }
}

impl From<ValidatedRequest> for ValidationRequestOrErrors {
    fn from(value: ValidatedRequest) -> Self {
        Self::Request(value)
    }
}

impl From<Vec<ValidationError>> for ValidationRequestOrErrors {
    fn from(value: Vec<ValidationError>) -> Self {
        Self::Errors(value)
    }
}

fn is_non_scalar_type(type_: &TypeOrUnionOrInterface) -> bool {
    match type_ {
        TypeOrUnionOrInterface::Type(Type::Object(_)) => true,
        TypeOrUnionOrInterface::Union(_) => true,
        TypeOrUnionOrInterface::Interface(_) => true,
        _ => false,
    }
}
