use std::mem;

use chrono::NaiveDate;
use jiff::Timestamp;
use serde::{Serialize, Serializer};
use smol_str::{format_smolstr, SmolStr};
use squalid::_d;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    ExternalDependencyValues, FieldPlan, IndexMap, Location, ParseOrLexError, ValidationError,
};

#[derive(Serialize)]
pub struct Response<'a> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ResponseError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseValue<'a>>,
}

impl<'a> Response<'a> {
    pub fn new(data: Option<ResponseValue<'a>>, errors: Vec<ResponseError>) -> Self {
        Self { data, errors }
    }
}

impl<'a> From<ResponseValue<'a>> for Response<'a> {
    fn from(value: ResponseValue<'a>) -> Self {
        Self {
            data: Some(value),
            errors: _d(),
        }
    }
}

impl From<Vec<ResponseError>> for Response<'_> {
    fn from(value: Vec<ResponseError>) -> Self {
        Self {
            data: _d(),
            errors: value,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ResponseValue<'a> {
    Null,
    List(Vec<ResponseValue<'a>>),
    Map(IndexMap<SmolStr, ResponseValue<'a>>),
    String(SmolStr),
    Boolean(bool),
    Int(i32),
    Float(f64),
    EnumValue(SmolStr),
    Uuid(Uuid),
    ListIter(ResponseValueListIterWrapper<'a>),
    // MapIter(ResponseValueMapIter),
}

pub struct ResponseValueListIterWrapper<'a>(Box<dyn CloneResponseValueIterator<'a> + 'a>);

impl<'a> ResponseValueListIterWrapper<'a> {
    fn from_inner<TIterator: Iterator<Item = ResponseValue<'a>> + Clone + Send + Sync + 'a>(
        value: ResponseValueListIter<'a, TIterator>,
    ) -> Self {
        Self(Box::new(value))
    }
}

impl Serialize for ResponseValueListIterWrapper<'_> {
    fn serialize<TSerializer>(
        &self,
        serializer: TSerializer,
    ) -> Result<TSerializer::Ok, TSerializer::Error>
    where
        TSerializer: Serializer,
    {
        // TODO: is there a way to avoid `unsafe` here?
        // safety: `.collect_seq()` immediately consumes
        // the entire iterator
        unsafe {
            serializer.collect_seq(
                mem::transmute::<
                    _,
                    &'static Box<dyn CloneResponseValueIterator<'static> + 'static>,
                >(&self.0)
                .cloned()
            )
        }
    }
}

pub struct ResponseValueListIter<
    'a,
    TIterator: Iterator<Item = ResponseValue<'a>> + Clone + Send + Sync,
> {
    iterator: TIterator,
}

impl<'a, TIterator: Iterator<Item = ResponseValue<'a>> + Clone + Send + Sync>
    ResponseValueListIter<'a, TIterator>
{
    pub fn new(iterator: TIterator) -> Self {
        Self { iterator }
    }
}

pub trait CloneResponseValueIterator<'a>: Send + Sync {
    fn cloned(&'a self) -> Box<dyn Iterator<Item = ResponseValue<'a>> + 'a>;
}

impl<'a, TIterator: Iterator<Item = ResponseValue<'a>> + Clone + Send + Sync>
    CloneResponseValueIterator<'a> for ResponseValueListIter<'a, TIterator>
{
    fn cloned(&'a self) -> Box<dyn Iterator<Item = ResponseValue<'a>> + 'a> {
        Box::new(self.iterator.clone())
    }
}

// pub struct ResponseValueMapIter(Box<dyn CloneResponseValueMapIterator>);

// impl Serialize for ResponseValueMapIter {
//     fn serialize<TSerializer>(
//         &self,
//         serializer: TSerializer,
//     ) -> Result<TSerializer::Ok, TSerializer::Error>
//     where
//         TSerializer: Serializer,
//     {
//         serializer.collect_map(self.0.cloned())
//     }
// }

// pub struct ResponseValueMapIterInner<TIterator: Iterator<Item = (SmolStr, ResponseValue)> + Clone> {
//     iterator: TIterator,
// }

// impl<TIterator: Iterator<Item = (SmolStr, ResponseValue)> + Clone>
//     ResponseValueMapIterInner<TIterator>
// {
//     pub fn new(iterator: TIterator) -> Self {
//         Self { iterator }
//     }
// }

// pub trait CloneResponseValueMapIterator: Send + Sync {
//     fn cloned<'a>(&'a self) -> Box<dyn Iterator<Item = (SmolStr, ResponseValue)> + 'a>;
// }

// impl<TIterator: Iterator<Item = (SmolStr, ResponseValue)> + Clone + Send + Sync>
//     CloneResponseValueMapIterator for ResponseValueMapIterInner<TIterator>
// {
//     fn cloned<'a>(&'a self) -> Box<dyn Iterator<Item = (SmolStr, ResponseValue)> + 'a> {
//         Box::new(self.iterator.clone())
//     }
// }

impl<'a, 'b> From<FieldsInProgress<'a, 'b>> for ResponseValue<'b> {
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

impl From<bool> for ResponseValue<'_> {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i32> for ResponseValue<'_> {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for ResponseValue<'_> {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<Timestamp> for ResponseValue<'_> {
    fn from(value: Timestamp) -> Self {
        Self::String(format_smolstr!("{}", value))
    }
}

impl<'a> From<&'a NaiveDate> for ResponseValue<'_> {
    fn from(value: &'a NaiveDate) -> Self {
        Self::String(format_smolstr!("{}", value))
    }
}

impl<'a, TInner: Into<ResponseValue<'a>>> From<Option<TInner>> for ResponseValue<'a> {
    fn from(value: Option<TInner>) -> Self {
        match value {
            None => Self::Null,
            Some(value) => value.into(),
        }
    }
}

impl<'a, TInner: Into<ResponseValue<'a>>> From<Vec<TInner>> for ResponseValue<'a> {
    fn from(value: Vec<TInner>) -> Self {
        Self::List(value.into_iter().map(Into::into).collect())
    }
}

pub type FieldsInProgress<'a, 'b> = IndexMap<SmolStr, ResponseValueOrInProgress<'a, 'b>>;

#[instrument(level = "trace", skip(field_plans, external_dependency_values))]
pub fn fields_in_progress_new<'a, 'b>(
    field_plans: &'a IndexMap<SmolStr, FieldPlan<'a>>,
    external_dependency_values: &ExternalDependencyValues,
) -> FieldsInProgress<'a, 'b> {
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

pub struct ResponseInProgress<'a, 'b> {
    pub fields: FieldsInProgress<'a, 'b>,
}

impl<'a, 'b> ResponseInProgress<'a, 'b> {
    pub fn new(fields: FieldsInProgress<'a, 'b>) -> Self {
        Self { fields }
    }
}

pub enum ResponseValueOrInProgress<'a, 'b> {
    ResponseValue(ResponseValue<'b>),
    InProgress(InProgress<'a>),
    InProgressRecursing(InProgressRecursing<'a, 'b>),
    InProgressRecursingList(InProgressRecursingList<'a, 'b>),
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

pub struct InProgressRecursing<'a, 'b> {
    pub field_plan: &'a FieldPlan<'a>,
    pub populated: ExternalDependencyValues,
    pub selection: FieldsInProgress<'a, 'b>,
}

impl<'a, 'b> InProgressRecursing<'a, 'b> {
    pub fn new(
        field_plan: &'a FieldPlan<'a>,
        populated: ExternalDependencyValues,
        selection: FieldsInProgress<'a, 'b>,
    ) -> Self {
        Self {
            field_plan,
            populated,
            selection,
        }
    }
}

pub struct InProgressRecursingList<'a, 'b> {
    pub field_plan: &'a FieldPlan<'a>,
    pub populated: Vec<ExternalDependencyValues>,
    pub selections: Vec<FieldsInProgress<'a, 'b>>,
}

impl<'a, 'b> InProgressRecursingList<'a, 'b> {
    pub fn new(
        field_plan: &'a FieldPlan<'a>,
        populated: Vec<ExternalDependencyValues>,
        selections: Vec<FieldsInProgress<'a, 'b>>,
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
    pub message: SmolStr,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
}

impl ResponseError {
    pub fn new(message: SmolStr, locations: Vec<Location>) -> Self {
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
            value.message().into(),
            value.location().into_iter().collect(),
        )
    }
}
