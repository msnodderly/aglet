use super::*;
use crate::harness::Harness;

#[test]
fn help_panel_renders_two_columns_on_wide_terminals() {
    let mut h = Harness::new();
    h.size(220, 50).keys("?");
    assert_eq!(h.app.mode, Mode::HelpPanel);
    let screen = h.screen();
    assert!(
        screen.contains("CURRENT ITEM") && screen.contains("Quit"),
        "wide help panel should fit all sections without scrolling: {screen}"
    );
    assert!(
        screen
            .lines()
            .any(|line| line.contains("CURRENT ITEM") && line.contains("COLUMNS")),
        "wide help panel should place sections side by side: {screen}"
    );
}
