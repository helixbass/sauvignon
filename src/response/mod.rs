use chrono::NaiveDate;
use jiff::Timestamp;
use serde::Serialize;
use smol_str::{format_smolstr, SmolStr};
use squalid::_d;
use uuid::Uuid;

use crate::{IndexMap, Location, ParseOrLexError, ValidationError};

mod produce;

pub use produce::produce_response;

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
    Map(IndexMap<SmolStr, ResponseValue>),
    String(SmolStr),
    Boolean(bool),
    Int(i32),
    Float(f64),
    EnumValue(SmolStr),
    Uuid(Uuid),
}

impl From<bool> for ResponseValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i32> for ResponseValue {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for ResponseValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<Timestamp> for ResponseValue {
    fn from(value: Timestamp) -> Self {
        Self::String(format_smolstr!("{}", value))
    }
}

impl<'a> From<&'a NaiveDate> for ResponseValue {
    fn from(value: &'a NaiveDate) -> Self {
        Self::String(format_smolstr!("{}", value))
    }
}

impl<TInner: Into<ResponseValue>> From<Option<TInner>> for ResponseValue {
    fn from(value: Option<TInner>) -> Self {
        match value {
            None => Self::Null,
            Some(value) => value.into(),
        }
    }
}

impl<TInner: Into<ResponseValue>> From<Vec<TInner>> for ResponseValue {
    fn from(value: Vec<TInner>) -> Self {
        Self::List(value.into_iter().map(Into::into).collect())
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
