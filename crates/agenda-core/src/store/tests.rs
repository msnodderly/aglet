    use super::*;
    use crate::model::{
        Assignment, AssignmentSource, BoardDisplayMode, Category, CategoryValueKind, Column,
        ColumnKind, CriterionMode, Item, ItemId, ItemLink, ItemLinkKind, NumericFormat, Query, Section,
        View, RESERVED_CATEGORY_NAME_DONE, RESERVED_CATEGORY_NAME_WHEN, RESERVED_CATEGORY_NAMES,
    };
    use chrono::{Duration, Utc};
    use rusqlite::params;
    use rust_decimal::Decimal;
    use std::collections::{BTreeMap, HashSet};
    use uuid::Uuid;

    fn new_category(name: &str) -> Category {
        Category::new(name.to_string())
    }

    fn new_view(name: &str) -> View {
        View::new(name.to_string())
    }

    fn make_item(store: &Store, text: &str) -> ItemId {
        let item = Item::new(text.to_string());
        let id = item.id;
        store.create_item(&item).unwrap();
        id
    }

    fn new_item_link(item_id: ItemId, other_item_id: ItemId, kind: ItemLinkKind) -> ItemLink {
        ItemLink {
            item_id,
            other_item_id,
            kind,
            created_at: Utc::now(),
            origin: Some("test".to_string()),
        }
    }

    fn category_id_by_name(store: &Store, name: &str) -> Uuid {
        let id: String = store
            .conn
            .query_row(
                "SELECT id FROM categories WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| row.get(0),
            )
            .unwrap();
        Uuid::parse_str(&id).unwrap()
    }

    fn child_names(store: &Store, parent_id: CategoryId) -> Vec<String> {
        let hierarchy = store.get_hierarchy().unwrap();
        let names_by_id: HashMap<CategoryId, String> = hierarchy
            .iter()
            .map(|category| (category.id, category.name.clone()))
            .collect();
        let parent = hierarchy
            .into_iter()
            .find(|category| category.id == parent_id)
            .expect("parent exists");
        parent
            .children
            .into_iter()
            .map(|id| names_by_id.get(&id).cloned().expect("child name exists"))
            .collect()
    }

    fn root_names(store: &Store) -> Vec<String> {
        store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .filter(|category| category.parent.is_none())
            .filter(|category| !Store::is_reserved_category_name(&category.name))
            .map(|category| category.name)
            .collect()
    }

    #[test]
    fn test_open_memory_creates_schema() {
        let store = Store::open_memory().expect("failed to open in-memory store");

        // Verify all tables exist by querying sqlite_master.
        let tables: Vec<String> = store
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"items".to_string()));
        assert!(tables.contains(&"categories".to_string()));
        assert!(tables.contains(&"assignments".to_string()));
        assert!(tables.contains(&"views".to_string()));
        assert!(tables.contains(&"deletion_log".to_string()));
        assert!(tables.contains(&"item_links".to_string()));
        assert!(tables.contains(&"app_settings".to_string()));
    }

    #[test]
    fn test_wal_mode_enabled() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let mode: String = store
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        // In-memory databases use "memory" journal mode, but the pragma was set.
        // For file-based DBs it would be "wal". Just verify no error.
        assert!(!mode.is_empty());
    }

    #[test]
    fn test_schema_version_set() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let version: i32 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let fk: i32 = store
            .conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_idempotent_init() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let reserved_before: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name IN ('When', 'Entry', 'Done')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let default_view_before: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM views WHERE name = 'All Items'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Calling init again should be idempotent.
        store.init().expect("second init should be idempotent");

        let reserved_after: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name IN ('When', 'Entry', 'Done')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let default_view_after: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM views WHERE name = 'All Items'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(reserved_before, 3);
        assert_eq!(reserved_after, 3);
        assert_eq!(default_view_before, 1);
        assert_eq!(default_view_after, 1);
    }

    #[test]
    fn test_upgrade_from_v5_creates_item_links_table_and_bumps_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 5).unwrap();

        let store = Store { conn };
        store.init().unwrap();

        let version: i32 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let exists: Option<String> = store
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='item_links'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(exists.as_deref(), Some("item_links"));

        let aliases_column_exists = store
            .column_exists("views", "category_aliases_json")
            .unwrap();
        assert!(
            aliases_column_exists,
            "view aliases column should be present after migration"
        );
    }

    #[test]
    fn test_upgrade_from_v6_adds_view_category_aliases_column_for_existing_views_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE views (
                id                          TEXT PRIMARY KEY,
                name                        TEXT NOT NULL UNIQUE,
                criteria_json               TEXT NOT NULL DEFAULT '{}',
                sections_json               TEXT NOT NULL DEFAULT '[]',
                columns_json                TEXT NOT NULL DEFAULT '[]',
                show_unmatched              INTEGER NOT NULL DEFAULT 1,
                unmatched_label             TEXT NOT NULL DEFAULT 'Unassigned',
                remove_from_view_unassign_json TEXT NOT NULL DEFAULT '[]',
                item_column_label           TEXT,
                board_display_mode          TEXT NOT NULL DEFAULT 'SingleLine'
            );
            "#,
        )
        .unwrap();

        let legacy_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO views (
                id, name, criteria_json, sections_json, columns_json,
                show_unmatched, unmatched_label, remove_from_view_unassign_json,
                item_column_label, board_display_mode
            ) VALUES (?1, 'Legacy', '{}', '[]', '[]', 1, 'Unassigned', '[]', NULL, '\"SingleLine\"')",
            params![legacy_id.to_string()],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 6).unwrap();

        let store = Store { conn };
        store.init().unwrap();

        let aliases_column_exists = store
            .column_exists("views", "category_aliases_json")
            .unwrap();
        assert!(aliases_column_exists, "migration should add aliases column");

        let legacy = store.get_view(legacy_id).expect("legacy view loads");
        assert!(
            legacy.category_aliases.is_empty(),
            "legacy rows default to no aliases"
        );
    }

    #[test]
    fn test_upgrade_from_v7_creates_app_settings_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 7).unwrap();

        let store = Store { conn };
        store.init().unwrap();

        let exists: Option<String> = store
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='app_settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(exists.as_deref(), Some("app_settings"));
    }

    #[test]
    fn test_app_settings_roundtrip_persists_across_reopen() {
        let tmp =
            std::env::temp_dir().join(format!("agenda-core-app-settings-{}.ag", Uuid::new_v4()));
        let store = Store::open(&tmp).expect("open temp db");
        store
            .set_app_setting("tui.auto_refresh_interval", "5s")
            .expect("write setting");
        drop(store);

        let reopened = Store::open(&tmp).expect("reopen temp db");
        let value = reopened
            .get_app_setting("tui.auto_refresh_interval")
            .expect("read setting");
        assert_eq!(value.as_deref(), Some("5s"));

        drop(reopened);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_first_launch_creates_reserved_categories_and_default_view() {
        let store = Store::open_memory().expect("failed to open in-memory store");

        let categories: Vec<String> = store
            .conn
            .prepare("SELECT name FROM categories ORDER BY sort_order ASC")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(categories, vec!["When", "Entry", "Done"]);

        // Reserved categories should have implicit string matching disabled
        // so common words like "done" or "when" don't trigger auto-assignment.
        for name in RESERVED_CATEGORY_NAMES {
            let cat = store
                .get_category(category_id_by_name(&store, name))
                .unwrap();
            assert!(
                !cat.enable_implicit_string,
                "{name} should have enable_implicit_string = false"
            );
            assert!(
                !cat.is_actionable,
                "{name} should have is_actionable = false"
            );
        }

        let _when_id = category_id_by_name(&store, RESERVED_CATEGORY_NAME_WHEN);
        let all_items_view: String = store
            .conn
            .query_row("SELECT id FROM views WHERE name = 'All Items'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let view = store
            .get_view(Uuid::parse_str(&all_items_view).unwrap())
            .unwrap();

        assert_eq!(view.name, "All Items");
        assert!(view.criteria.criteria.is_empty());
        assert!(view.sections.is_empty());
    }

    #[test]
    fn test_create_and_get_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Buy groceries".to_string());
        let id = item.id;
        store.create_item(&item).unwrap();

        let loaded = store.get_item(id).unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.text, "Buy groceries");
        assert!(!loaded.is_done);
        assert!(loaded.note.is_none());
    }

    #[test]
    fn test_get_item_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.get_item(Uuid::new_v4());
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_update_item() {
        let store = Store::open_memory().unwrap();
        let mut item = Item::new("Draft".to_string());
        store.create_item(&item).unwrap();

        item.text = "Final version".to_string();
        item.note = Some("Added details".to_string());
        item.modified_at = Utc::now();
        store.update_item(&item).unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.text, "Final version");
        assert_eq!(loaded.note.as_deref(), Some("Added details"));
    }

    #[test]
    fn test_update_nonexistent_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Ghost".to_string());
        let result = store.update_item(&item);
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_delete_item_writes_log() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("To be deleted".to_string());
        let id = item.id;
        store.create_item(&item).unwrap();

        store.delete_item(id, "user").unwrap();

        // Item should be gone.
        assert!(matches!(
            store.get_item(id),
            Err(AgendaError::NotFound { .. })
        ));

        // Deletion log should have an entry.
        let count: i32 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM deletion_log WHERE item_id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_deleted_items_returns_latest_first() {
        let store = Store::open_memory().unwrap();

        let first = Item::new("First deleted".to_string());
        let second = Item::new("Second deleted".to_string());
        store.create_item(&first).unwrap();
        store.create_item(&second).unwrap();

        store.delete_item(first.id, "user").unwrap();
        store.delete_item(second.id, "user").unwrap();

        let deleted = store.list_deleted_items().unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].item_id, second.id);
        assert_eq!(deleted[1].item_id, first.id);
    }

    #[test]
    fn test_restore_deleted_item_recreates_item_and_assignments() {
        let store = Store::open_memory().unwrap();
        let category_id = make_category(&store, "RestoreTarget");

        let item = Item::new("Restore me".to_string());
        store.create_item(&item).unwrap();
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
            numeric_value: None,
        };
        store
            .assign_item(item.id, category_id, &assignment)
            .unwrap();
        store.delete_item(item.id, "user").unwrap();

        let log_entry_id: Uuid = store
            .conn
            .query_row(
                "SELECT id FROM deletion_log WHERE item_id = ?1 ORDER BY deleted_at DESC LIMIT 1",
                params![item.id.to_string()],
                |row| {
                    let id_str: String = row.get(0)?;
                    Ok(Uuid::parse_str(&id_str).unwrap())
                },
            )
            .unwrap();

        let restored_item_id = store.restore_deleted_item(log_entry_id).unwrap();
        assert_eq!(restored_item_id, item.id);

        let restored = store.get_item(restored_item_id).unwrap();
        assert_eq!(restored.text, "Restore me");
        let assignments = store.get_assignments_for_item(restored_item_id).unwrap();
        assert!(assignments.contains_key(&category_id));
    }

    #[test]
    fn test_list_items() {
        let store = Store::open_memory().unwrap();
        store.create_item(&Item::new("First".to_string())).unwrap();
        store.create_item(&Item::new("Second".to_string())).unwrap();

        let items = store.list_items().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_create_items_allows_duplicate_text_with_distinct_ids() {
        let store = Store::open_memory().unwrap();
        let first = Item::new("Buy milk".to_string());
        let second = Item::new("Buy milk".to_string());
        assert_ne!(first.id, second.id);

        store.create_item(&first).unwrap();
        store.create_item(&second).unwrap();

        let items = store.list_items().unwrap();
        let duplicates: Vec<&Item> = items
            .iter()
            .filter(|item| item.text == "Buy milk")
            .collect();
        assert_eq!(duplicates.len(), 2);
        assert_ne!(duplicates[0].id, duplicates[1].id);
    }

    #[test]
    fn test_create_item_link_exists_and_delete_item_link() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        let link = new_item_link(a, b, ItemLinkKind::DependsOn);
        store.create_item_link(&link).unwrap();
        assert!(store
            .item_link_exists(a, b, ItemLinkKind::DependsOn)
            .unwrap());

        store
            .delete_item_link(a, b, ItemLinkKind::DependsOn)
            .unwrap();
        assert!(!store
            .item_link_exists(a, b, ItemLinkKind::DependsOn)
            .unwrap());

        // Idempotent delete.
        store
            .delete_item_link(a, b, ItemLinkKind::DependsOn)
            .unwrap();
    }

    #[test]
    fn test_list_dependency_ids_for_item_returns_outbound_depends_on() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");
        let d = make_item(&store, "D");

        store
            .create_item_link(&new_item_link(a, b, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(a, c, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(d, a, ItemLinkKind::DependsOn))
            .unwrap();

        let deps = store.list_dependency_ids_for_item(a).unwrap();
        assert_eq!(deps, vec![b, c]);
    }

    #[test]
    fn test_list_dependent_ids_for_item_returns_inverse_blocks_view() {
        let store = Store::open_memory().unwrap();
        let blocker = make_item(&store, "Blocker");
        let dep1 = make_item(&store, "Dep1");
        let dep2 = make_item(&store, "Dep2");
        let unrelated = make_item(&store, "Unrelated");

        store
            .create_item_link(&new_item_link(dep1, blocker, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(dep2, blocker, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(unrelated, dep1, ItemLinkKind::DependsOn))
            .unwrap();

        let dependents = store.list_dependent_ids_for_item(blocker).unwrap();
        assert_eq!(dependents, vec![dep1, dep2]);
    }

    #[test]
    fn test_list_related_ids_for_item_is_symmetric() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        let (ab_left, ab_right) = if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        };
        let (ac_left, ac_right) = if a.to_string() < c.to_string() {
            (a, c)
        } else {
            (c, a)
        };

        store
            .create_item_link(&new_item_link(ab_left, ab_right, ItemLinkKind::Related))
            .unwrap();
        store
            .create_item_link(&new_item_link(ac_left, ac_right, ItemLinkKind::Related))
            .unwrap();

        let related_to_a = store.list_related_ids_for_item(a).unwrap();
        assert_eq!(related_to_a, vec![b, c]);

        let related_to_b = store.list_related_ids_for_item(b).unwrap();
        assert_eq!(related_to_b, vec![a]);
    }

    #[test]
    fn test_list_item_links_for_item_includes_inbound_outbound_and_related() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");
        let d = make_item(&store, "D");

        store
            .create_item_link(&new_item_link(a, b, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(c, a, ItemLinkKind::DependsOn))
            .unwrap();
        let (left, right) = if a.to_string() < d.to_string() {
            (a, d)
        } else {
            (d, a)
        };
        store
            .create_item_link(&new_item_link(left, right, ItemLinkKind::Related))
            .unwrap();

        let links = store.list_item_links_for_item(a).unwrap();
        assert_eq!(links.len(), 3);
        assert!(links
            .iter()
            .any(|l| l.kind == ItemLinkKind::DependsOn && l.item_id == a && l.other_item_id == b));
        assert!(links
            .iter()
            .any(|l| l.kind == ItemLinkKind::DependsOn && l.item_id == c && l.other_item_id == a));
        assert!(links.iter().any(|l| l.kind == ItemLinkKind::Related
            && ((l.item_id == a && l.other_item_id == d)
                || (l.item_id == d && l.other_item_id == a))));
    }

    #[test]
    fn test_item_links_disallow_self_link_via_db_check() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let result = store.create_item_link(&new_item_link(a, a, ItemLinkKind::DependsOn));
        assert!(matches!(result, Err(AgendaError::StorageError { .. })));
    }

    #[test]
    fn test_item_links_related_requires_normalized_order_via_db_check() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let (low, high) = if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        };

        // Reverse order should violate CHECK(kind <> 'related' OR item_id < other_item_id).
        let result = store.create_item_link(&new_item_link(high, low, ItemLinkKind::Related));
        assert!(matches!(result, Err(AgendaError::StorageError { .. })));
    }

    #[test]
    fn test_item_links_allow_depends_on_and_related_for_same_pair() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let (low, high) = if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        };

        store
            .create_item_link(&new_item_link(a, b, ItemLinkKind::DependsOn))
            .unwrap();
        store
            .create_item_link(&new_item_link(low, high, ItemLinkKind::Related))
            .unwrap();

        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM item_links WHERE (item_id = ?1 AND other_item_id = ?2 AND kind = 'depends-on')
                   OR (item_id = ?3 AND other_item_id = ?4 AND kind = 'related')",
                params![a.to_string(), b.to_string(), low.to_string(), high.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_delete_item_cascades_item_links() {
        let store = Store::open_memory().unwrap();
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        store
            .create_item_link(&new_item_link(a, b, ItemLinkKind::DependsOn))
            .unwrap();
        assert_eq!(
            store
                .conn
                .query_row("SELECT COUNT(*) FROM item_links", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        store.delete_item(a, "test").unwrap();

        assert_eq!(
            store
                .conn
                .query_row("SELECT COUNT(*) FROM item_links", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_item_with_assignments_loaded() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test assignments".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        // Insert a category and assignment directly.
        let cat_id = Uuid::new_v4();
        store
            .conn
            .execute(
                "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
                params![cat_id.to_string(), "TestCat", Utc::now().to_rfc3339()],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO assignments (item_id, category_id, source, assigned_at, sticky, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item_id.to_string(),
                    cat_id.to_string(),
                    "Manual",
                    Utc::now().to_rfc3339(),
                    1,
                    "manual",
                ],
            )
            .unwrap();

        let loaded = store.get_item(item_id).unwrap();
        assert_eq!(loaded.assignments.len(), 1);
        assert!(loaded.assignments.contains_key(&cat_id));
        assert_eq!(loaded.assignments[&cat_id].source, AssignmentSource::Manual);
    }

    fn make_category(store: &Store, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        store
            .conn
            .execute(
                "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
                params![id.to_string(), name, Utc::now().to_rfc3339()],
            )
            .unwrap();
        id
    }

    #[test]
    fn test_assign_and_get_assignments() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Project");
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual".to_string()),
            numeric_value: None,
        };
        store.assign_item(item_id, cat_id, &assignment).unwrap();

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert!(assignments.contains_key(&cat_id));
        assert_eq!(assignments[&cat_id].source, AssignmentSource::Manual);
        assert_eq!(assignments[&cat_id].origin.as_deref(), Some("manual"));
    }

    #[test]
    fn test_assign_and_get_numeric_assignment_value() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Expense item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let mut cat = new_category("Cost");
        cat.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cat).unwrap();

        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual".to_string()),
            numeric_value: Some(Decimal::new(24596, 2)),
        };
        store.assign_item(item_id, cat.id, &assignment).unwrap();

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(
            assignments.get(&cat.id).and_then(|a| a.numeric_value),
            Some(Decimal::new(24596, 2))
        );
    }

    #[test]
    fn test_assign_upsert_replaces() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Status");
        let a1 = Assignment {
            source: AssignmentSource::AutoMatch,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("cat:Status".to_string()),
            numeric_value: None,
        };
        store.assign_item(item_id, cat_id, &a1).unwrap();

        // Re-assign with different source — should replace.
        let a2 = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: false,
            origin: Some("manual".to_string()),
            numeric_value: None,
        };
        store.assign_item(item_id, cat_id, &a2).unwrap();

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[&cat_id].source, AssignmentSource::Manual);
        assert!(!assignments[&cat_id].sticky);
    }

    #[test]
    fn test_unassign_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Remove");
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: None,
            numeric_value: None,
        };
        store.assign_item(item_id, cat_id, &assignment).unwrap();
        assert_eq!(store.get_assignments_for_item(item_id).unwrap().len(), 1);

        store.unassign_item(item_id, cat_id).unwrap();
        assert_eq!(store.get_assignments_for_item(item_id).unwrap().len(), 0);
    }

    #[test]
    fn test_unassign_nonexistent_is_ok() {
        let store = Store::open_memory().unwrap();
        // Unassigning something that doesn't exist should not error.
        store.unassign_item(Uuid::new_v4(), Uuid::new_v4()).unwrap();
    }

    #[test]
    fn test_multiple_assignments() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Multi-assign".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat1 = make_category(&store, "Cat1");
        let cat2 = make_category(&store, "Cat2");
        let cat3 = make_category(&store, "Cat3");

        for (cat_id, src) in [
            (cat1, AssignmentSource::Manual),
            (cat2, AssignmentSource::AutoMatch),
            (cat3, AssignmentSource::Subsumption),
        ] {
            let a = Assignment {
                source: src,
                assigned_at: Utc::now(),
                sticky: true,
                origin: None,
                numeric_value: None,
            };
            store.assign_item(item_id, cat_id, &a).unwrap();
        }

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[&cat1].source, AssignmentSource::Manual);
        assert_eq!(assignments[&cat2].source, AssignmentSource::AutoMatch);
        assert_eq!(assignments[&cat3].source, AssignmentSource::Subsumption);
    }

    #[test]
    fn test_category_name_unique_case_insensitive() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let category = new_category("TestCat");
        store.create_category(&category).unwrap();

        let duplicate = new_category("testcat");
        let result = store.create_category(&duplicate);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "testcat"
        ));
    }

    #[test]
    fn test_create_and_get_category() {
        let store = Store::open_memory().unwrap();
        let mut root = new_category("Projects");
        root.is_exclusive = true;
        root.note = Some("top-level".to_string());
        store.create_category(&root).unwrap();

        let mut child = new_category("Aglet");
        child.parent = Some(root.id);
        store.create_category(&child).unwrap();

        let loaded_root = store.get_category(root.id).unwrap();
        assert_eq!(loaded_root.name, "Projects");
        assert!(loaded_root.children.contains(&child.id));
        assert!(loaded_root.is_exclusive);
        assert_eq!(loaded_root.note.as_deref(), Some("top-level"));

        let loaded_child = store.get_category(child.id).unwrap();
        assert_eq!(loaded_child.parent, Some(root.id));
    }

    #[test]
    fn test_create_and_get_numeric_category_roundtrip() {
        let store = Store::open_memory().unwrap();
        let mut category = new_category("Cost");
        category.value_kind = CategoryValueKind::Numeric;
        category.numeric_format = Some(NumericFormat {
            decimal_places: 2,
            currency_symbol: Some("$".to_string()),
            use_thousands_separator: true,
        });
        store.create_category(&category).unwrap();

        let loaded = store.get_category(category.id).unwrap();
        assert_eq!(loaded.value_kind, CategoryValueKind::Numeric);
        assert_eq!(
            loaded
                .numeric_format
                .as_ref()
                .and_then(|f| f.currency_symbol.as_deref()),
            Some("$")
        );
        assert_eq!(
            loaded
                .numeric_format
                .as_ref()
                .map(|f| f.use_thousands_separator),
            Some(true)
        );
    }

    #[test]
    fn test_create_category_rejects_child_under_numeric_parent() {
        let store = Store::open_memory().unwrap();
        let mut parent = new_category("Cost");
        parent.value_kind = CategoryValueKind::Numeric;
        store.create_category(&parent).unwrap();

        let mut child = new_category("SubCost");
        child.parent = Some(parent.id);

        let err = store.create_category(&child).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
        assert!(err.to_string().contains("numeric category"));
    }

    #[test]
    fn test_create_category_rejects_reserved_names() {
        let store = Store::open_memory().unwrap();
        let reserved = new_category("wHeN");
        let result = store.create_category(&reserved);
        assert!(matches!(
            result,
            Err(AgendaError::ReservedName { name }) if name == "wHeN"
        ));
    }

    #[test]
    fn test_create_category_with_invalid_parent_rejected() {
        let store = Store::open_memory().unwrap();
        let mut category = new_category("Orphan");
        category.parent = Some(Uuid::new_v4());

        let result = store.create_category(&category);
        assert!(matches!(
            result,
            Err(AgendaError::NotFound {
                entity: "Category",
                ..
            })
        ));
    }

    #[test]
    fn test_update_category_touches_modified_at() {
        let store = Store::open_memory().unwrap();
        let mut category = new_category("Draft");
        category.modified_at = Utc::now() - Duration::minutes(10);
        store.create_category(&category).unwrap();

        let original_modified_at = category.modified_at;
        category.name = "Published".to_string();
        category.enable_implicit_string = false;
        category.note = Some("updated".to_string());
        store.update_category(&category).unwrap();

        let loaded = store.get_category(category.id).unwrap();
        assert_eq!(loaded.name, "Published");
        assert!(!loaded.enable_implicit_string);
        assert_eq!(loaded.note.as_deref(), Some("updated"));
        assert!(loaded.modified_at > original_modified_at);
    }

    #[test]
    fn test_update_category_not_found() {
        let store = Store::open_memory().unwrap();
        let missing = new_category("Missing");
        let result = store.update_category(&missing);
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_update_category_rename_to_duplicate_rejected() {
        let store = Store::open_memory().unwrap();
        let mut one = new_category("One");
        let two = new_category("Two");
        store.create_category(&one).unwrap();
        store.create_category(&two).unwrap();

        one.name = "Two".to_string();
        let result = store.update_category(&one);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "Two"
        ));
    }

    #[test]
    fn test_update_category_reparent_cycle_rejected() {
        let store = Store::open_memory().unwrap();
        let root = new_category("Root");
        store.create_category(&root).unwrap();

        let mut child = new_category("Child");
        child.parent = Some(root.id);
        store.create_category(&child).unwrap();

        let mut updated_root = store.get_category(root.id).unwrap();
        updated_root.parent = Some(child.id);

        let result = store.update_category(&updated_root);
        assert!(matches!(result, Err(AgendaError::InvalidOperation { .. })));
    }

    #[test]
    fn test_move_category_within_parent_reorders_root_siblings() {
        let store = Store::open_memory().unwrap();
        let a = new_category("A");
        let b = new_category("B");
        let c = new_category("C");
        store.create_category(&a).unwrap();
        store.create_category(&b).unwrap();
        store.create_category(&c).unwrap();

        store.move_category_within_parent(c.id, -1).unwrap();
        assert_eq!(root_names(&store), vec!["A", "C", "B"]);

        store.move_category_within_parent(c.id, -10).unwrap();
        assert_eq!(root_names(&store), vec!["C", "A", "B"]);
    }

    #[test]
    fn test_move_category_within_parent_reorders_nested_siblings() {
        let store = Store::open_memory().unwrap();
        let parent = new_category("Parent");
        store.create_category(&parent).unwrap();

        let mut alpha = new_category("Alpha");
        alpha.parent = Some(parent.id);
        let mut beta = new_category("Beta");
        beta.parent = Some(parent.id);
        let mut gamma = new_category("Gamma");
        gamma.parent = Some(parent.id);
        store.create_category(&alpha).unwrap();
        store.create_category(&beta).unwrap();
        store.create_category(&gamma).unwrap();

        store.move_category_within_parent(gamma.id, -1).unwrap();
        assert_eq!(
            child_names(&store, parent.id),
            vec!["Alpha", "Gamma", "Beta"]
        );

        store.move_category_within_parent(alpha.id, 10).unwrap();
        assert_eq!(
            child_names(&store, parent.id),
            vec!["Gamma", "Beta", "Alpha"]
        );
    }

    #[test]
    fn test_move_category_to_parent_reparents_and_appends() {
        let store = Store::open_memory().unwrap();
        let left = new_category("Left");
        let right = new_category("Right");
        store.create_category(&left).unwrap();
        store.create_category(&right).unwrap();

        let mut child = new_category("Child");
        child.parent = Some(left.id);
        store.create_category(&child).unwrap();

        store
            .move_category_to_parent(child.id, Some(right.id), None)
            .unwrap();

        let loaded = store.get_category(child.id).unwrap();
        assert_eq!(loaded.parent, Some(right.id));
        assert_eq!(child_names(&store, left.id), Vec::<String>::new());
        assert_eq!(child_names(&store, right.id), vec!["Child"]);
    }

    #[test]
    fn test_move_category_to_parent_inserts_at_index() {
        let store = Store::open_memory().unwrap();
        let parent_a = new_category("ParentA");
        let parent_b = new_category("ParentB");
        store.create_category(&parent_a).unwrap();
        store.create_category(&parent_b).unwrap();

        let mut one = new_category("One");
        one.parent = Some(parent_b.id);
        let mut two = new_category("Two");
        two.parent = Some(parent_b.id);
        let mut moving = new_category("Moving");
        moving.parent = Some(parent_a.id);
        store.create_category(&one).unwrap();
        store.create_category(&two).unwrap();
        store.create_category(&moving).unwrap();

        store
            .move_category_to_parent(moving.id, Some(parent_b.id), Some(0))
            .unwrap();

        assert_eq!(
            child_names(&store, parent_b.id),
            vec!["Moving", "One", "Two"]
        );
    }

    #[test]
    fn test_move_category_to_parent_rejects_cycle() {
        let store = Store::open_memory().unwrap();
        let root = new_category("Root");
        store.create_category(&root).unwrap();
        let mut child = new_category("Child");
        child.parent = Some(root.id);
        store.create_category(&child).unwrap();
        let mut grandchild = new_category("Grandchild");
        grandchild.parent = Some(child.id);
        store.create_category(&grandchild).unwrap();

        let err = store
            .move_category_to_parent(root.id, Some(grandchild.id), None)
            .unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn test_delete_category() {
        let store = Store::open_memory().unwrap();
        let category = new_category("Temp");
        let id = category.id;
        store.create_category(&category).unwrap();

        store.delete_category(id).unwrap();
        assert!(matches!(
            store.get_category(id),
            Err(AgendaError::NotFound { .. })
        ));
    }

    #[test]
    fn test_delete_category_with_children_rejected() {
        let store = Store::open_memory().unwrap();
        let parent = new_category("Parent");
        store.create_category(&parent).unwrap();

        let mut child = new_category("Child");
        child.parent = Some(parent.id);
        store.create_category(&child).unwrap();

        let result = store.delete_category(parent.id);
        assert!(matches!(result, Err(AgendaError::InvalidOperation { .. })));
    }

    #[test]
    fn test_delete_reserved_category_rejected() {
        let store = Store::open_memory().unwrap();
        let reserved_id = category_id_by_name(&store, RESERVED_CATEGORY_NAME_DONE);

        let result = store.delete_category(reserved_id);
        assert!(matches!(
            result,
            Err(AgendaError::ReservedName { name }) if name == RESERVED_CATEGORY_NAME_DONE
        ));
    }

    #[test]
    fn test_update_reserved_category_allowed_without_rename() {
        let store = Store::open_memory().unwrap();
        let reserved_id = category_id_by_name(&store, RESERVED_CATEGORY_NAME_WHEN);

        let mut category = store.get_category(reserved_id).unwrap();
        category.note = Some("allowed".to_string());
        category.enable_implicit_string = false;
        store.update_category(&category).unwrap();

        let loaded = store.get_category(reserved_id).unwrap();
        assert_eq!(loaded.name, RESERVED_CATEGORY_NAME_WHEN);
        assert_eq!(loaded.note.as_deref(), Some("allowed"));
        assert!(!loaded.enable_implicit_string);
    }

    #[test]
    fn test_create_and_get_view() {
        let store = Store::open_memory().unwrap();
        let when_category = make_category(&store, "WhenColumn");

        let mut view = new_view("Inbox");
        view.criteria
            .set_criterion(CriterionMode::And, when_category);

        let mut section_criteria = Query::default();
        section_criteria.set_criterion(CriterionMode::And, when_category);
        view.sections.push(Section {
            title: "Due Soon".to_string(),
            criteria: section_criteria,
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: when_category,
                width: 18,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::from([when_category]),
            on_remove_unassign: HashSet::new(),
            show_children: true,
            board_display_mode_override: None,
        });
        view.show_unmatched = false;
        view.unmatched_label = "Other".to_string();
        view.remove_from_view_unassign.insert(when_category);
        view.category_aliases = BTreeMap::from([(when_category, "Due".to_string())]);

        store.create_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.id, view.id);
        assert_eq!(loaded.name, "Inbox");
        assert_eq!(loaded.criteria.criteria, view.criteria.criteria);
        assert_eq!(loaded.sections.len(), 1);
        assert_eq!(loaded.sections[0].title, "Due Soon");
        assert!(loaded.sections[0].show_children);
        assert_eq!(loaded.sections[0].columns.len(), 1);
        assert_eq!(loaded.sections[0].columns[0].heading, when_category);
        assert_eq!(loaded.sections[0].columns[0].width, 18);
        assert!(!loaded.show_unmatched);
        assert_eq!(loaded.unmatched_label, "Other");
        assert_eq!(
            loaded.remove_from_view_unassign,
            view.remove_from_view_unassign
        );
        assert_eq!(loaded.category_aliases, view.category_aliases);
    }

    #[test]
    fn test_get_view_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.get_view(Uuid::new_v4());
        assert!(matches!(
            result,
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_create_view_duplicate_name_rejected() {
        let store = Store::open_memory().unwrap();
        let one = new_view("Planning");
        let two = new_view("Planning");
        store.create_view(&one).unwrap();

        let result = store.create_view(&two);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "Planning"
        ));
    }

    #[test]
    fn test_create_view_reserved_system_name_rejected() {
        let store = Store::open_memory().unwrap();
        let result = store.create_view(&new_view("all items"));
        assert!(matches!(
            result,
            Err(AgendaError::InvalidOperation { message })
            if message.contains("cannot create system view")
        ));
    }

    #[test]
    fn test_clone_view_copies_configuration_and_is_independent() {
        let store = Store::open_memory().unwrap();
        let area = make_category(&store, "Area");
        let mut child_category = new_category("CLI");
        child_category.parent = Some(area);
        let child = child_category.id;
        store.create_category(&child_category).unwrap();

        let mut source = new_view("Source");
        source.criteria.set_criterion(CriterionMode::And, area);
        source.show_unmatched = false;
        source.unmatched_label = "Other".to_string();
        source.remove_from_view_unassign = HashSet::from([area]);
        source.category_aliases = BTreeMap::from([(area, "Team".to_string())]);
        source.item_column_label = Some("Task".to_string());
        source.board_display_mode = BoardDisplayMode::MultiLine;
        let mut section_criteria = Query::default();
        section_criteria.set_criterion(CriterionMode::And, child);
        source.sections.push(Section {
            title: "Section One".to_string(),
            criteria: section_criteria,
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area,
                width: 24,
            }],
            item_column_index: 1,
            on_insert_assign: HashSet::from([child]),
            on_remove_unassign: HashSet::from([area]),
            show_children: true,
            board_display_mode_override: Some(BoardDisplayMode::SingleLine),
        });
        store.create_view(&source).unwrap();

        let cloned = store
            .clone_view(source.id, "Source Copy".to_string())
            .expect("clone view");
        assert_ne!(cloned.id, source.id);
        assert_eq!(cloned.name, "Source Copy");
        assert_eq!(cloned.criteria.criteria, source.criteria.criteria);
        assert_eq!(cloned.sections.len(), source.sections.len());
        assert_eq!(cloned.sections[0].title, source.sections[0].title);
        assert_eq!(
            cloned.sections[0].criteria.criteria,
            source.sections[0].criteria.criteria
        );
        assert_eq!(cloned.sections[0].columns.len(), 1);
        assert_eq!(cloned.sections[0].columns[0].heading, area);
        assert_eq!(cloned.sections[0].columns[0].width, 24);
        assert_eq!(cloned.sections[0].item_column_index, 1);
        assert_eq!(
            cloned.sections[0].on_insert_assign,
            source.sections[0].on_insert_assign
        );
        assert_eq!(
            cloned.sections[0].on_remove_unassign,
            source.sections[0].on_remove_unassign
        );
        assert!(cloned.sections[0].show_children);
        assert_eq!(
            cloned.sections[0].board_display_mode_override,
            Some(BoardDisplayMode::SingleLine)
        );
        assert_eq!(cloned.show_unmatched, source.show_unmatched);
        assert_eq!(cloned.unmatched_label, source.unmatched_label);
        assert_eq!(
            cloned.remove_from_view_unassign,
            source.remove_from_view_unassign
        );
        assert_eq!(cloned.category_aliases, source.category_aliases);
        assert_eq!(cloned.item_column_label, source.item_column_label);
        assert_eq!(cloned.board_display_mode, source.board_display_mode);

        let mut edited_clone = store.get_view(cloned.id).expect("load clone");
        edited_clone.unmatched_label = "Changed".to_string();
        store.update_view(&edited_clone).expect("update clone");
        let reloaded_source = store.get_view(source.id).expect("reload source");
        assert_eq!(reloaded_source.unmatched_label, "Other");
    }

    #[test]
    fn test_clone_view_rejects_reserved_target_name() {
        let store = Store::open_memory().unwrap();
        let source = new_view("Source");
        store.create_view(&source).unwrap();

        let result = store.clone_view(source.id, "All Items".to_string());
        assert!(matches!(
            result,
            Err(AgendaError::InvalidOperation { message })
            if message.contains("cannot create system view")
        ));
    }

    #[test]
    fn test_update_view() {
        let store = Store::open_memory().unwrap();
        let mut view = new_view("Daily");
        store.create_view(&view).unwrap();

        let category_id = make_category(&store, "Schedule");
        view.name = "Daily Agenda".to_string();
        view.criteria.set_criterion(CriterionMode::And, category_id);
        view.sections.push(Section {
            title: "Today".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::from([category_id]),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        view.show_unmatched = false;
        view.unmatched_label = "Unsectioned".to_string();
        view.remove_from_view_unassign.insert(category_id);
        view.category_aliases = BTreeMap::from([(category_id, "Today".to_string())]);

        store.update_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.name, "Daily Agenda");
        assert!(loaded
            .criteria
            .and_category_ids()
            .any(|id| id == category_id));
        assert_eq!(loaded.sections.len(), 1);
        assert!(!loaded.show_unmatched);
        assert_eq!(loaded.unmatched_label, "Unsectioned");
        assert_eq!(
            loaded.remove_from_view_unassign,
            HashSet::from([category_id])
        );
        assert_eq!(
            loaded.category_aliases,
            BTreeMap::from([(category_id, "Today".to_string())])
        );
    }

    #[test]
    fn test_update_view_not_found() {
        let store = Store::open_memory().unwrap();
        let missing = new_view("Missing");
        let result = store.update_view(&missing);
        assert!(matches!(
            result,
            Err(AgendaError::NotFound {
                entity: "View",
                id
            }) if id == missing.id
        ));
    }

    #[test]
    fn test_update_default_view_rejected() {
        let store = Store::open_memory().unwrap();
        let mut default_view = store
            .list_views()
            .unwrap()
            .into_iter()
            .find(|view| view.name.eq_ignore_ascii_case("All Items"))
            .expect("default view exists");
        default_view.unmatched_label = "Custom".to_string();

        let result = store.update_view(&default_view);
        assert!(matches!(
            result,
            Err(AgendaError::InvalidOperation { message })
            if message.contains("cannot modify system view")
        ));
    }

    #[test]
    fn test_update_view_rename_to_system_name_rejected() {
        let store = Store::open_memory().unwrap();
        let mut view = new_view("Daily");
        store.create_view(&view).unwrap();
        view.name = "all items".to_string();

        let result = store.update_view(&view);
        assert!(matches!(
            result,
            Err(AgendaError::InvalidOperation { message })
            if message.contains("reserved system view name")
        ));
    }

    #[test]
    fn test_view_board_display_mode_roundtrip_and_section_override() {
        let store = Store::open_memory().unwrap();
        let mut view = new_view("Display");
        view.board_display_mode = BoardDisplayMode::MultiLine;
        view.sections.push(Section {
            title: "One".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 1,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: Some(BoardDisplayMode::SingleLine),
        });
        store.create_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.board_display_mode, BoardDisplayMode::MultiLine);
        assert_eq!(
            loaded.sections[0].board_display_mode_override,
            Some(BoardDisplayMode::SingleLine)
        );
        assert_eq!(
            loaded.sections[0].item_column_index, 1,
            "roundtrips section field"
        );
    }

    #[test]
    fn test_sections_json_without_display_override_defaults_to_none() {
        let legacy_json = r#"[{"title":"Legacy","criteria":{},"columns":[],"on_insert_assign":[],"on_remove_unassign":[],"show_children":false}]"#;
        let sections: Vec<Section> = serde_json::from_str(legacy_json).expect("legacy json parses");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].item_column_index, 0);
        assert_eq!(sections[0].board_display_mode_override, None);
    }

    #[test]
    fn test_list_views_ordered_by_name_case_insensitive() {
        let store = Store::open_memory().unwrap();
        store.create_view(&new_view("zeta")).unwrap();
        store.create_view(&new_view("Alpha")).unwrap();
        store.create_view(&new_view("beta")).unwrap();

        let views = store.list_views().unwrap();
        let names: Vec<String> = views.into_iter().map(|v| v.name).collect();
        assert_eq!(names, vec!["All Items", "Alpha", "beta", "zeta"]);
    }

    #[test]
    fn test_delete_view() {
        let store = Store::open_memory().unwrap();
        let view = new_view("Temp");
        let id = view.id;
        store.create_view(&view).unwrap();

        store.delete_view(id).unwrap();
        assert!(matches!(
            store.get_view(id),
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_delete_view_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.delete_view(Uuid::new_v4());
        assert!(matches!(
            result,
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_delete_default_view_rejected() {
        let store = Store::open_memory().unwrap();
        let default_id = store
            .list_views()
            .unwrap()
            .into_iter()
            .find(|view| view.name.eq_ignore_ascii_case("All Items"))
            .expect("default view exists")
            .id;

        let result = store.delete_view(default_id);
        assert!(matches!(
            result,
            Err(AgendaError::InvalidOperation { message })
            if message.contains("cannot modify system view")
        ));
    }

    #[test]
    fn test_get_hierarchy_returns_depth_first_with_children() {
        let store = Store::open_memory().unwrap();
        let root_a = new_category("RootA");
        let root_b = new_category("RootB");
        store.create_category(&root_a).unwrap();
        store.create_category(&root_b).unwrap();

        let mut child_a = new_category("ChildA");
        child_a.parent = Some(root_a.id);
        store.create_category(&child_a).unwrap();

        let mut grandchild = new_category("Grandchild");
        grandchild.parent = Some(child_a.id);
        store.create_category(&grandchild).unwrap();

        let hierarchy = store.get_hierarchy().unwrap();
        let root_a_pos = hierarchy.iter().position(|c| c.id == root_a.id).unwrap();
        let child_a_pos = hierarchy.iter().position(|c| c.id == child_a.id).unwrap();
        let grandchild_pos = hierarchy
            .iter()
            .position(|c| c.id == grandchild.id)
            .unwrap();
        let root_b_pos = hierarchy.iter().position(|c| c.id == root_b.id).unwrap();

        assert!(root_a_pos < child_a_pos);
        assert!(child_a_pos < grandchild_pos);
        assert!(grandchild_pos < root_b_pos);

        let loaded_root_a = hierarchy.iter().find(|c| c.id == root_a.id).unwrap();
        assert_eq!(loaded_root_a.children, vec![child_a.id]);

        let loaded_child_a = hierarchy.iter().find(|c| c.id == child_a.id).unwrap();
        assert_eq!(loaded_child_a.children, vec![grandchild.id]);
    }

    #[test]
    fn resolve_item_prefix_unique_match() {
        let store = Store::open(":memory:").unwrap();
        let id = make_item(&store, "test item");
        let prefix = &id.to_string()[..8];
        let resolved = store.resolve_item_prefix(prefix).unwrap();
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_item_prefix_full_hex_no_hyphens() {
        let store = Store::open(":memory:").unwrap();
        let id = make_item(&store, "test item");
        let full_hex = id.to_string().replace('-', "");
        let resolved = store.resolve_item_prefix(&full_hex).unwrap();
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_item_prefix_case_insensitive() {
        let store = Store::open(":memory:").unwrap();
        let id = make_item(&store, "test item");
        let prefix = id.to_string()[..8].to_uppercase();
        let resolved = store.resolve_item_prefix(&prefix).unwrap();
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_item_prefix_no_match() {
        let store = Store::open(":memory:").unwrap();
        make_item(&store, "test item");
        let result = store.resolve_item_prefix("00000000");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no item found"), "got: {msg}");
    }

    #[test]
    fn resolve_item_prefix_ambiguous() {
        let store = Store::open(":memory:").unwrap();
        for i in 0..50 {
            make_item(&store, &format!("item {i}"));
        }
        let items = store.list_items().unwrap();
        let first_char = items[0].id.to_string().chars().next().unwrap();
        let matching: Vec<_> = items
            .iter()
            .filter(|it| it.id.to_string().starts_with(first_char))
            .collect();
        if matching.len() >= 2 {
            let result = store.resolve_item_prefix(&first_char.to_string());
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                matches!(err, AgendaError::AmbiguousId { .. }),
                "expected AmbiguousId, got: {err}"
            );
        }
    }

    #[test]
    fn resolve_item_prefix_invalid_hex() {
        let store = Store::open(":memory:").unwrap();
        let result = store.resolve_item_prefix("zzzz");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("invalid item id prefix"), "got: {msg}");
    }

    #[test]
    fn resolve_item_prefix_empty() {
        let store = Store::open(":memory:").unwrap();
        let result = store.resolve_item_prefix("");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("empty item id prefix"), "got: {msg}");
    }

    // ── column_exists ─────────────────────────────────────────────────────────

    #[test]
    fn column_exists_returns_true_for_existing_column() {
        let store = Store::open_memory().unwrap();
        assert!(
            store.column_exists("categories", "is_actionable").unwrap(),
            "is_actionable should exist on categories table"
        );
    }

    #[test]
    fn column_exists_returns_false_for_nonexistent_column() {
        let store = Store::open_memory().unwrap();
        assert!(
            !store.column_exists("categories", "does_not_exist").unwrap(),
            "does_not_exist should not be present"
        );
    }

    // ── v3 columns_json kind migration ────────────────────────────────────────

    #[test]
    fn upgrade_from_v2_injects_kind_into_existing_columns_json() {
        // Build a database that looks like a v2 store: all current tables exist
        // (SCHEMA_SQL is idempotent), but the views already have columns_json
        // rows without a "kind" field.  After init() the migration must inject
        // "kind": "When" for columns whose heading matches the When category ID
        // and "kind": "Standard" for all others.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();

        // Insert the When category so the migration can identify it.
        let when_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO categories
             (id, name, is_exclusive, is_actionable, enable_implicit_string,
              conditions_json, actions_json, sort_order, created_at, modified_at,
              value_kind, numeric_format_json)
             VALUES (?1, 'When', 0, 0, 0, '[]', '[]', 0,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z',
                     'Tag', 'null')",
            params![when_id.to_string()],
        )
        .unwrap();

        // Insert a view whose columns_json has entries without a "kind" field.
        // First column heading matches the When category ID; second does not.
        let view_id = Uuid::new_v4();
        let columns_without_kind = serde_json::json!([
            {"heading": when_id.to_string()},
            {"heading": "SomeStandardHeading"}
        ])
        .to_string();

        conn.execute(
            "INSERT INTO views
             (id, name, criteria_json, sections_json, columns_json,
              show_unmatched, unmatched_label, remove_from_view_unassign_json,
              category_aliases_json, board_display_mode)
             VALUES (?1, 'TestView', '{}', '[]', ?2, 1, 'Unassigned', '[]', '{}',
                     '\"SingleLine\"')",
            params![view_id.to_string(), columns_without_kind],
        )
        .unwrap();

        // Stamp as v2 so init() will call apply_migrations(2).
        conn.pragma_update(None, "user_version", 2).unwrap();

        let store = Store { conn };
        store.init().unwrap();

        // Read back raw columns_json from the DB.
        let raw: String = store
            .conn
            .query_row(
                "SELECT columns_json FROM views WHERE id = ?1",
                params![view_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();

        let columns: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(columns.len(), 2);

        let kind0 = columns[0]
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let kind1 = columns[1]
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        assert_eq!(
            kind0, "When",
            "column whose heading matches When category id should get kind=When"
        );
        assert_eq!(
            kind1, "Standard",
            "column with unrecognised heading should get kind=Standard"
        );
    }

    #[test]
    fn upgrade_from_v2_skips_columns_that_already_have_kind() {
        // If a column already has a "kind" field (e.g. from a partial migration),
        // the migration must leave it unchanged.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();

        let view_id = Uuid::new_v4();
        let already_has_kind = serde_json::json!([
            {"heading": "SomeHeading", "kind": "Standard"}
        ])
        .to_string();

        conn.execute(
            "INSERT INTO views
             (id, name, criteria_json, sections_json, columns_json,
              show_unmatched, unmatched_label, remove_from_view_unassign_json,
              category_aliases_json, board_display_mode)
             VALUES (?1, 'PreMigrated', '{}', '[]', ?2, 1, 'Unassigned', '[]', '{}',
                     '\"SingleLine\"')",
            params![view_id.to_string(), already_has_kind],
        )
        .unwrap();

        conn.pragma_update(None, "user_version", 2).unwrap();

        let store = Store { conn };
        store.init().unwrap();

        let raw: String = store
            .conn
            .query_row(
                "SELECT columns_json FROM views WHERE id = ?1",
                params![view_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();

        let columns: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(columns.len(), 1);
        // kind must still be "Standard" — not duplicated or overwritten.
        let kind = columns[0]
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(kind, "Standard");
    }
