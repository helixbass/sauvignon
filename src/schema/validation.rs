use itertools::Itertools;
use squalid::{OptionExt, _d};

use crate::{
    ExecutableDefinition, FieldInterface, Location, PositionsTracker, Request, Schema, Selection,
    SelectionField, Type, TypeOrUnionOrInterface,
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
        // TODO: finish this
        // validate_argument_names_exist(request, self).append_to(&mut errors);

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

fn validate_type_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    let mut ret: Vec<ValidationError> = _d();

    request
        .document
        .definitions
        .iter()
        .for_each(|definition| match definition {
            ExecutableDefinition::Operation(operation) => {
                validate_type_names_exist_selection_set(
                    &operation.selection_set,
                    schema,
                    request,
                    &mut ret,
                );
            }
            ExecutableDefinition::Fragment(fragment) => {
                if schema
                    .maybe_type_or_union_or_interface(&fragment.on)
                    .is_none()
                {
                    ret.push(type_names_exist_validation_error(
                        &fragment.on,
                        PositionsTracker::current().map(|positions_tracker| {
                            positions_tracker
                                .fragment_definition_location(fragment, &request.document)
                        }),
                    ));
                }
                validate_type_names_exist_selection_set(
                    &fragment.selection_set,
                    schema,
                    request,
                    &mut ret,
                );
            }
        });

    ret
}

fn validate_type_names_exist_selection_set(
    selection_set: &[Selection],
    schema: &Schema,
    request: &Request,
    ret: &mut Vec<ValidationError>,
) {
    selection_set
        .into_iter()
        .for_each(|selection| match selection {
            Selection::Field(field) => {
                if let Some(selection_set) = field.selection_set.as_ref() {
                    validate_type_names_exist_selection_set(selection_set, schema, request, ret);
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                if let Some(on) = inline_fragment.on.as_ref() {
                    if schema.maybe_type_or_union_or_interface(on).is_none() {
                        ret.push(type_names_exist_validation_error(
                            on,
                            PositionsTracker::current().map(|positions_tracker| {
                                positions_tracker
                                    .inline_fragment_location(inline_fragment, &request.document)
                            }),
                        ));
                    }
                }
            }
            _ => {}
        });
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

fn validate_argument_names_exist(request: &Request, schema: &Schema) -> Vec<ValidationError> {
    let mut ret: Vec<ValidationError> = _d();

    request
        .document
        .definitions
        .iter()
        .for_each(|definition| match definition {
            ExecutableDefinition::Operation(operation_definition) => {
                validate_argument_names_exist_selection_set(
                    &operation_definition.selection_set,
                    schema,
                    &mut ret,
                );
            }
            ExecutableDefinition::Fragment(fragment_definition) => {
                validate_argument_names_exist_selection_set(
                    &fragment_definition.selection_set,
                    schema,
                    &mut ret,
                );
            }
        });

    ret
}

fn validate_argument_names_exist_selection_set(
    selection_set: &[Selection],
    schema: &Schema,
    ret: &mut Vec<ValidationError>,
) {
    unimplemented!();
    selection_set
        .into_iter()
        .for_each(|selection| match selection {
            Selection::Field(field) => {}
            Selection::InlineFragment(inline_fragment) => {}
            _ => {}
        })
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
