use std::collections::HashMap;

use derive_builder::Builder;

use crate::OperationType;

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub enum ExecutableDefinition {
    Operation(OperationDefinition),
    Fragment(FragmentDefinition),
}

impl ExecutableDefinition {
    pub fn maybe_as_operation_definition(&self) -> Option<&OperationDefinition> {
        match self {
            Self::Operation(operation_definition) => Some(operation_definition),
            _ => None,
        }
    }

    pub fn maybe_as_fragment_definition(&self) -> Option<&FragmentDefinition> {
        match self {
            Self::Fragment(fragment_definition) => Some(fragment_definition),
            _ => None,
        }
    }

    pub fn as_fragment_definition(&self) -> &FragmentDefinition {
        self.maybe_as_fragment_definition()
            .expect("expected fragment")
    }
}

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct OperationDefinition {
    pub operation_type: OperationType,
    #[builder(setter(into), default)]
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Selection {
    Field(Field),
    FragmentSpread(FragmentSpread),
    InlineFragment(InlineFragment),
}

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct Field {
    #[builder(setter(into), default)]
    pub alias: Option<String>,
    #[builder(setter(into))]
    pub name: String,
    #[builder(setter(strip_option), default)]
    pub selection_set: Option<Vec<Selection>>,
    #[builder(setter(custom), default)]
    pub arguments: Option<Vec<Argument>>,
}

impl FieldBuilder {
    pub fn arguments(self, arguments: impl IntoIterator<Item = Argument>) -> Self {
        let mut new = self;
        new.arguments = Some(Some(arguments.into_iter().collect()));
        new
    }
}

#[derive(Debug)]
pub struct FragmentSpread {
    pub name: String,
}

impl FragmentSpread {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[derive(Debug)]
pub struct InlineFragment {
    pub on: Option<String>,
    pub selection_set: Vec<Selection>,
}

impl InlineFragment {
    pub fn new(on: Option<String>, selection_set: Vec<Selection>) -> Self {
        Self { on, selection_set }
    }
}

#[derive(Clone, Debug)]
pub struct Argument {
    pub name: String,
    pub value: Value,
}

impl Argument {
    pub fn new(name: String, value: Value) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i32),
    String(String),
    Null,
}
