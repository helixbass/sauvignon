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
    operations: RefCell<Vec<Operation>>,
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

    pub fn receive_operation(&self) {
        self.operations.borrow_mut().push(Operation::new(
            self.last_token_start.borrow().clone().unwrap(),
        ));
    }

    pub fn receive_operation_name(&self) {
        self.operations
            .borrow_mut()
            .last_mut()
            .unwrap()
            .name_location = Some(self.last_token_start.borrow().clone().unwrap());
    }

    pub fn receive_token_pre_start(&self) {
        *self.should_next_char_record_as_token_start.borrow_mut() = true;
    }

    pub fn nth_named_operation_name_location(&self, index: usize) -> Location {
        self.operations
            .borrow()
            .iter()
            .filter_map(|operation| operation.name_location)
            .nth(index)
            .unwrap()
    }

    pub fn anonymous_operation_location(&self) -> Location {
        self.operations
            .borrow()
            .iter()
            .find_map(|operation| match operation.name_location.as_ref() {
                None => Some(operation.location),
                Some(_) => None,
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

    pub fn emit_operation_name() {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_operation_name();
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

#[derive(Debug)]
struct Operation {
    pub location: Location,
    pub name_location: Option<Location>,
}

impl Operation {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            name_location: _d(),
        }
    }
}
