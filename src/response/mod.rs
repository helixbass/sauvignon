use crate::{FieldPlan, IndexMap};

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

pub struct ResponseInProgress<'a> {
    fields: IndexMap<String, ResponseValueOrInProgress<'a>>,
}

impl<'a> ResponseInProgress<'a> {
    pub fn new(fields: IndexMap<String, ResponseValueOrInProgress<'a>>) -> Self {
        Self { fields }
    }
}

pub enum ResponseValueOrInProgress<'a> {
    ResponseValue(ResponseValue),
    InProgress(&'a FieldPlan<'a>),
}
