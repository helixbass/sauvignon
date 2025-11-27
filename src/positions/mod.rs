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

    pub fn nth_named_operation_name_location(&self, index: usize) -> Location {
        unimplemented!()
    }

    pub fn emit_char(ch: char) {
        match illicit::get::<PositionsTracker>() {
            Ok(positions_tracker) => {
                positions_tracker.receive_char(ch);
            }
            _ => {}
        }
    }

    pub fn emit_operation_name() {
        match illicit::get::<PositionsTracker>() {
            Ok(positions_tracker) => {
                positions_tracker.receive_operation_name();
            }
            _ => {}
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
