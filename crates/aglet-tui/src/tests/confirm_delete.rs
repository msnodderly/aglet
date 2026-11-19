use super::*;
use crate::harness::Harness;

#[test]
fn confirm_delete_popup_names_single_item() {
    let store = Store::open_memory().expect("memory store");
    store
        .create_item(&Item::new("Wash DRZ".to_string()))
        .expect("create item");
    let mut h = Harness::with_store(store);
    h.size(110, 28).keys("x");
    assert_eq!(h.app.mode, Mode::ConfirmDelete, "status: {}", h.app.status);

    let screen = h.screen();
    assert!(
        screen.contains("Delete \"Wash DRZ\"?"),
        "confirm popup should quote the item title: {screen}"
    );
    assert!(
        screen.contains("restorable via 'aglet deleted'"),
        "confirm popup should note recoverability: {screen}"
    );
    assert!(
        screen.contains("y:confirm"),
        "confirm popup should show keys: {screen}"
    );
}
