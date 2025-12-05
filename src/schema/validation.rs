use std::collections::HashSet;

use itertools::Itertools;
use squalid::{OptionExt, _d};

use crate::{
    ExecutableDefinition, FieldInterface, FragmentDefinition, FragmentSpread, InlineFragment,
    Location, OperationDefinition, PositionsTracker, Request, Schema, Selection, SelectionField,
    Type, TypeOrInterfaceField, TypeOrUnionOrInterface,
};

impl Schema {
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
        let errors = validate_argument_names_exist(request, self);
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

        ValidatedRequest::new().into()
    }
}

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
        "Anonymous operation must be only operation".to_owned(),
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

fn validate_type_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect(&TypeNamesExistCollector::default(), request, schema)
}

#[derive(Default)]
struct TypeNamesExistCollector {}

impl Collector<ValidationError, Vec<ValidationError>> for TypeNamesExistCollector {
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

#[derive(Default)]
struct ArgumentNamesExistCollector {}

impl CollectorTyped<ValidationError, Vec<ValidationError>> for ArgumentNamesExistCollector {
    fn visit_field(
        &self,
        field: &SelectionField,
        type_field: TypeOrInterfaceField<'_>,
        _schema: &Schema,
        request: &Request,
    ) -> (Vec<ValidationError>, bool) {
        (
            {
                let params = type_field.params();
                field
                    .arguments
                    .as_ref()
                    .map(|arguments| {
                        arguments
                            .keys()
                            .enumerate()
                            .filter(|(_index, key)| !params.contains_key(&**key))
                            .map(|(index, key)| {
                                ValidationError::new(
                                    format!("Non-existent argument: `{key}`"),
                                    PositionsTracker::current()
                                        .map(|positions_tracker| {
                                            positions_tracker.field_nth_argument_location(
                                                field,
                                                index,
                                                &request.document,
                                            )
                                        })
                                        .into_iter()
                                        .collect(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            },
            true,
        )
    }
}

fn validate_argument_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    collect_typed(&ArgumentNamesExistCollector::default(), request, schema)
}

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

impl Collector<String, HashSet<String>> for FragmentNamesUsedCollector {
    fn visit_fragment_spread(
        &self,
        fragment_spread: &FragmentSpread,
        _schema: &Schema,
        _request: &Request,
    ) -> HashSet<String> {
        [fragment_spread.name.clone()].into()
    }
}

#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
    pub locations: Vec<Location>,
}

impl ValidationError {
    pub fn new(message: String, locations: Vec<Location>) -> Self {
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
