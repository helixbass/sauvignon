use std::{cell::RefCell, fmt::Debug, iter::Peekable, ops::Deref};

use serde::Serialize;
use squalid::_d;

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

    pub fn receive_selection_set(&self) {
        let mut document = self.document.borrow_mut();
        let currently_active_selection_set = document.find_currently_active_selection_set();
        match currently_active_selection_set {
            None => match document.definitions.first_mut().unwrap() {
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
        unimplemented!()
    }
}

trait FindCurrentlyActiveSelectionSet {
    fn find_currently_active_selection_set(&mut self) -> Option<&mut SelectionSet>;
}

#[derive(Debug)]
enum OperationOrFragment {
    Operation(Operation),
    Fragment(Fragment),
}

impl OperationOrFragment {
    pub fn maybe_as_operation(&self) -> Option<&Operation> {
        match self {
            Self::Operation(operation) => Some(operation),
            _ => None,
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

#[derive(Debug, Default)]
struct Fragment {
    pub selection_set: SelectionSet,
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
            Selection::Field(field) => field.selection_set.open(),
            Selection::InlineFragment(inline_fragment) => inline_fragment.selection_set.open(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
struct Field {
    pub location: Location,
    pub selection_set: SelectionSet,
}

impl Field {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            selection_set: _d(),
        }
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

fn find_selection_to_open(selection_set: &mut [Selection]) -> &mut Selection {
    selection_set
        .iter_mut()
        .filter(|selection| match selection {
            Selection::Field(field) => {
                field.selection_set.status == SelectionSetStatus::NotYetStarted
            }
            Selection::InlineFragment(inline_fragment) => {
                inline_fragment.selection_set.status == SelectionSetStatus::NotYetStarted
            }
            _ => false,
        })
        .next()
        .unwrap()
}
