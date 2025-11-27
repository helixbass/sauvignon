use std::{fmt::Debug, ops::Deref};

use crate::Token;

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

#[derive(Debug)]
pub struct PositionsTracker {}

impl PositionsTracker {
    pub fn receive_char(&self, ch: char) {
        unimplemented!()
    }

    pub fn receive_operation_name(&self) {
        unimplemented!()
    }

    pub fn receive_token_pre_start(&self) {
        unimplemented!()
    }

    pub fn nth_named_operation_name_location(&self, index: usize) -> Location {
        unimplemented!()
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
