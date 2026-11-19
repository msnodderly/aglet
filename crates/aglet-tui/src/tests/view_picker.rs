use super::*;
use crate::harness::Harness;

#[test]
fn view_picker_delete_uses_x_and_removes_selected_view() {
    let store = Store::open_memory().expect("memory store");
    store
        .create_view(&View::new("Keep Me".to_string()))
        .expect("create keep view");
    store
        .create_view(&View::new("Remove Me".to_string()))
        .expect("create remove view");
    let mut h = Harness::with_store(store);
    let position = |h: &Harness, name: &str| {
        h.app
            .views
            .iter()
            .position(|view| view.name == name)
            .expect("view exists")
    };

    h.keys("v");
    assert_eq!(h.app.mode, Mode::ViewPicker);
    let steps = position(&h, "Remove Me") - position(&h, "All Items");
    h.keys(&"j ".repeat(steps)).keys("x");
    assert_eq!(h.app.mode, Mode::ViewDeleteConfirm);
    h.keys("y");
    assert_eq!(h.app.mode, Mode::ViewPicker);
    let names: Vec<String> = h
        .store
        .list_views()
        .expect("list views")
        .into_iter()
        .map(|view| view.name)
        .collect();
    assert!(!names.contains(&"Remove Me".to_string()), "{names:?}");
    assert!(names.contains(&"Keep Me".to_string()), "{names:?}");

    // 'd' is no longer a separate datebook-creation shortcut.
    h.keys("d");
    assert_eq!(h.app.mode, Mode::ViewPicker);
    assert!(h
        .app
        .status
        .contains("Use n:new, then set View type to Datebook"));
    assert_eq!(h.store.list_views().expect("list views").len(), names.len());
}
