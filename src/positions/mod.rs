use std::{cell::RefCell, fmt::Debug, ops::Deref};

pub struct CharsEmitter<TIterator: Iterator<Item = char>> {
    inner: TIterator,
}

impl<TIterator: Iterator<Item = char>> CharsEmitter<TIterator> {
    pub fn new(inner: TIterator) -> Self {
        Self { inner }
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
    operation_name_positions: RefCell<Vec<Location>>,
}

impl PositionsTracker {
    pub fn receive_char(&self, ch: char) {
        let just_newlined = { *self.just_newlined.borrow() };
        match ch {
            '\r' => {
                *self.just_newlined.borrow_mut() = true;
                *self.just_saw_carriage_return.borrow_mut() = true;
            }
            '\n' => {
                if !*self.just_saw_carriage_return.borrow() {
                    *self.just_newlined.borrow_mut() = true;
                }
            }
            _ => {}
        }
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
        *self.last_char.borrow_mut() = Some(new_last_char);
    }

    pub fn receive_operation_name(&self) {
        self.operation_name_positions
            .borrow_mut()
            .push(self.last_token_start.borrow().clone().unwrap());
    }

    pub fn receive_token_pre_start(&self) {
        *self.should_next_char_record_as_token_start.borrow_mut() = true;
    }

    pub fn nth_named_operation_name_location(&self, index: usize) -> Location {
        self.operation_name_positions.borrow()[index]
    }

    pub fn current() -> Option<impl Deref<Target = Self> + Debug + 'static> {
        illicit::get::<Self>().ok()
    }

    pub fn emit_char(ch: char) {
        if let Some(positions_tracker) = Self::current() {
            positions_tracker.receive_char(ch);
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
