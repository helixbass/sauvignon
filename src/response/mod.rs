use crate::{ExternalDependencyValues, FieldPlan, IndexMap};

pub struct Response {}

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
        Self::Map(IndexMap::from_iter(fields_in_progress.into_iter().map(
            |(name, response_value_or_in_progress)| {
                (
                    name,
                    match response_value_or_in_progress {
                        ResponseValueOrInProgress::ResponseValue(response_value) => response_value,
                        _ => unreachable!(),
                    },
                )
            },
        )))
    }
}

pub type FieldsInProgress<'a> = IndexMap<String, ResponseValueOrInProgress<'a>>;

pub fn fields_in_progress_new<'a>(
    field_plans: &'a [FieldPlan<'a>],
    external_dependency_values: &ExternalDependencyValues,
) -> FieldsInProgress<'a> {
    IndexMap::from_iter(field_plans.into_iter().map(|field_plan| {
        (
            field_plan.request_field.name.clone(),
            ResponseValueOrInProgress::InProgress(InProgress::new(
                field_plan,
                external_dependency_values.clone(),
            )),
        )
    }))
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
