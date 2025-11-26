use std::collections::HashMap;

use derive_builder::Builder;

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

    pub fn fragment(&self, name: &str) -> &FragmentDefinition {
        self.document.fragment(name)
    }
}

pub struct Document {
    pub definitions: Vec<ExecutableDefinition>,
    pub fragments_by_name: HashMap<String, usize>,
}

impl Document {
    pub fn new(definitions: Vec<ExecutableDefinition>) -> Self {
        let fragments_by_name = definitions
            .iter()
            .enumerate()
            .filter_map(|(index, definition)| match definition {
                ExecutableDefinition::Fragment(fragment) => Some((fragment.name.clone(), index)),
                _ => None,
            })
            .collect();
        Self {
            definitions,
            fragments_by_name,
        }
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

    pub fn fragment(&self, name: &str) -> &FragmentDefinition {
        self.definitions[*self.fragments_by_name.get(name).unwrap()].as_fragment_definition()
    }
}

pub enum ExecutableDefinition {
    Operation(OperationDefinition),
    Fragment(FragmentDefinition),
}

impl ExecutableDefinition {
    pub fn as_fragment_definition(&self) -> &FragmentDefinition {
        match self {
            Self::Fragment(fragment_definition) => fragment_definition,
            _ => panic!("expected fragment"),
        }
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct OperationDefinition {
    pub operation_type: OperationType,
    #[builder(setter(into), default)]
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

pub struct FragmentDefinition {
    pub name: String,
    pub on: String,
    pub selection_set: Vec<Selection>,
}

impl FragmentDefinition {
    pub fn new(name: String, on: String, selection_set: Vec<Selection>) -> Self {
        Self {
            name,
            on,
            selection_set,
        }
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
    pub selection_set: Option<Vec<Selection>>,
    pub arguments: Option<HashMap<String, Argument>>,
}

impl Field {
    pub fn new(
        alias: Option<String>,
        name: String,
        selection_set: Option<Vec<Selection>>,
        arguments: Option<Vec<Argument>>,
    ) -> Self {
        Self {
            alias,
            name,
            selection_set,
            arguments: arguments.map(|arguments| {
                arguments
                    .into_iter()
                    .map(|argument| (argument.name.clone(), argument))
                    .collect()
            }),
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

pub struct InlineFragment {
    pub on: Option<String>,
    pub selection_set: Vec<Selection>,
}

impl InlineFragment {
    pub fn new(on: Option<String>, selection_set: Vec<Selection>) -> Self {
        Self { on, selection_set }
    }
}

pub struct Argument {
    pub name: String,
    pub value: Value,
}

impl Argument {
    pub fn new(name: String, value: Value) -> Self {
        Self { name, value }
    }
}

pub enum Value {
    Int(i32),
    String(String),
}
