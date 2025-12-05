use std::{cell::RefCell, fmt::Debug, iter::Peekable, ops::Deref, ptr};

use serde::Serialize;
use squalid::{EverythingExt, _d};
use tracing::instrument;

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
    start_of_upcoming_selection_inline_fragment_or_fragment_spread: RefCell<Option<Location>>,
    is_in_directives: RefCell<bool>,
}

impl PositionsTracker {
    #[instrument(level = "trace")]
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

    pub fn last_char(&self) -> Location {
        self.last_char.borrow().clone().unwrap()
    }

    pub fn maybe_last_token(&self) -> Option<Location> {
        self.last_token_start.borrow().clone()
    }

    fn last_token(&self) -> Location {
        self.maybe_last_token().unwrap()
    }

    #[instrument(level = "trace")]
    pub fn receive_operation(&self) {
        self.document
            .borrow_mut()
            .definitions
            .push(OperationOrFragment::Operation(Operation::new(
                self.last_token(),
            )));
    }

    #[instrument(level = "trace")]
    pub fn receive_fragment_definition(&self) {
        self.document
            .borrow_mut()
            .definitions
            .push(OperationOrFragment::Fragment(FragmentDefinition::new(
                self.last_token(),
            )));
    }

    #[instrument(level = "trace")]
    pub fn receive_selection_set(&self) {
        let mut document = self.document.borrow_mut();
        match document.find_currently_active_selection_set() {
            None => match document.definitions.last_mut().unwrap() {
                OperationOrFragment::Operation(operation) => {
                    assert_eq!(
                        operation.selection_set.status,
                        SelectionSetStatus::NotYetStarted
                    );
                    operation.selection_set.open();
                }
                OperationOrFragment::Fragment(fragment) => {
                    assert_eq!(
                        fragment.selection_set.status,
                        SelectionSetStatus::NotYetStarted
                    );
                    fragment.selection_set.open();
                }
            },
            Some(currently_active_selection_set) => {
                find_selection_to_open(&mut currently_active_selection_set.selections).open()
            }
        }
    }

    #[instrument(level = "trace")]
    pub fn receive_end_selection_set(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .close();
    }

    #[instrument(level = "trace")]
    pub fn receive_selection_field(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::Field(Field::new(self.last_token())));
    }

    #[instrument(level = "trace")]
    pub fn receive_selection_inline_fragment_or_fragment_spread(&self) {
        *self
            .start_of_upcoming_selection_inline_fragment_or_fragment_spread
            .borrow_mut() = Some(self.last_token());
    }

    #[instrument(level = "trace")]
    pub fn receive_selection_inline_fragment(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::InlineFragment(InlineFragment::new(
                self.start_of_upcoming_selection_inline_fragment_or_fragment_spread
                    .borrow()
                    .clone()
                    .unwrap(),
            )));
        *self
            .start_of_upcoming_selection_inline_fragment_or_fragment_spread
            .borrow_mut() = None;
    }

    #[instrument(level = "trace")]
    pub fn receive_selection_fragment_spread(&self) {
        self.document
            .borrow_mut()
            .find_currently_active_selection_set()
            .unwrap()
            .selections
            .push(Selection::FragmentSpread(FragmentSpread::new(
                self.start_of_upcoming_selection_inline_fragment_or_fragment_spread
                    .borrow()
                    .clone()
                    .unwrap(),
            )));
        *self
            .start_of_upcoming_selection_inline_fragment_or_fragment_spread
            .borrow_mut() = None;
    }

    #[instrument(level = "trace")]
    pub fn receive_argument(&self) {
        if *self.is_in_directives.borrow() {
            self.document
                .borrow_mut()
                .find_currently_active_directive()
                .arguments
                .push(self.last_token());
        } else {
            self.document
                .borrow_mut()
                .find_currently_active_selection_set()
                .unwrap()
                .selections
                .last_mut()
                .unwrap()
                .as_field_mut()
                .arguments
                .push(self.last_token());
        }
    }

    #[instrument(level = "trace")]
    pub fn receive_token_pre_start(&self) {
        *self.should_next_char_record_as_token_start.borrow_mut() = true;
    }

    #[instrument(level = "trace")]
    pub fn receive_start_directives(&self) {
        *self.is_in_directives.borrow_mut() = true;
    }

    #[instrument(level = "trace")]
    pub fn receive_end_directives(&self) {
        *self.is_in_directives.borrow_mut() = false;
    }

    #[instrument(level = "trace")]
    pub fn receive_directive(&self) {
        let mut document = self.document.borrow_mut();
        match document.find_currently_active_selection_set() {
            None => match document.definitions.last_mut().unwrap() {
                OperationOrFragment::Operation(operation) => {
                    operation.directives.push(Directive::new(self.last_token()));
                }
                OperationOrFragment::Fragment(fragment) => {
                    assert_eq!(
                        fragment.selection_set.status,
                        SelectionSetStatus::NotYetStarted
                    );
                    fragment.directives.push(Directive::new(self.last_token()));
                }
            },
            Some(currently_active_selection_set) => {
                match currently_active_selection_set
                    .selections
                    .last_mut()
                    .unwrap()
                {
                    Selection::Field(field) => {
                        field.directives.push(Directive::new(self.last_token()))
                    }
                    Selection::FragmentSpread(fragment_spread) => fragment_spread
                        .directives
                        .push(Directive::new(self.last_token())),
                    Selection::InlineFragment(inline_fragment) => inline_fragment
                        .directives
                        .push(Directive::new(self.last_token())),
                }
            }
        }
    }

    #[instrument(level = "trace")]
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

    #[instrument(level = "trace")]
    pub fn nth_fragment_location(&self, index: usize) -> Location {
        self.document
            .borrow()
            .definitions
            .iter()
            .filter_map(|definition| definition.maybe_as_fragment())
            .nth(index)
            .unwrap()
            .location
    }

    #[instrument(level = "trace", skip(fragment, document))]
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
            .as_fragment()
            .location
    }

    #[instrument(level = "trace", skip(inline_fragment, document))]
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

    #[instrument(level = "trace", skip(field, document))]
    pub fn field_location(
        &self,
        field: &crate::request::Field,
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
                    ) => maybe_field_location_selection_set(
                        field,
                        &operation_definition.selection_set,
                        &operation_positions.selection_set,
                    ),
                    (
                        ExecutableDefinition::Fragment(fragment_definition),
                        OperationOrFragment::Fragment(fragment_positions),
                    ) => maybe_field_location_selection_set(
                        field,
                        &fragment_definition.selection_set,
                        &fragment_positions.selection_set,
                    ),
                    _ => unreachable!(),
                }
            })
            .unwrap()
    }

    #[instrument(level = "trace", skip(field, document))]
    pub fn field_nth_argument_location(
        &self,
        field: &crate::request::Field,
        index: usize,
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
                    ) => maybe_field_nth_argument_location_selection_set(
                        field,
                        index,
                        &operation_definition.selection_set,
                        &operation_positions.selection_set,
                    ),
                    (
                        ExecutableDefinition::Fragment(fragment_definition),
                        OperationOrFragment::Fragment(fragment_positions),
                    ) => maybe_field_nth_argument_location_selection_set(
                        field,
                        index,
                        &fragment_definition.selection_set,
                        &fragment_positions.selection_set,
                    ),
                    _ => unreachable!(),
                }
            })
            .unwrap()
    }

    #[instrument(level = "trace", skip(fragment_spread, document))]
    pub fn fragment_spread_location(
        &self,
        fragment_spread: &crate::request::FragmentSpread,
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
                    ) => maybe_fragment_spread_location_selection_set(
                        fragment_spread,
                        &operation_definition.selection_set,
                        &operation_positions.selection_set,
                    ),
                    (
                        ExecutableDefinition::Fragment(fragment_definition),
                        OperationOrFragment::Fragment(fragment_positions),
                    ) => maybe_fragment_spread_location_selection_set(
                        fragment_spread,
                        &fragment_definition.selection_set,
                        &fragment_positions.selection_set,
                    ),
                    _ => unreachable!(),
                }
            })
            .unwrap()
    }

    #[instrument(level = "trace", skip(directive, document))]
    pub fn directive_location(
        &self,
        directive: &crate::request::Directive,
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
                    ) => {
                        if let Some(index) = operation_definition
                            .directives
                            .iter()
                            .position(|directive_current| ptr::eq(directive_current, directive))
                        {
                            return Some(operation_positions.directives[index].location);
                        }
                        maybe_directive_location_selection_set(
                            directive,
                            &operation_definition.selection_set,
                            &operation_positions.selection_set,
                        )
                    }
                    (
                        ExecutableDefinition::Fragment(fragment_definition),
                        OperationOrFragment::Fragment(fragment_positions),
                    ) => {
                        if let Some(index) = fragment_definition
                            .directives
                            .iter()
                            .position(|directive_current| ptr::eq(directive_current, directive))
                        {
                            return Some(fragment_positions.directives[index].location);
                        }
                        maybe_directive_location_selection_set(
                            directive,
                            &fragment_definition.selection_set,
                            &fragment_positions.selection_set,
                        )
                    }
                    _ => unreachable!(),
                }
            })
            .unwrap()
    }

    #[instrument(level = "trace")]
    pub fn current() -> Option<impl Deref<Target = Self> + Debug + 'static> {
        // TODO: per https://github.com/anp/moxie/issues/308 is using illicit
        // ok thread-local-wise vs eg Tokio can move tasks across threads?
        illicit::get::<Self>().ok()
    }

    #[instrument(level = "trace")]
    pub fn emit_char(ch: char) {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_char(ch);
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_operation() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_operation();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_fragment_definition() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_fragment_definition();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_selection_set() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_set();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_end_selection_set() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_end_selection_set();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_selection_field() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_field();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_selection_inline_fragment_or_fragment_spread() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_inline_fragment_or_fragment_spread();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_selection_inline_fragment() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_inline_fragment();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_selection_fragment_spread() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_selection_fragment_spread();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_argument() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_argument();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_token_pre_start() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_token_pre_start();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_start_directives() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_start_directives();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_end_directives() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_end_directives();
        }
    }

    #[instrument(level = "trace")]
    pub fn emit_directive() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_directive();
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

trait FindCurrentlyActiveDirective: FindCurrentlyActiveSelectionSet {
    fn find_currently_active_directive(&mut self) -> &mut Directive;
}

impl FindCurrentlyActiveDirective for Document {
    fn find_currently_active_directive(&mut self) -> &mut Directive {
        if self.find_currently_active_selection_set().is_some() {
            return match self
                .find_currently_active_selection_set()
                .unwrap()
                .selections
                .last_mut()
                .unwrap()
            {
                Selection::Field(field) => field.directives.last_mut().unwrap(),
                Selection::FragmentSpread(fragment_spread) => {
                    fragment_spread.directives.last_mut().unwrap()
                }
                Selection::InlineFragment(inline_fragment) => {
                    inline_fragment.directives.last_mut().unwrap()
                }
            };
        }
        match self.definitions.last_mut().unwrap() {
            OperationOrFragment::Operation(operation) => operation.directives.last_mut().unwrap(),
            OperationOrFragment::Fragment(fragment_definition) => {
                fragment_definition.directives.last_mut().unwrap()
            }
        }
    }
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

    pub fn maybe_as_fragment(&self) -> Option<&FragmentDefinition> {
        match self {
            Self::Fragment(fragment_definition) => Some(fragment_definition),
            _ => None,
        }
    }

    pub fn as_fragment(&self) -> &FragmentDefinition {
        self.maybe_as_fragment()
            .expect("Expected fragment definition")
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
    pub directives: Vec<Directive>,
}

impl Operation {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
            directives: _d(),
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
    pub directives: Vec<Directive>,
}

impl FragmentDefinition {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
            directives: _d(),
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
    FragmentSpread(FragmentSpread),
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

    pub fn as_field_mut(&mut self) -> &mut Field {
        match self {
            Selection::Field(field) => field,
            _ => panic!("Expected field"),
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
            Self::FragmentSpread(_) => None,
        }
    }
}

#[derive(Debug)]
struct Field {
    pub location: Location,
    pub arguments: Vec<Location>,
    pub selection_set: Option<SelectionSet>,
    pub directives: Vec<Directive>,
}

impl Field {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            arguments: _d(),
            selection_set: _d(),
            directives: _d(),
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
    pub directives: Vec<Directive>,
}

impl InlineFragment {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
            directives: _d(),
        }
    }
}

#[derive(Debug)]
struct FragmentSpread {
    pub location: Location,
    pub directives: Vec<Directive>,
}

impl FragmentSpread {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            directives: _d(),
        }
    }
}

#[derive(Debug)]
struct Directive {
    pub location: Location,
    pub arguments: Vec<Location>,
}

impl Directive {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            arguments: _d(),
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
                        &inline_fragment_current.selection_set,
                        &inline_fragment_positions.selection_set,
                    )
                }
                (crate::Selection::FragmentSpread(_), Selection::FragmentSpread(_)) => None,
                _ => unreachable!(),
            },
        )
}

fn maybe_field_location_selection_set(
    field: &crate::request::Field,
    selection_set: &[crate::Selection],
    selection_set_positions: &SelectionSet,
) -> Option<Location> {
    selection_set
        .into_iter()
        .zip(selection_set_positions.selections.iter())
        .find_map(
            |(selection, selection_positions)| match (selection, selection_positions) {
                (crate::Selection::Field(field_current), Selection::Field(field_positions)) => {
                    if ptr::eq(field, field_current) {
                        return Some(field_positions.location);
                    }
                    field_current
                        .selection_set
                        .as_ref()
                        .and_then(|selection_set| {
                            maybe_field_location_selection_set(
                                field,
                                selection_set,
                                field_positions.selection_set.as_ref().unwrap(),
                            )
                        })
                }
                (
                    crate::Selection::InlineFragment(inline_fragment_current),
                    Selection::InlineFragment(inline_fragment_positions),
                ) => maybe_field_location_selection_set(
                    field,
                    &inline_fragment_current.selection_set,
                    &inline_fragment_positions.selection_set,
                ),
                (crate::Selection::FragmentSpread(_), Selection::FragmentSpread(_)) => None,
                _ => unreachable!(),
            },
        )
}

fn maybe_field_nth_argument_location_selection_set(
    field: &crate::request::Field,
    index: usize,
    selection_set: &[crate::Selection],
    selection_set_positions: &SelectionSet,
) -> Option<Location> {
    selection_set
        .into_iter()
        .zip(selection_set_positions.selections.iter())
        .find_map(
            |(selection, selection_positions)| match (selection, selection_positions) {
                (crate::Selection::Field(field_current), Selection::Field(field_positions)) => {
                    if ptr::eq(field, field_current) {
                        return Some(field_positions.arguments[index]);
                    }
                    field_current
                        .selection_set
                        .as_ref()
                        .and_then(|selection_set| {
                            maybe_field_location_selection_set(
                                field,
                                selection_set,
                                field_positions.selection_set.as_ref().unwrap(),
                            )
                        })
                }
                (
                    crate::Selection::InlineFragment(inline_fragment_current),
                    Selection::InlineFragment(inline_fragment_positions),
                ) => maybe_field_location_selection_set(
                    field,
                    &inline_fragment_current.selection_set,
                    &inline_fragment_positions.selection_set,
                ),
                (crate::Selection::FragmentSpread(_), Selection::FragmentSpread(_)) => None,
                _ => unreachable!(),
            },
        )
}

fn maybe_fragment_spread_location_selection_set(
    fragment_spread: &crate::request::FragmentSpread,
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
                        maybe_fragment_spread_location_selection_set(
                            fragment_spread,
                            selection_set,
                            field_positions.selection_set.as_ref().unwrap(),
                        )
                    })
                }
                (
                    crate::Selection::InlineFragment(inline_fragment_current),
                    Selection::InlineFragment(inline_fragment_positions),
                ) => maybe_fragment_spread_location_selection_set(
                    fragment_spread,
                    &inline_fragment_current.selection_set,
                    &inline_fragment_positions.selection_set,
                ),
                (
                    crate::Selection::FragmentSpread(fragment_spread_current),
                    Selection::FragmentSpread(fragment_spread_positions),
                ) => ptr::eq(fragment_spread, fragment_spread_current)
                    .then_some(fragment_spread_positions.location),
                _ => unreachable!(),
            },
        )
}

fn maybe_directive_location_selection_set(
    directive: &crate::request::Directive,
    selection_set: &[crate::Selection],
    selection_set_positions: &SelectionSet,
) -> Option<Location> {
    selection_set
        .into_iter()
        .zip(selection_set_positions.selections.iter())
        .find_map(
            |(selection, selection_positions)| match (selection, selection_positions) {
                (crate::Selection::Field(field_current), Selection::Field(field_positions)) => {
                    if let Some(index) = field_current
                        .directives
                        .iter()
                        .position(|directive_current| ptr::eq(directive_current, directive))
                    {
                        return Some(field_positions.directives[index].location);
                    }
                    field_current
                        .selection_set
                        .as_ref()
                        .and_then(|selection_set| {
                            maybe_directive_location_selection_set(
                                directive,
                                selection_set,
                                field_positions.selection_set.as_ref().unwrap(),
                            )
                        })
                }
                (
                    crate::Selection::InlineFragment(inline_fragment_current),
                    Selection::InlineFragment(inline_fragment_positions),
                ) => {
                    if let Some(index) = inline_fragment_current
                        .directives
                        .iter()
                        .position(|directive_current| ptr::eq(directive_current, directive))
                    {
                        return Some(inline_fragment_positions.directives[index].location);
                    }
                    maybe_directive_location_selection_set(
                        directive,
                        &inline_fragment_current.selection_set,
                        &inline_fragment_positions.selection_set,
                    )
                }
                (
                    crate::Selection::FragmentSpread(fragment_spread_current),
                    Selection::FragmentSpread(fragment_spread_positions),
                ) => {
                    if let Some(index) = fragment_spread_current
                        .directives
                        .iter()
                        .position(|directive_current| ptr::eq(directive_current, directive))
                    {
                        return Some(fragment_spread_positions.directives[index].location);
                    }
                    None
                }
                _ => unreachable!(),
            },
        )
}
