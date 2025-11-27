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
            .inspect(|ch| match illicit::get::<PositionsTracker>() {
                Ok(positions_tracker) => {
                    positions_tracker.receive_char(*ch);
                }
                _ => {}
            })
    }
}

#[derive(Debug)]
pub struct PositionsTracker {}

impl PositionsTracker {
    pub fn receive_char(&self, ch: char) {
        unimplemented!()
    }
}
