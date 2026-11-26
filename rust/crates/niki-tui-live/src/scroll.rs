/// How far the transcript viewport is scrolled back from its newest line.
///
/// The offset counts lines above the bottom, so `0` pins the view to the newest
/// output and a turn that streams in keeps being visible without the reader
/// moving anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scroll {
    offset: usize,
}

impl Scroll {
    /// Lines above the newest one that the top of the viewport shows.
    #[must_use]
    pub fn offset(self) -> usize {
        self.offset
    }

    /// Pin the viewport to the newest line.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Scroll back by `lines`.
    pub fn up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_add(lines);
    }

    /// Scroll forward by `lines`, never past the newest line.
    pub fn down(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    /// Scroll to the oldest line.
    pub fn top(&mut self, total: usize, height: usize) {
        self.offset = max_offset(total, height);
    }

    /// Scroll to the newest line.
    pub fn end(&mut self) {
        self.reset();
    }

    /// Keep the offset inside the transcript after it grew or shrank.
    pub fn clamp(&mut self, total: usize, height: usize) {
        self.offset = self.offset.min(max_offset(total, height));
    }

    /// Index of the first visible line.
    #[must_use]
    pub fn start(self, total: usize, height: usize) -> usize {
        max_offset(total, height).saturating_sub(self.offset)
    }
}

fn max_offset(total: usize, height: usize) -> usize {
    total.saturating_sub(height.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_view_is_pinned_to_the_bottom() {
        let scroll = Scroll::default();
        assert_eq!(scroll.offset(), 0);
        assert_eq!(scroll.start(100, 10), 90);
        assert_eq!(scroll.start(5, 10), 0);
    }

    #[test]
    fn scrolling_back_walks_towards_the_oldest_line() {
        let mut scroll = Scroll::default();
        scroll.up(3);
        assert_eq!(scroll.start(100, 10), 87);
        scroll.down(1);
        assert_eq!(scroll.start(100, 10), 88);
        scroll.up(usize::MAX);
        scroll.clamp(100, 10);
        assert_eq!(scroll.start(100, 10), 0);
        scroll.top(100, 10);
        assert_eq!(scroll.offset(), 90);
        scroll.end();
        assert_eq!(scroll.offset(), 0);
        scroll.down(5);
        assert_eq!(scroll.offset(), 0, "already at the newest line");
    }

    #[test]
    fn a_shrinking_transcript_pulls_the_view_back() {
        let mut scroll = Scroll::default();
        scroll.up(90);
        assert_eq!(scroll.start(100, 10), 0);
        scroll.clamp(20, 10);
        assert_eq!(scroll.offset(), 10);
        scroll.clamp(4, 10);
        assert_eq!(scroll.offset(), 0);
    }
}
