use std::collections::HashMap;

use derive_builder::Builder;
use rkyv::{Archive, Deserialize, Serialize};
use smol_str::SmolStr;

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

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct Document {
    pub definitions: Vec<ExecutableDefinition>,
    pub fragments_by_name: HashMap<SmolStr, usize>,
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

#[derive(Debug, Archive, Serialize, Deserialize)]
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

#[derive(Builder, Debug, Archive, Serialize, Deserialize)]
#[builder(pattern = "owned")]
pub struct OperationDefinition {
    pub operation_type: OperationType,
    #[builder(setter(into), default)]
    pub name: Option<SmolStr>,
    pub selection_set: Vec<Selection>,
    #[builder(default)]
    pub directives: Vec<Directive>,
}

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct FragmentDefinition {
    pub name: SmolStr,
    pub on: SmolStr,
    pub selection_set: Vec<Selection>,
    pub directives: Vec<Directive>,
}

impl FragmentDefinition {
    pub fn new(
        name: SmolStr,
        on: SmolStr,
        directives: Vec<Directive>,
        selection_set: Vec<Selection>,
    ) -> Self {
        Self {
            name,
            on,
            selection_set,
            directives,
        }
    }
}

#[derive(Debug, Archive, Serialize, Deserialize)]
pub enum Selection {
    Field(Field),
    FragmentSpread(FragmentSpread),
    InlineFragment(InlineFragment),
}

impl Selection {
    pub fn as_field(&self) -> &Field {
        match self {
            Self::Field(field) => field,
            _ => panic!("Expected field"),
        }
    }
}

#[derive(Builder, Debug, Archive, Serialize, Deserialize)]
#[builder(pattern = "owned")]
#[rkyv(serialize_bounds(
    __S: rkyv::ser::Writer,
    __S: rkyv::ser::Allocator,
    <__S as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
))]
#[rkyv(deserialize_bounds(
    <__D as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
))]
#[rkyv(bytecheck(
    bounds(
        __C: rkyv::validation::ArchiveContext,
        <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
    )
))]
pub struct Field {
    #[builder(setter(into), default)]
    pub alias: Option<SmolStr>,
    #[builder(setter(into))]
    pub name: SmolStr,
    #[builder(setter(strip_option), default)]
    #[rkyv(omit_bounds)]
    pub selection_set: Option<Vec<Selection>>,
    #[builder(setter(custom), default)]
    pub arguments: Option<Vec<Argument>>,
    #[builder(default)]
    pub directives: Vec<Directive>,
}

impl FieldBuilder {
    pub fn arguments(self, arguments: impl IntoIterator<Item = Argument>) -> Self {
        let mut new = self;
        new.arguments = Some(Some(arguments.into_iter().collect()));
        new
    }
}

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct FragmentSpread {
    pub name: SmolStr,
    pub directives: Vec<Directive>,
}

impl FragmentSpread {
    pub fn new(name: SmolStr, directives: Vec<Directive>) -> Self {
        Self { name, directives }
    }
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(serialize_bounds(
    __S: rkyv::ser::Writer,
    __S: rkyv::ser::Allocator,
    <__S as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
))]
#[rkyv(deserialize_bounds(
    <__D as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
))]
#[rkyv(bytecheck(
    bounds(
        __C: rkyv::validation::ArchiveContext,
        <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source,
    )
))]
pub struct InlineFragment {
    pub on: Option<SmolStr>,
    #[rkyv(omit_bounds)]
    pub selection_set: Vec<Selection>,
    pub directives: Vec<Directive>,
}

impl InlineFragment {
    pub fn new(
        on: Option<SmolStr>,
        directives: Vec<Directive>,
        selection_set: Vec<Selection>,
    ) -> Self {
        Self {
            on,
            selection_set,
            directives,
        }
    }
}

#[derive(Clone, Debug, Archive, Serialize, Deserialize)]
pub struct Argument {
    pub name: SmolStr,
    pub value: Value,
}

impl Argument {
    pub fn new(name: SmolStr, value: Value) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum Value {
    Int(i32),
    String(SmolStr),
    Null,
    Bool(bool),
    EnumVariant(SmolStr),
}

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct Directive {
    pub name: SmolStr,
    pub arguments: Option<Vec<Argument>>,
}

impl Directive {
    pub fn new(name: SmolStr, arguments: Option<Vec<Argument>>) -> Self {
        Self { name, arguments }
    }
}
