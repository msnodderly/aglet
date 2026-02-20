use crossterm::event::KeyCode;
use tui_textarea::{CursorMove, TextArea};

/// A text buffer with a cursor position for single-line and multi-line editing.
///
/// `cursor` is a char offset into `text`. It may transiently exceed the text
/// length; all public accessors clamp it on read.
#[derive(Clone, Default)]
pub(crate) struct TextBuffer {
    text: String,
    cursor: usize,
}

impl TextBuffer {
    /// New buffer with the given text; cursor placed at end.
    pub(crate) fn new(text: String) -> Self {
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    /// Empty buffer with cursor at 0.
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// New buffer with the given text and an explicit cursor position.
    /// The cursor is clamped to the text length on construction.
    #[cfg(test)]
    pub(crate) fn with_cursor(text: String, cursor: usize) -> Self {
        let len = text.chars().count();
        Self {
            cursor: cursor.min(len),
            text,
        }
    }

    /// Replace the text and move the cursor to the end.
    pub(crate) fn set(&mut self, text: String) {
        self.cursor = text.chars().count();
        self.text = text;
    }

    /// Clear the text and reset cursor to 0.
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// The buffer's text contents.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Cursor position as a char offset, clamped to text length.
    pub(crate) fn cursor(&self) -> usize {
        self.cursor.min(self.len_chars())
    }

    /// Number of Unicode characters in the buffer.
    pub(crate) fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    /// True if the buffer contains no characters.
    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Text with leading/trailing whitespace removed.
    pub(crate) fn trimmed(&self) -> &str {
        self.text.trim()
    }

    /// For multi-line buffers: (line_index, col_index) of the cursor.
    /// Used by render code to position the terminal cursor.
    pub(crate) fn line_col(&self) -> (usize, usize) {
        cursor_line_col(&self.text, self.cursor())
    }

    /// Dispatch a key event into the buffer.
    ///
    /// When `multiline` is true, `Enter` inserts a newline and `Up`/`Down`
    /// move between lines. When false, those keys are not consumed.
    ///
    /// Returns `true` if the key was consumed, `false` if it was ignored.
    pub(crate) fn handle_key(&mut self, code: KeyCode, multiline: bool) -> bool {
        if multiline {
            self.with_textarea(true, |textarea| match code {
                KeyCode::Left => {
                    textarea.move_cursor(CursorMove::Back);
                    true
                }
                KeyCode::Right => {
                    textarea.move_cursor(CursorMove::Forward);
                    true
                }
                KeyCode::Up => {
                    textarea.move_cursor(CursorMove::Up);
                    true
                }
                KeyCode::Down => {
                    textarea.move_cursor(CursorMove::Down);
                    true
                }
                KeyCode::Home => {
                    textarea.move_cursor(CursorMove::Head);
                    true
                }
                KeyCode::End => {
                    textarea.move_cursor(CursorMove::End);
                    true
                }
                KeyCode::Backspace => {
                    let _ = textarea.delete_char();
                    true
                }
                KeyCode::Delete => {
                    let _ = textarea.delete_next_char();
                    true
                }
                KeyCode::Enter => {
                    textarea.insert_newline();
                    true
                }
                KeyCode::Char(c) if !c.is_control() => {
                    textarea.insert_char(c);
                    true
                }
                _ => false,
            })
        } else {
            self.with_textarea(false, |textarea| match code {
                KeyCode::Left => {
                    textarea.move_cursor(CursorMove::Back);
                    true
                }
                KeyCode::Right => {
                    textarea.move_cursor(CursorMove::Forward);
                    true
                }
                KeyCode::Home => {
                    textarea.move_cursor(CursorMove::Head);
                    true
                }
                KeyCode::End => {
                    textarea.move_cursor(CursorMove::End);
                    true
                }
                KeyCode::Backspace => {
                    let _ = textarea.delete_char();
                    true
                }
                KeyCode::Delete => {
                    let _ = textarea.delete_next_char();
                    true
                }
                KeyCode::Char(c) if !c.is_control() => {
                    textarea.insert_char(c);
                    true
                }
                _ => false,
            })
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Create a TextArea pre-loaded with the buffer's text and cursor, apply
    /// `edit`, then write the result back. Returns whatever `edit` returns.
    fn with_textarea<F>(&mut self, multiline: bool, edit: F) -> bool
    where
        F: FnOnce(&mut TextArea<'static>) -> bool,
    {
        let cursor = self.cursor();
        let mut textarea = if multiline {
            build_multiline_textarea(&self.text, cursor)
        } else {
            build_single_line_textarea(&self.text, cursor)
        };

        let consumed = edit(&mut textarea);

        if consumed {
            let (new_text, new_cursor) = extract_value_and_cursor(textarea);
            self.text = new_text;
            self.cursor = new_cursor.min(self.text.chars().count());
        }

        consumed
    }
}

// ── Free helpers (used by TextBuffer internals) ───────────────────────────────

fn build_single_line_textarea(text: &str, cursor: usize) -> TextArea<'static> {
    let mut textarea = TextArea::new(vec![text.to_string()]);
    let col = cursor.min(text.chars().count()).min(u16::MAX as usize) as u16;
    textarea.move_cursor(CursorMove::Jump(0, col));
    textarea
}

fn build_multiline_textarea(text: &str, cursor: usize) -> TextArea<'static> {
    let mut textarea = TextArea::new(text.split('\n').map(str::to_string).collect());
    let (line, col) = cursor_line_col(text, cursor.min(text.chars().count()));
    let row = line.min(u16::MAX as usize) as u16;
    let col = col.min(u16::MAX as usize) as u16;
    textarea.move_cursor(CursorMove::Jump(row, col));
    textarea
}

fn extract_value_and_cursor(textarea: TextArea<'static>) -> (String, usize) {
    let (row, col) = textarea.cursor();
    let value = textarea.into_lines().join("\n");
    let cursor = char_index_from_line_col(&value, row, col);
    (value, cursor)
}

fn char_index_from_line_col(text: &str, row: usize, col: usize) -> usize {
    let line_starts = line_start_chars(text);
    if line_starts.is_empty() {
        return 0;
    }
    let line_index = row.min(line_starts.len().saturating_sub(1));
    let line_start = line_starts[line_index];
    let text_len = text.chars().count();
    let line_end = if line_index + 1 < line_starts.len() {
        line_starts[line_index + 1].saturating_sub(1)
    } else {
        text_len
    };
    line_start + col.min(line_end.saturating_sub(line_start))
}

/// Char offsets where each line starts (line 0 always starts at 0).
fn line_start_chars(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let mut idx = 0usize;
    for c in text.chars() {
        idx += 1;
        if c == '\n' {
            starts.push(idx);
        }
    }
    starts
}

/// Convert a char-offset cursor into (line, col) for multi-line display.
fn cursor_line_col(text: &str, cursor_chars: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for c in text.chars().take(cursor_chars) {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    // ── Construction ──────────────────────────────────────────────────────────

    #[test]
    fn empty_buffer_has_zero_cursor() {
        let buf = TextBuffer::empty();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn new_places_cursor_at_end() {
        let buf = TextBuffer::new("hello".to_string());
        assert_eq!(buf.cursor(), 5);
        assert!(!buf.is_empty());
    }

    #[test]
    fn set_replaces_text_and_moves_cursor_to_end() {
        let mut buf = TextBuffer::empty();
        buf.set("hi".to_string());
        assert_eq!(buf.text(), "hi");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn clear_empties_text_and_resets_cursor() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.clear();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
    }

    // ── Cursor clamping ───────────────────────────────────────────────────────

    #[test]
    fn cursor_is_clamped_after_text_shrinks() {
        let mut buf = TextBuffer::new("hello".to_string()); // cursor=5
        buf.text = "hi".to_string(); // shrink without moving cursor
        assert_eq!(buf.cursor(), 2); // clamped
    }

    // ── Single-line ops ───────────────────────────────────────────────────────

    #[test]
    fn left_right_move_cursor_single_line() {
        let mut buf = TextBuffer::new("abc".to_string()); // cursor=3
        buf.handle_key(KeyCode::Left, false);
        assert_eq!(buf.cursor(), 2);
        buf.handle_key(KeyCode::Right, false);
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn home_end_move_cursor_to_boundaries() {
        let mut buf = TextBuffer::new("abc".to_string()); // cursor=3
        buf.handle_key(KeyCode::Home, false);
        assert_eq!(buf.cursor(), 0);
        buf.handle_key(KeyCode::End, false);
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn insert_char_advances_cursor() {
        let mut buf = TextBuffer::new("ac".to_string());
        buf.handle_key(KeyCode::Left, false); // cursor=1
        buf.handle_key(KeyCode::Char('b'), false);
        assert_eq!(buf.text(), "abc");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut buf = TextBuffer::empty();
        buf.handle_key(KeyCode::Backspace, false);
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.handle_key(KeyCode::Backspace, false);
        assert_eq!(buf.text(), "ab");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn delete_at_end_is_noop() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.handle_key(KeyCode::Delete, false);
        assert_eq!(buf.text(), "abc");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn delete_removes_char_at_cursor() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.handle_key(KeyCode::Home, false);
        buf.handle_key(KeyCode::Delete, false);
        assert_eq!(buf.text(), "bc");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn insert_control_char_is_ignored() {
        let mut buf = TextBuffer::empty();
        buf.handle_key(KeyCode::Char('\x01'), false); // Ctrl-A
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn unhandled_key_returns_false() {
        let mut buf = TextBuffer::empty();
        assert!(!buf.handle_key(KeyCode::F(1), false));
        assert!(!buf.handle_key(KeyCode::Esc, false));
    }

    #[test]
    fn enter_not_consumed_in_single_line_mode() {
        let mut buf = TextBuffer::empty();
        assert!(!buf.handle_key(KeyCode::Enter, false));
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn up_down_not_consumed_in_single_line_mode() {
        let mut buf = TextBuffer::new("abc".to_string());
        assert!(!buf.handle_key(KeyCode::Up, false));
        assert!(!buf.handle_key(KeyCode::Down, false));
    }

    // ── Multi-line ops ────────────────────────────────────────────────────────

    #[test]
    fn enter_inserts_newline_in_multiline_mode() {
        let mut buf = TextBuffer::new("ab".to_string());
        buf.handle_key(KeyCode::Home, true);
        buf.handle_key(KeyCode::Enter, true);
        // "ab" with cursor at 0, insert newline → "\nab"
        assert!(buf.text().contains('\n'));
    }

    #[test]
    fn up_down_move_between_lines() {
        let mut buf = TextBuffer::new("line1\nline2".to_string());
        // cursor is at end of line2; move up should go to line1
        buf.handle_key(KeyCode::Up, true);
        let (line, _) = buf.line_col();
        assert_eq!(line, 0);
        buf.handle_key(KeyCode::Down, true);
        let (line, _) = buf.line_col();
        assert_eq!(line, 1);
    }

    #[test]
    fn line_col_returns_correct_position() {
        let buf = TextBuffer::new("hello\nworld".to_string());
        // cursor at end = char index 11
        let (line, col) = buf.line_col();
        assert_eq!(line, 1);
        assert_eq!(col, 5); // "world" has 5 chars
    }

    #[test]
    fn line_col_for_single_line() {
        let buf = TextBuffer::new("abc".to_string());
        assert_eq!(buf.line_col(), (0, 3));
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    #[test]
    fn trimmed_strips_whitespace() {
        let buf = TextBuffer::new("  hello  ".to_string());
        assert_eq!(buf.trimmed(), "hello");
    }

    #[test]
    fn len_chars_counts_unicode() {
        let buf = TextBuffer::new("héllo".to_string()); // é is one char
        assert_eq!(buf.len_chars(), 5);
    }
}
