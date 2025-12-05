use serde::Serialize;
use squalid::_d;
use tracing::instrument;

use crate::{
    ExternalDependencyValues, FieldPlan, IndexMap, Location, ParseOrLexError, ValidationError,
};

#[derive(Serialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ResponseError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseValue>,
}

impl Response {
    pub fn new(data: Option<ResponseValue>, errors: Vec<ResponseError>) -> Self {
        Self { data, errors }
    }
}

impl From<ResponseValue> for Response {
    fn from(value: ResponseValue) -> Self {
        Self {
            data: Some(value),
            errors: _d(),
        }
    }
}

impl From<Vec<ResponseError>> for Response {
    fn from(value: Vec<ResponseError>) -> Self {
        Self {
            data: _d(),
            errors: value,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ResponseValue {
    Null,
    List(Vec<ResponseValue>),
    Map(IndexMap<String, ResponseValue>),
    String(String),
    Boolean(bool),
    Int(i32),
    Float(f64),
    EnumValue(String),
}

impl From<FieldsInProgress<'_>> for ResponseValue {
    fn from(fields_in_progress: FieldsInProgress) -> Self {
        Self::Map(
            fields_in_progress
                .into_iter()
                .map(|(name, response_value_or_in_progress)| {
                    (
                        name,
                        match response_value_or_in_progress {
                            ResponseValueOrInProgress::ResponseValue(response_value) => {
                                response_value
                            }
                            _ => unreachable!(),
                        },
                    )
                })
                .collect(),
        )
    }
}

impl From<Vec<FieldsInProgress<'_>>> for ResponseValue {
    fn from(fields_in_progress: Vec<FieldsInProgress>) -> Self {
        Self::List(fields_in_progress.into_iter().map(Into::into).collect())
    }
}

pub type FieldsInProgress<'a> = IndexMap<String, ResponseValueOrInProgress<'a>>;

#[instrument(level = "trace", skip(field_plans, external_dependency_values))]
pub fn fields_in_progress_new<'a>(
    field_plans: &'a IndexMap<String, FieldPlan<'a>>,
    external_dependency_values: &ExternalDependencyValues,
) -> FieldsInProgress<'a> {
    // TODO: this looks like a map_values()
    field_plans
        .into_iter()
        .map(|(field_name, field_plan)| {
            (
                field_name.clone(),
                ResponseValueOrInProgress::InProgress(InProgress::new(
                    field_plan,
                    external_dependency_values.clone(),
                )),
            )
        })
        .collect()
}

pub struct ResponseInProgress<'a> {
    pub fields: FieldsInProgress<'a>,
}

impl<'a> ResponseInProgress<'a> {
    pub fn new(fields: FieldsInProgress<'a>) -> Self {
        Self { fields }
    }
}

pub enum ResponseValueOrInProgress<'a> {
    ResponseValue(ResponseValue),
    InProgress(InProgress<'a>),
    InProgressRecursing(InProgressRecursing<'a>),
    InProgressRecursingList(InProgressRecursingList<'a>),
}

pub struct InProgress<'a> {
    pub field_plan: &'a FieldPlan<'a>,
    pub external_dependency_values: ExternalDependencyValues,
}

impl<'a> InProgress<'a> {
    pub fn new(
        field_plan: &'a FieldPlan<'a>,
        external_dependency_values: ExternalDependencyValues,
    ) -> Self {
        Self {
            field_plan,
            external_dependency_values,
        }
    }
}

pub struct InProgressRecursing<'a> {
    pub field_plan: &'a FieldPlan<'a>,
    pub populated: ExternalDependencyValues,
    pub selection: FieldsInProgress<'a>,
}

impl<'a> InProgressRecursing<'a> {
    pub fn new(
        field_plan: &'a FieldPlan<'a>,
        populated: ExternalDependencyValues,
        selection: FieldsInProgress<'a>,
    ) -> Self {
        Self {
            field_plan,
            populated,
            selection,
        }
    }
}

pub struct InProgressRecursingList<'a> {
    pub field_plan: &'a FieldPlan<'a>,
    pub populated: Vec<ExternalDependencyValues>,
    pub selections: Vec<FieldsInProgress<'a>>,
}

impl<'a> InProgressRecursingList<'a> {
    pub fn new(
        field_plan: &'a FieldPlan<'a>,
        populated: Vec<ExternalDependencyValues>,
        selections: Vec<FieldsInProgress<'a>>,
    ) -> Self {
        Self {
            field_plan,
            populated,
            selections,
        }
    }
}

#[derive(Serialize)]
pub struct ResponseError {
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
}

impl ResponseError {
    pub fn new(message: String, locations: Vec<Location>) -> Self {
        Self { message, locations }
    }
}

impl From<ValidationError> for ResponseError {
    fn from(value: ValidationError) -> Self {
        Self::new(value.message, value.locations)
    }
}

impl From<ParseOrLexError> for ResponseError {
    fn from(value: ParseOrLexError) -> Self {
        Self::new(
            value.message().to_owned(),
            value.location().into_iter().collect(),
        )
    }
}
