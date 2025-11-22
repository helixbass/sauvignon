use crate::OperationType;

pub struct Request {
    pub document: Document,
}

impl Request {
    pub fn new(document: Document) -> Self {
        Self { document }
    }

    pub fn chosen_operation(&self) -> &OperationDefinition {
        self.document.chosen_operation()
    }
}

pub struct Document {
    pub definitions: Vec<ExecutableDefinition>,
}

impl Document {
    pub fn new(definitions: Vec<ExecutableDefinition>) -> Self {
        Self { definitions }
    }

    pub fn chosen_operation(&self) -> &OperationDefinition {
        for definition in self.definitions.iter() {
            match definition {
                ExecutableDefinition::Operation(operation_definition) => {
                    return operation_definition;
                }
                _ => continue,
            }
        }
        panic!()
    }
}

pub enum ExecutableDefinition {
    Operation(OperationDefinition),
    Fragment(FragmentDefinition),
}

pub struct OperationDefinition {
    pub operation_type: OperationType,
    pub name: Option<String>,
    pub selection_set: SelectionSet,
}

impl OperationDefinition {
    pub fn new(
        operation_type: OperationType,
        name: Option<String>,
        selection_set: SelectionSet,
    ) -> Self {
        Self {
            operation_type,
            name,
            selection_set,
        }
    }
}

pub struct FragmentDefinition {
    pub name: String,
    pub on: String,
    pub selection_set: SelectionSet,
}

impl FragmentDefinition {
    pub fn new(name: String, on: String, selection_set: SelectionSet) -> Self {
        Self {
            name,
            on,
            selection_set,
        }
    }
}

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

pub struct FragmentSpread {
    pub name: String,
}

impl FragmentSpread {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub struct InlineFragment {}
