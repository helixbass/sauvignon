use std::{cell::RefCell, fmt::Debug, iter::Peekable, ops::Deref, ptr};

use serde::Serialize;
use squalid::{EverythingExt, _d};

use crate::ExecutableDefinition;

pub struct CharsEmitter<TIterator: Iterator<Item = char>> {
    inner: Peekable<TIterator>,
}

impl<TIterator: Iterator<Item = char>> CharsEmitter<TIterator> {
    pub fn new(inner: Peekable<TIterator>) -> Self {
        Self { inner }
    }

    pub fn peek(&mut self) -> Option<&char> {
        self.inner.peek()
    }
}

impl<TIterator: Iterator<Item = char>> Iterator for CharsEmitter<TIterator> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.inner
            .next()
            .inspect(|ch| PositionsTracker::emit_char(*ch))
    }
}

#[derive(Debug, Default)]
pub struct PositionsTracker {
    last_char: RefCell<Option<Location>>,
    last_token_start: RefCell<Option<Location>>,
    just_saw_carriage_return: RefCell<bool>,
    just_newlined: RefCell<bool>,
    should_next_char_record_as_token_start: RefCell<bool>,
    document: RefCell<Document>,
}

impl PositionsTracker {
    pub fn receive_char(&self, ch: char) {
        if *self.just_saw_carriage_return.borrow() {
            if ch != '\n' {
                *self.just_newlined.borrow_mut() = true;
            }
        }
        let just_newlined = { *self.just_newlined.borrow() };
        *self.just_saw_carriage_return.borrow_mut() = ch == '\r';
        *self.just_newlined.borrow_mut() = ch == '\n';
        let last_char = { *self.last_char.borrow() };
        let new_last_char = match last_char {
            Some(last_char) => {
                if just_newlined {
                    Location::new(last_char.line + 1, 1)
                } else {
                    Location::new(last_char.line, last_char.column + 1)
                }
            }
            None => Location::new(1, 1),
        };
        if *self.should_next_char_record_as_token_start.borrow() {
            *self.last_token_start.borrow_mut() = Some(new_last_char);
        }
        *self.should_next_char_record_as_token_start.borrow_mut() = false;
        *self.last_char.borrow_mut() = Some(new_last_char);
    }

    fn last_token(&self) -> Location {
        self.last_token_start.borrow().clone().unwrap()
    }

    pub fn receive_operation(&self) {
        self.document
            .borrow_mut()
            .definitions
            .push(OperationOrFragment::Operation(Operation::new(
                self.last_token(),
            )));
    }

    pub fn receive_fragment_definition(&self) {
        self.document
            .borrow_mut()
            .definitions
            .push(OperationOrFragment::Fragment(FragmentDefinition::new(
                self.last_token(),
            )));
    }

    pub fn receive_selection_set(&self) {
        let mut document = self.document.borrow_mut();
        let currently_active_selection_set = document.find_currently_active_selection_set();
        match currently_active_selection_set {
            None => match document
                .definitions
                .iter_mut()
                .find(|definition| match definition {
                    OperationOrFragment::Operation(operation) => {
                        operation.selection_set.status == SelectionSetStatus::NotYetStarted
                    }
                    OperationOrFragment::Fragment(fragment) => {
                        fragment.selection_set.status == SelectionSetStatus::NotYetStarted
                    }
                })
                .unwrap()
            {
                OperationOrFragment::Operation(operation) => operation.selection_set.open(),
                OperationOrFragment::Fragment(fragment) => fragment.selection_set.open(),
            },
            Some(currently_active_selection_set) => {
                find_selection_to_open(&mut currently_active_selection_set.selections).open()
            }
        }
    }

    pub fn receive_end_selection_set(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .close();
    }

    pub fn receive_selection_field(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::Field(Field::new(self.last_token())));
    }

    pub fn receive_selection_inline_fragment(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::InlineFragment(InlineFragment::new(
                self.last_token(),
            )));
    }

    pub fn receive_selection_fragment_spread(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::FragmentSpread);
    }

    pub fn receive_token_pre_start(&self) {
        *self.should_next_char_record_as_token_start.borrow_mut() = true;
    }

    pub fn nth_operation_location(&self, index: usize) -> Location {
        self.document
            .borrow()
            .definitions
            .iter()
            .filter_map(|definition| definition.maybe_as_operation())
            .nth(index)
            .unwrap()
            .location
    }

    pub fn fragment_definition_location(
        &self,
        fragment: &crate::FragmentDefinition,
        document: &crate::Document,
    ) -> Location {
        let index = document.definitions.iter().position(|definition| {
            matches!(
                definition,
                ExecutableDefinition::Fragment(fragment_definition) if ptr::eq(fragment_definition, fragment)
            )
        }).unwrap();
        self.document.borrow().definitions[index]
            .as_fragment_definition()
            .location
    }

    pub fn inline_fragment_location(
        &self,
        inline_fragment: &crate::InlineFragment,
        document: &crate::Document,
    ) -> Location {
        document
            .definitions
            .iter()
            .zip(self.document.borrow().definitions.iter())
            .find_map(|(definition, definition_positions)| {
                match (definition, definition_positions) {
                    (
                        ExecutableDefinition::Operation(operation_definition),
                        OperationOrFragment::Operation(operation_positions),
                    ) => maybe_inline_fragment_location_selection_set(
                        inline_fragment,
                        &operation_definition.selection_set,
                        &operation_positions.selection_set,
                    ),
                    (
                        ExecutableDefinition::Fragment(fragment_definition),
                        OperationOrFragment::Fragment(fragment_positions),
                    ) => maybe_inline_fragment_location_selection_set(
                        inline_fragment,
                        &fragment_definition.selection_set,
                        &fragment_positions.selection_set,
                    ),
                    _ => unreachable!(),
                }
            })
            .unwrap()
    }

    pub fn current() -> Option<impl Deref<Target = Self> + Debug + 'static> {
        // TODO: per https://github.com/anp/moxie/issues/308 is using illicit
        // ok thread-local-wise vs eg Tokio can move tasks across threads?
        illicit::get::<Self>().ok()
    }

    pub fn emit_char(ch: char) {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_char(ch);
        }
    }

    pub fn emit_operation() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_operation();
        }
    }

    pub fn emit_fragment_definition() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_fragment_definition();
        }
    }

    pub fn emit_selection_set() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_set();
        }
    }

    pub fn emit_end_selection_set() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_end_selection_set();
        }
    }

    pub fn emit_selection_field() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_field();
        }
    }

    pub fn emit_selection_inline_fragment() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_inline_fragment();
        }
    }

    pub fn emit_selection_fragment_spread() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_fragment_spread();
        }
    }

    pub fn emit_token_pre_start() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_token_pre_start();
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Location {
    /// 1-based
    pub line: usize,
    /// 1-based
    pub column: usize,
}

impl Location {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Default)]
struct Document {
    pub definitions: Vec<OperationOrFragment>,
}

impl FindCurrentlyActiveSelectionSet for Document {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        for definition in &mut self.definitions {
            match definition.find_currently_active_selection_set() {
                Some(selection_set) => return Some(selection_set),
                _ => {}
            }
        }
        None
    }
}

trait FindCurrentlyActiveSelectionSet {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet>;
}

#[derive(Debug)]
enum OperationOrFragment {
    Operation(Operation),
    Fragment(FragmentDefinition),
}

impl OperationOrFragment {
    pub fn maybe_as_operation(&self) -> Option<&Operation> {
        match self {
            Self::Operation(operation) => Some(operation),
            _ => None,
        }
    }

    pub fn as_fragment_definition(&self) -> &FragmentDefinition {
        match self {
            Self::Fragment(fragment_definition) => fragment_definition,
            _ => panic!("Expected fragment definition"),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for OperationOrFragment {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        match self {
            Self::Operation(operation) => operation.find_currently_active_selection_set(),
            Self::Fragment(fragment) => fragment.find_currently_active_selection_set(),
        }
    }
}

#[derive(Debug)]
struct Operation {
    pub location: Location,
    pub selection_set: SelectionSet,
}

impl Operation {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for Operation {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        self.selection_set.find_currently_active_selection_set()
    }
}

#[derive(Debug)]
struct FragmentDefinition {
    pub location: Location,
    pub selection_set: SelectionSet,
}

impl FragmentDefinition {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for FragmentDefinition {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        self.selection_set.find_currently_active_selection_set()
    }
}

#[derive(Debug, Default)]
struct SelectionSet {
    selections: Vec<Selection>,
    status: SelectionSetStatus,
}

impl SelectionSet {
    pub fn open(&mut self) {
        assert!(self.status == SelectionSetStatus::NotYetStarted);
        self.status = SelectionSetStatus::Started;
    }

    pub fn close(&mut self) {
        assert!(self.status == SelectionSetStatus::Started);
        self.status = SelectionSetStatus::Done;
    }
}

impl FindCurrentlyActiveSelectionSet for SelectionSet {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        match self.status {
            SelectionSetStatus::Started => {
                // per https://users.rust-lang.org/t/returning-mutable-referernces-to-optional-self-fields/98206
                let mut found_sub = false;
                for selection in self.selections.iter_mut() {
                    if let Some(_) = selection.find_currently_active_selection_set() {
                        // return Some(selection_set);
                        found_sub = true;
                        break;
                    }
                }
                if !found_sub {
                    return Some(self);
                }
                for selection in self.selections.iter_mut() {
                    if let Some(selection_set) = selection.find_currently_active_selection_set() {
                        return Some(selection_set);
                    }
                }
                unreachable!()
            }
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
enum SelectionSetStatus {
    #[default]
    NotYetStarted,
    Started,
    Done,
}

#[derive(Debug)]
enum Selection {
    Field(Field),
    FragmentSpread,
    InlineFragment(InlineFragment),
}

impl Selection {
    pub fn open(&mut self) {
        match self {
            Selection::Field(field) => {
                field.selection_set = Some(_d());
                field.selection_set.as_mut().unwrap().open();
            }
            Selection::InlineFragment(inline_fragment) => inline_fragment.selection_set.open(),
            _ => unreachable!(),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for Selection {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        match self {
            Self::Field(field) => field.find_currently_active_selection_set(),
            Self::InlineFragment(inline_fragment) => {
                inline_fragment.find_currently_active_selection_set()
            }
            Self::FragmentSpread => None,
        }
    }
}

#[derive(Debug)]
struct Field {
    pub location: Location,
    pub selection_set: Option<SelectionSet>,
}

impl Field {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for Field {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        self.selection_set
            .as_mut()
            .and_then(|selection_set| selection_set.find_currently_active_selection_set())
    }
}

#[derive(Debug)]
struct InlineFragment {
    pub location: Location,
    pub selection_set: SelectionSet,
}

impl InlineFragment {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
        }
    }
}

impl FindCurrentlyActiveSelectionSet for InlineFragment {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet> {
        self.selection_set.find_currently_active_selection_set()
    }
}

fn find_selection_to_open(selection_set: &mut [Selection]) -> &mut Selection {
    selection_set
        .last_mut()
        .unwrap()
        .tap(|selection| match selection {
            Selection::Field(field) => assert!(field.selection_set.is_none()),
            Selection::InlineFragment(inline_fragment) => {
                assert!(inline_fragment.selection_set.status == SelectionSetStatus::NotYetStarted)
            }
            _ => unreachable!(),
        })
}

fn maybe_inline_fragment_location_selection_set(
    inline_fragment: &crate::InlineFragment,
    selection_set: &[crate::Selection],
    selection_set_positions: &SelectionSet,
) -> Option<Location> {
    selection_set
        .into_iter()
        .zip(selection_set_positions.selections.iter())
        .find_map(
            |(selection, selection_positions)| match (selection, selection_positions) {
                (crate::Selection::Field(field), Selection::Field(field_positions)) => {
                    field.selection_set.as_ref().and_then(|selection_set| {
                        maybe_inline_fragment_location_selection_set(
                            inline_fragment,
                            selection_set,
                            field_positions.selection_set.as_ref().unwrap(),
                        )
                    })
                }
                (
                    crate::Selection::InlineFragment(inline_fragment_current),
                    Selection::InlineFragment(inline_fragment_positions),
                ) => {
                    if ptr::eq(inline_fragment, inline_fragment_current) {
                        return Some(inline_fragment_positions.location);
                    }
                    maybe_inline_fragment_location_selection_set(
                        inline_fragment,
                        &inline_fragment.selection_set,
                        &inline_fragment_positions.selection_set,
                    )
                }
                (crate::Selection::FragmentSpread(_), Selection::FragmentSpread) => None,
                _ => unreachable!(),
            },
        )
}
