use super::*;
use crate::harness::Harness;

#[test]
fn help_panel_renders_two_columns_on_wide_terminals() {
    let mut h = Harness::new();
    h.size(220, 50).keys("?");
    assert_eq!(h.app.mode, Mode::HelpPanel { scroll: 0 });
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

#[test]
fn help_panel_scrolls_to_last_section_on_small_terminals() {
    let mut h = Harness::new();
    h.size(100, 30).keys("?");
    let rendered = h.screen();
    assert!(
        rendered.contains("CURRENT ITEM"),
        "top of help should be visible before scrolling: {rendered}"
    );
    assert!(
        !rendered.contains("Quit"),
        "GLOBAL section should be clipped at 100x30 before scrolling: {rendered}"
    );
    assert!(
        rendered.contains("\u{2026} more \u{2193}"),
        "clipped help should show a more-content indicator: {rendered}"
    );

    h.keys("<End>");
    let rendered = h.screen();
    assert!(
        rendered.contains("Quit"),
        "last help section must be reachable by scrolling: {rendered}"
    );
    assert!(
        !rendered.contains("\u{2026} more \u{2193}"),
        "more-indicator should clear at the bottom: {rendered}"
    );
}

#[test]
fn help_panel_key_labels_never_run_into_descriptions() {
    let mut h = Harness::new();
    h.size(220, 50).keys("?");
    let rendered = h.screen();
    // Audit symptom: fixed 12-char gutter overflowed for wide key labels and
    // rendered "v/V/F8 ,/. gaViews, previous/next view…".
    assert!(
        !rendered.contains("gaViews"),
        "key label must not run into its description: {rendered}"
    );
    assert!(
        rendered.contains("v/V/F8 ,/. ga "),
        "wide key label should keep a gutter before its description: {rendered}"
    );
}

#[test]
fn help_panel_render_contains_shortcut_cheat_sheet() {
    let mut h = Harness::new();
    h.size(120, 60).keys("?");
    let rendered = h.screen();

    assert!(
        rendered.contains("Keyboard Shortcuts"),
        "help panel title should render: {rendered}"
    );
    assert!(
        rendered.contains("CURRENT ITEM"),
        "help panel should include CURRENT ITEM group header: {rendered}"
    );
    assert!(
        rendered.contains("NAVIGATION"),
        "help panel should include NAVIGATION group header: {rendered}"
    );
    assert!(
        rendered.contains("GLOBAL"),
        "help panel should include GLOBAL group header: {rendered}"
    );
    assert!(
        rendered.contains("Search focused section / all sections"),
        "help panel should include global search description: {rendered}"
    );
    assert!(
        rendered.contains("Open Global Settings"),
        "help panel should include global settings shortcut: {rendered}"
    );
    assert!(
        rendered.contains("Assign categories to current item or selection"),
        "help panel should include updated assign description: {rendered}"
    );
    assert!(
        rendered.contains("Open dependency link wizard"),
        "help panel should include current dependency link shortcuts: {rendered}"
    );
    assert!(
        rendered.contains("Classify selected item(s) now"),
        "help panel should include manual classification shortcut: {rendered}"
    );
    assert!(
        rendered.contains("Step previous / next bucket"),
        "help panel should include datebook browsing shortcuts: {rendered}"
    );
    assert!(
        !rendered.contains("Ctrl-R"),
        "help panel should no longer advertise Ctrl-R: {rendered}"
    );
}
