use crate::OperationType;

pub struct Request {}

pub struct Document {
    pub definitions: Vec<ExecutableDefinition>,
}

impl Document {
    pub fn new(definitions: Vec<ExecutableDefinition>) -> Self {
        Self { definitions }
    }
}

enum ExecutableDefinition {
    Operation(OperationDefinition),
    Fragment(FragmentDefinition),
}

pub struct OperationDefinition {
    pub operation_type: OperationType,
    pub name: Option<String>,
    pub selection_set: SelectionSet,
}

pub struct FragmentDefinition {}

pub struct SelectionSet {
    pub selections: Vec<Selection>,
}

impl SelectionSet {
    pub fn new(selections: Vec<Selection>) -> Self {
        Self { selections }
    }
}

pub enum Selection {
    Field(Field),
    FragmentSpread(FragmentSpread),
    InlineFragment(InlineFragment),
}

pub struct Field {
    pub alias: Option<String>,
    pub name: String,
    pub selection_set: Option<SelectionSet>,
}

impl Field {
    pub fn new(alias: Option<String>, name: String, selection_set: Option<SelectionSet>) -> Self {
        Self {
            alias,
            name,
            selection_set,
        }
    }
}

pub struct FragmentSpread {}

pub struct InlineFragment {}
