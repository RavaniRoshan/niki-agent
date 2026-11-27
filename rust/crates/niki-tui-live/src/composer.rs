/// The multi-line prompt at the bottom of the screen.
///
/// The cursor is a character index, not a byte offset, so a prompt with
/// non-ASCII text edits and renders the way the person typed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
    text: String,
    cursor: usize,
}

impl Composer {
    /// The text as typed.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether there is anything to send.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// How many lines the text occupies.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.text.split('\n').count()
    }

    /// The cursor as a zero-based line and a zero-based column.
    #[must_use]
    pub fn row_col(&self) -> (usize, usize) {
        let mut row = 0;
        let mut column = 0;
        for (index, character) in self.text.chars().enumerate() {
            if index == self.cursor {
                break;
            }
            if character == '\n' {
                row += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        (row, column)
    }

    /// The lines to draw, and the first line of that slice, keeping the cursor
    /// line inside a box of at most `max_rows` rows.
    #[must_use]
    pub fn window(&self, max_rows: usize) -> (Vec<&str>, usize) {
        let lines: Vec<&str> = self.text.split('\n').collect();
        let max_rows = max_rows.max(1);
        if lines.len() <= max_rows {
            return (lines, 0);
        }
        let (row, _) = self.row_col();
        let first = row.saturating_add(1).saturating_sub(max_rows);
        (
            lines[first..(first + max_rows).min(lines.len())].to_vec(),
            first,
        )
    }

    /// Insert one character at the cursor.
    pub fn insert_char(&mut self, character: char) {
        let at = self.byte_index();
        self.text.insert(at, character);
        self.cursor += 1;
    }

    /// Insert text, newlines included, at the cursor.
    pub fn insert_str(&mut self, text: &str) {
        let at = self.byte_index();
        self.text.insert_str(at, text);
        self.cursor += text.chars().count();
    }

    /// Break the line at the cursor.
    pub fn newline(&mut self) {
        self.insert_char('\n');
    }

    /// Remove the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = self.byte_index();
        self.cursor -= 1;
        let start = self.byte_index();
        self.text.replace_range(start..end, "");
    }

    /// Remove the character under the cursor.
    pub fn delete(&mut self) {
        if self.cursor >= self.text.chars().count() {
            return;
        }
        let start = self.byte_index();
        self.cursor += 1;
        let end = self.byte_index();
        self.cursor -= 1;
        self.text.replace_range(start..end, "");
    }

    /// Move one character left.
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move one character right.
    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    /// Move to the same column of the previous line, or to the start of the
    /// text from the first line.
    pub fn up(&mut self) {
        let (row, column) = self.row_col();
        if row == 0 {
            self.cursor = 0;
            return;
        }
        let start = self.row_start(row - 1);
        let width = self.row_width(row - 1);
        self.cursor = start + column.min(width);
    }

    /// Move to the same column of the next line, or to the end of the text from
    /// the last line.
    pub fn down(&mut self) {
        let (row, column) = self.row_col();
        if row + 1 >= self.line_count() {
            self.cursor = self.text.chars().count();
            return;
        }
        let start = self.row_start(row + 1);
        let width = self.row_width(row + 1);
        self.cursor = start + column.min(width);
    }

    /// Move to the start of the current line.
    pub fn home(&mut self) {
        let (row, _) = self.row_col();
        self.cursor = self.row_start(row);
    }

    /// Move to the end of the current line.
    pub fn end(&mut self) {
        let (row, _) = self.row_col();
        self.cursor = self.row_start(row) + self.row_width(row);
    }

    /// Empty the composer and return what it held.
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    /// Empty the composer.
    pub fn clear(&mut self) {
        self.cursor = 0;
        self.text.clear();
    }

    fn byte_index(&self) -> usize {
        self.text
            .char_indices()
            .nth(self.cursor)
            .map_or(self.text.len(), |(index, _)| index)
    }

    fn row_start(&self, row: usize) -> usize {
        let mut current = 0;
        for (index, character) in self.text.chars().enumerate() {
            if current == row {
                return index;
            }
            if character == '\n' {
                current += 1;
            }
        }
        self.text.chars().count()
    }

    fn row_width(&self, row: usize) -> usize {
        let mut current = 0;
        let mut width = 0;
        for character in self.text.chars() {
            if current == row {
                if character == '\n' {
                    return width;
                }
                width += 1;
            } else if character == '\n' {
                current += 1;
            }
        }
        width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_appends_and_the_cursor_follows() {
        let mut composer = Composer::default();
        for character in "hello".chars() {
            composer.insert_char(character);
        }
        assert_eq!(composer.text(), "hello");
        assert_eq!(composer.row_col(), (0, 5));
        composer.left();
        composer.left();
        composer.insert_char('!');
        assert_eq!(composer.text(), "hel!lo");
        assert_eq!(composer.row_col(), (0, 4));
        composer.delete();
        assert_eq!(composer.text(), "hel!o");
        composer.right();
        assert_eq!(composer.row_col(), (0, 5), "the end of the text");
    }

    #[test]
    fn backspace_removes_the_character_before_the_cursor() {
        let mut composer = Composer::default();
        composer.insert_str("abc");
        composer.backspace();
        assert_eq!(composer.text(), "ab");
        composer.home();
        composer.backspace();
        assert_eq!(composer.text(), "ab");
        composer.delete();
        assert_eq!(composer.text(), "b");
    }

    #[test]
    fn a_newline_starts_a_second_row() {
        let mut composer = Composer::default();
        composer.insert_str("first");
        composer.newline();
        composer.insert_str("second");
        assert_eq!(composer.line_count(), 2);
        assert_eq!(composer.row_col(), (1, 6));
        composer.up();
        assert_eq!(composer.row_col(), (0, 5));
        composer.end();
        assert_eq!(composer.row_col(), (0, 5));
        composer.home();
        assert_eq!(composer.row_col(), (0, 0));
        composer.down();
        assert_eq!(composer.row_col(), (1, 0));
        composer.end();
        composer.down();
        assert_eq!(composer.row_col(), (1, 6), "the last row is the end");
        composer.up();
        composer.up();
        assert_eq!(composer.row_col(), (0, 0), "the first row is the start");
    }

    #[test]
    fn moving_between_rows_keeps_the_column_when_it_fits() {
        let mut composer = Composer::default();
        composer.insert_str("abcdef\nxy\nlonger line");
        assert_eq!(composer.row_col(), (2, 11));
        composer.up();
        assert_eq!(composer.row_col(), (1, 2), "the row is only two wide");
        composer.up();
        assert_eq!(composer.row_col(), (0, 2));
        composer.left();
        composer.left();
        assert_eq!(composer.row_col(), (0, 0));
    }

    #[test]
    fn the_window_follows_the_cursor_inside_a_small_box() {
        let mut composer = Composer::default();
        composer.insert_str("one\ntwo\nthree");
        let (lines, first) = composer.window(5);
        assert_eq!(lines, vec!["one", "two", "three"]);
        assert_eq!(first, 0);

        let (lines, first) = composer.window(1);
        assert_eq!(lines, vec!["three"]);
        assert_eq!(first, 2);

        composer.up();
        let (lines, first) = composer.window(1);
        assert_eq!(lines, vec!["two"]);
        assert_eq!(first, 1);
    }

    #[test]
    fn non_ascii_text_edits_at_the_cursor() {
        let mut composer = Composer::default();
        composer.insert_str("héllo");
        assert_eq!(composer.row_col(), (0, 5));
        composer.left();
        composer.left();
        composer.insert_char('ô');
        assert_eq!(composer.text(), "hélôlo");
        composer.backspace();
        assert_eq!(composer.text(), "héllo");
        composer.clear();
        assert!(composer.is_empty());
        assert_eq!(composer.row_col(), (0, 0));
    }

    #[test]
    fn take_returns_the_text_and_resets() {
        let mut composer = Composer::default();
        composer.insert_str("send me");
        assert_eq!(composer.take(), "send me");
        assert!(composer.is_empty());
        assert_eq!(composer.take(), "");
    }
}
