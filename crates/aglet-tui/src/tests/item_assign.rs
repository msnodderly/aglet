use super::*;
use crate::harness::Harness;

#[test]
fn item_assign_input_renders_inside_popup_not_footer() {
    let store = Store::open_memory().expect("memory store");
    store
        .create_category(&Category::new("Alpha".to_string()))
        .expect("create category");
    store
        .create_item(&Item::new("Ride".to_string()))
        .expect("create item");
    let mut h = Harness::with_store(store);
    h.size(110, 30).keys("a / moto");
    assert_eq!(
        h.app.mode,
        Mode::ItemAssignInput,
        "status: {}",
        h.app.status
    );

    let screen = h.screen();
    let lines: Vec<&str> = screen.lines().collect();
    let (popup_lines, footer_lines) = lines.split_at(lines.len() - 4);
    assert!(
        popup_lines.iter().any(|line| line.contains("moto")),
        "typed query should render inside the assign popup: {screen}"
    );
    assert!(
        popup_lines.iter().any(|line| line.contains("> Category")),
        "popup should show a focused category input block: {screen}"
    );
    assert!(
        !footer_lines.iter().any(|line| line.contains("moto")),
        "footer should keep hints, not the typed query: {footer_lines:?}"
    );

    let popup_area = crate::centered_rect(88, 72, Rect::new(0, 0, 110, 30));
    let cursor = h
        .app
        .item_assign_input_cursor_position(popup_area)
        .expect("cursor should be positioned in the popup input");
    assert!(
        cursor.0 > popup_area.x
            && cursor.0 < popup_area.x + popup_area.width
            && cursor.1 > popup_area.y
            && cursor.1 < popup_area.y + popup_area.height,
        "cursor {cursor:?} should sit inside the popup {popup_area:?}"
    );
}
