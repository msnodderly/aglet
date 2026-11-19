//! Test harness: drives an [`App`] the way `App::run` does, over an
//! in-memory store, and renders the screen as text.
//!
//! ```ignore
//! let mut h = Harness::new();
//! h.keys("n").type_text("Buy milk").keys("<Enter>");
//! assert!(h.screen().contains("Buy milk"));
//! ```

use aglet_core::aglet::Aglet;
use aglet_core::matcher::SubstringClassifier;
use aglet_core::store::Store;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::{App, Mode};

pub(crate) struct Harness {
    pub(crate) store: Store,
    pub(crate) app: App,
    width: u16,
    height: u16,
}

impl Harness {
    /// A fresh in-memory store with the default view loaded.
    pub(crate) fn new() -> Self {
        Self::with_store(Store::open_memory().expect("in-memory store"))
    }

    /// Wrap a store the test has already populated.
    pub(crate) fn with_store(store: Store) -> Self {
        let mut app = App::default();
        app.refresh(&store).expect("initial refresh");
        Self {
            store,
            app,
            width: 120,
            height: 40,
        }
    }

    /// Switch to a view by name, as the view picker would.
    pub(crate) fn view(&mut self, name: &str) -> &mut Self {
        self.app.set_view_selection_by_name(name);
        self.refresh()
    }

    /// Reload items, categories, and views from the store.
    pub(crate) fn refresh(&mut self) -> &mut Self {
        self.app.refresh(&self.store).expect("refresh");
        self
    }

    pub(crate) fn size(&mut self, width: u16, height: u16) -> &mut Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Press keys. Tokens are separated by spaces. `<Enter>`, `<Esc>`,
    /// `<Tab>`, `<BackTab>`, `<Space>`, `<Backspace>`, `<Delete>`, `<Up>`,
    /// `<Down>`, `<Left>`, `<Right>`, `<Home>`, `<End>`, `<PageUp>`, and
    /// `<PageDown>` name special keys; `<C-x>` is Ctrl+x and `<A-x>` is
    /// Alt+x. Any other token types its characters one by one.
    pub(crate) fn keys(&mut self, keys: &str) -> &mut Self {
        for token in keys.split_whitespace() {
            match parse_special(token) {
                Some(event) => self.press(event),
                None => {
                    for ch in token.chars() {
                        self.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
                    }
                }
            }
        }
        self
    }

    /// Press `keys` until `reached` holds, at most `limit` times, and panic
    /// if it never does. Use it to walk to a row so a navigation regression
    /// fails the test instead of looping forever.
    pub(crate) fn press_until(
        &mut self,
        keys: &str,
        limit: usize,
        reached: impl Fn(&App) -> bool,
    ) -> &mut Self {
        for _ in 0..limit {
            if reached(&self.app) {
                return self;
            }
            self.keys(keys);
        }
        assert!(
            reached(&self.app),
            "target not reached after pressing {keys:?} {limit} times"
        );
        self
    }

    /// Type text literally, spaces included.
    pub(crate) fn type_text(&mut self, text: &str) -> &mut Self {
        for ch in text.chars() {
            self.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        self
    }

    /// One key press, handled like `App::run`: an error returns to Normal
    /// mode and shows in the status line.
    pub(crate) fn press(&mut self, key: KeyEvent) {
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&self.store, &classifier);
        if let Err(err) = self.app.handle_key_event(key, &aglet) {
            self.app.mode = Mode::Normal;
            self.app.clear_input();
            self.app.status = format!("Error: {err}");
        }
    }

    /// The rendered screen, one line per terminal row.
    pub(crate) fn screen(&mut self) -> String {
        let mut terminal =
            Terminal::new(TestBackend::new(self.width, self.height)).expect("test terminal");
        terminal.draw(|frame| self.app.draw(frame)).expect("render");
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Texts of the items in the selected slot, in display order.
    pub(crate) fn slot_items(&self) -> Vec<String> {
        self.app
            .slots
            .get(self.app.slot_index)
            .map(|slot| slot.items.iter().map(|item| item.text.clone()).collect())
            .unwrap_or_default()
    }
}

fn parse_special(token: &str) -> Option<KeyEvent> {
    let inner = token.strip_prefix('<')?.strip_suffix('>')?;
    let (modifiers, name) = match inner.split_once('-') {
        Some(("C", rest)) if !rest.is_empty() => (KeyModifiers::CONTROL, rest),
        Some(("A", rest)) if !rest.is_empty() => (KeyModifiers::ALT, rest),
        Some(("S", rest)) if !rest.is_empty() => (KeyModifiers::SHIFT, rest),
        _ => (KeyModifiers::NONE, inner),
    };
    let code = match name {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "BackTab" => KeyCode::BackTab,
        "Space" => KeyCode::Char(' '),
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        single if single.chars().count() == 1 => KeyCode::Char(single.chars().next()?),
        _ => return None,
    };
    Some(KeyEvent::new(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_and_modified_keys() {
        assert_eq!(
            parse_special("<C-s>"),
            Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            parse_special("<Enter>"),
            Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        );
        assert_eq!(parse_special("plain"), None);
    }

    #[test]
    fn ctrl_s_reaches_the_input_panel_as_a_modified_key() {
        let mut h = Harness::new();
        h.keys("n").type_text("Call the vet").keys("<C-s>");
        assert_eq!(h.app.mode, Mode::Normal, "status: {}", h.app.status);
        let texts: Vec<String> = h
            .store
            .list_items()
            .expect("list items")
            .into_iter()
            .map(|item| item.text)
            .collect();
        assert_eq!(texts, ["Call the vet"]);
    }

    #[test]
    #[should_panic(expected = "target not reached")]
    fn press_until_fails_instead_of_spinning() {
        Harness::new().press_until("j", 5, |_| false);
    }

    #[test]
    fn adds_an_item_and_shows_it_on_screen() {
        let mut h = Harness::new();
        h.keys("n").type_text("Buy milk").keys("<Enter>");
        assert_eq!(h.app.mode, Mode::Normal, "status: {}", h.app.status);
        assert!(h.screen().contains("Buy milk"), "{}", h.screen());
    }
}
