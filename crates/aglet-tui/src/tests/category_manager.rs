use super::*;
use crate::harness::Harness;
use aglet_core::model::{Condition, CriterionMode};

#[test]
fn category_manager_condition_edit_add_and_save() {
    let store = Store::open_memory().expect("memory store");
    let urgent = Category::new("Urgent".to_string());
    let escalated = Category::new("Escalated".to_string());
    for category in [
        &urgent,
        &Category::new("Project Alpha".to_string()),
        &escalated,
    ] {
        store.create_category(category).expect("create category");
    }
    let mut h = Harness::with_store(store);

    h.keys("c");
    let rows = h.app.category_rows.len();
    h.press_until("<Down>", rows, |app| {
        app.selected_category_row()
            .is_some_and(|row| row.name == "Escalated")
    });
    // Details, then also-match, then conditions; Enter opens the list.
    h.keys("<Tab> <Tab> <Tab> <Enter>");
    let edit = h
        .app
        .category_manager_condition_edit()
        .expect("condition list open");
    assert!(!edit.picker_open);

    h.keys("a");
    assert!(h.app.category_manager_condition_edit().unwrap().picker_open);
    h.press_until("j", rows, |app| {
        app.category_manager_condition_edit()
            .and_then(|edit| app.category_rows.get(edit.picker_index))
            .is_some_and(|row| row.name == "Urgent")
    });
    h.keys("<Space>");
    let edit = h.app.category_manager_condition_edit().unwrap();
    assert_eq!(
        edit.draft_query.mode_for(urgent.id),
        Some(CriterionMode::And)
    );

    h.keys("<Enter>");
    let saved = h.store.get_category(escalated.id).expect("load");
    match saved.conditions.as_slice() {
        [Condition::Profile { criteria }] => {
            assert_eq!(criteria.and_category_ids().collect::<Vec<_>>(), [urgent.id]);
        }
        other => panic!("expected one Profile condition, got {other:?}"),
    }
    assert!(!h.app.category_manager_condition_edit().unwrap().picker_open);

    h.keys("<Esc>");
    assert!(h.app.category_manager_condition_edit().is_none());
}
