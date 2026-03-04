use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, TextArea, WrapMode};

/// Persistent text-edit buffer backed by `tui-textarea-2`.
#[derive(Clone, Debug)]
pub(crate) struct TextBuffer {
    text: String,
    textarea: TextArea<'static>,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl TextBuffer {
    /// New buffer with the given text; cursor placed at end.
    pub(crate) fn new(text: String) -> Self {
        Self::build(text, None)
    }

    /// Empty buffer with cursor at 0.
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// New buffer with the given text and an explicit cursor position.
    /// The cursor is clamped to the text length on construction.
    #[cfg(test)]
    pub(crate) fn with_cursor(text: String, cursor: usize) -> Self {
        Self::build(text, Some(cursor))
    }

    /// Replace the text and move the cursor to the end.
    pub(crate) fn set(&mut self, text: String) {
        *self = Self::new(text);
    }

    /// Clear the text and reset cursor to 0.
    pub(crate) fn clear(&mut self) {
        self.set(String::new());
    }

    /// The buffer's text contents.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Cursor position as a char offset in the full text.
    pub(crate) fn cursor(&self) -> usize {
        let (row, col) = self.textarea.cursor();
        char_index_from_line_col(&self.text, row, col)
    }

    /// Number of Unicode characters in the buffer.
    #[cfg(test)]
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
    pub(crate) fn line_col(&self) -> (usize, usize) {
        self.textarea.cursor()
    }

    /// Access the backing textarea widget for rendering.
    pub(crate) fn widget(&self) -> &TextArea<'static> {
        &self.textarea
    }

    /// Dispatch a full key event into the buffer.
    ///
    /// When `multiline` is true, Enter and vertical motion are enabled.
    /// When false, those keys are not consumed.
    pub(crate) fn handle_key_event(&mut self, key: KeyEvent, multiline: bool) -> bool {
        if !multiline && blocks_single_line(key) {
            return false;
        }

        let before_cursor = self.textarea.cursor();
        let modified = self.textarea.input(key);
        if modified {
            self.sync_text_from_textarea();
        }
        modified
            || self.textarea.cursor() != before_cursor
            || non_mutating_text_key_consumed(key, multiline)
    }

    /// Compatibility shim for callsites/tests that only pass `KeyCode`.
    #[cfg(test)]
    pub(crate) fn handle_key(&mut self, code: KeyCode, multiline: bool) -> bool {
        self.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE), multiline)
    }

    fn build(text: String, cursor: Option<usize>) -> Self {
        let mut textarea = TextArea::new(split_lines_preserve_trailing_newline(&text));
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);

        if let Some(cursor_chars) = cursor {
            let clamped = cursor_chars.min(text.chars().count());
            let (line, col) = line_col_from_char_index(&text, clamped);
            let row = line.min(u16::MAX as usize) as u16;
            let col = col.min(u16::MAX as usize) as u16;
            textarea.move_cursor(CursorMove::Jump(row, col));
        } else {
            textarea.move_cursor(CursorMove::Jump(u16::MAX, u16::MAX));
        }

        Self { text, textarea }
    }

    fn sync_text_from_textarea(&mut self) {
        self.text = self.textarea.lines().join("\n");
    }
}

fn blocks_single_line(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter)
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M')))
}

fn non_mutating_text_key_consumed(key: KeyEvent, multiline: bool) -> bool {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return matches!(
            key.code,
            KeyCode::Char(_)
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Delete
                | KeyCode::Backspace
                | KeyCode::Tab
                | KeyCode::BackTab
        );
    }

    matches!(
        key.code,
        KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Delete
            | KeyCode::Backspace
            | KeyCode::PageUp
            | KeyCode::PageDown
    ) || (multiline && matches!(key.code, KeyCode::Up | KeyCode::Down))
}

fn split_lines_preserve_trailing_newline(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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

fn line_col_from_char_index(text: &str, cursor_chars: usize) -> (usize, usize) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn with_cursor_clamps_to_text_len() {
        let buf = TextBuffer::with_cursor("hi".to_string(), 99);
        assert_eq!(buf.cursor(), 2);
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

    #[test]
    fn left_right_move_cursor_single_line() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.handle_key(KeyCode::Left, false);
        assert_eq!(buf.cursor(), 2);
        buf.handle_key(KeyCode::Right, false);
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn ctrl_a_moves_cursor_to_line_head() {
        let mut buf = TextBuffer::new("abc".to_string());
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(buf.handle_key_event(key, false));
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn home_end_move_cursor_to_boundaries() {
        let mut buf = TextBuffer::new("abc".to_string());
        buf.handle_key(KeyCode::Home, false);
        assert_eq!(buf.cursor(), 0);
        buf.handle_key(KeyCode::End, false);
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn insert_char_advances_cursor() {
        let mut buf = TextBuffer::new("ac".to_string());
        buf.handle_key(KeyCode::Left, false);
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

    #[test]
    fn enter_inserts_newline_in_multiline_mode() {
        let mut buf = TextBuffer::new("ab".to_string());
        buf.handle_key(KeyCode::Home, true);
        buf.handle_key(KeyCode::Enter, true);
        assert!(buf.text().contains('\n'));
    }

    #[test]
    fn up_down_move_between_lines() {
        let mut buf = TextBuffer::new("line1\nline2".to_string());
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
        let (line, col) = buf.line_col();
        assert_eq!(line, 1);
        assert_eq!(col, 5);
    }

    #[test]
    fn line_col_for_single_line() {
        let buf = TextBuffer::new("abc".to_string());
        assert_eq!(buf.line_col(), (0, 3));
    }

    #[test]
    fn trimmed_strips_whitespace() {
        let buf = TextBuffer::new("  hello  ".to_string());
        assert_eq!(buf.trimmed(), "hello");
    }

    #[test]
    fn len_chars_counts_unicode() {
        let buf = TextBuffer::new("héllo".to_string());
        assert_eq!(buf.len_chars(), 5);
    }
}
