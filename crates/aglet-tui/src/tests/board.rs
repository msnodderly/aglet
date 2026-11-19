use super::*;
use crate::harness::Harness;

/// A store with a "Board" view: one section with one column headed by
/// `heading`.
fn board_store(heading: &Category) -> Store {
    let store = Store::open_memory().expect("memory store");
    store.create_category(heading).expect("create heading");
    let mut view = View::new("Board".to_string());
    view.sections.push(Section {
        title: "Main".to_string(),
        criteria: Query::default(),
        columns: vec![Column {
            kind: ColumnKind::Standard,
            heading: heading.id,
            width: 12,
            summary_fn: None,
        }],
        item_column_index: 0,
        on_insert_assign: std::collections::HashSet::new(),
        on_remove_unassign: std::collections::HashSet::new(),
        show_children: false,
        board_display_mode_override: None,
    });
    store.create_view(&view).expect("create board view");
    store
}

#[test]
fn board_sorting_childless_category_column_puts_assigned_items_first() {
    let urgent = Category::new("Urgent".to_string());
    let store = board_store(&urgent);
    let alpha = Item::new("Alpha".to_string());
    store.create_item(&alpha).expect("create alpha");
    store
        .create_item(&Item::new("Bravo".to_string()))
        .expect("create bravo");
    let classifier = SubstringClassifier;
    Aglet::new(&store, &classifier)
        .assign_item_manual(alpha.id, urgent.id, Some("test:assign".to_string()))
        .expect("assign alpha urgent");

    let mut h = Harness::with_store(store);
    h.view("Board");
    assert_eq!(h.slot_items(), ["Bravo", "Alpha"]);

    h.keys("l s");
    assert_eq!(h.slot_items(), ["Alpha", "Bravo"]);
}
