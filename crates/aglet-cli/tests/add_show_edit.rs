//! End-to-end coverage for `aglet add`, `aglet show`, and `aglet edit`.
//!
//! Backed by the `common` harness which spawns the real `aglet` binary
//! against a temp `.ag` database per test.
//!
//! Tracks the checklist on bugtracker item `b36b6305`.

mod common;

use common::fixtures::{
    assign_category, create_category, create_child_category, create_exclusive_category, create_item,
};
use common::{snapshot_filters, AgletEnv};

#[test]
fn add_rejects_empty_text() {
    let env = AgletEnv::new();
    let run = env.run(["add", ""]);
    run.assert_failure();
    assert!(
        run.stderr.contains("text cannot be empty"),
        "stderr should explain the rejection, got: {}",
        run.stderr
    );
}

#[test]
fn add_rejects_whitespace_text() {
    let env = AgletEnv::new();
    let run = env.run(["add", "   \t  "]);
    run.assert_failure();
    assert!(
        run.stderr.contains("text cannot be empty"),
        "stderr should explain the rejection, got: {}",
        run.stderr
    );
}

#[test]
fn add_basic_prints_created_uuid() {
    let env = AgletEnv::new();
    let run = env.run(["add", "Buy milk"]);
    run.assert_success();

    insta::with_settings!({ filters => snapshot_filters() }, {
        insta::assert_snapshot!(run.stdout, @"created [uuid]
    ");
    });
}

#[test]
fn add_with_note_when_and_category_persists() {
    let env = AgletEnv::new();
    create_category(&env, "Urgent");

    let add_run = env.run([
        "add",
        "Buy milk",
        "--note",
        "Two percent, organic",
        "--when",
        "2026-06-01",
        "--category",
        "Urgent",
    ]);
    add_run.assert_success();

    insta::with_settings!({ filters => snapshot_filters() }, {
        insta::assert_snapshot!("add_with_note_when_and_category_persists__add_stdout", add_run.stdout);
    });

    let item_id = common::extract_created_id(&add_run.stdout);
    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();

    insta::with_settings!({ filters => snapshot_filters() }, {
        insta::assert_snapshot!("add_with_note_when_and_category_persists__show_stdout", show_run.stdout);
    });
}

#[test]
fn add_natural_language_when_parses() {
    let env = AgletEnv::new();
    let run = env.run(["add", "Tomorrow's task", "--when", "tomorrow"]);
    run.assert_success();
    assert!(
        run.stdout.contains("parsed_when="),
        "expected parsed_when feedback, got: {}",
        run.stdout
    );
}

#[test]
fn add_with_numeric_value_persists() {
    let env = AgletEnv::new();
    env.run(["category", "create", "Hours", "--type", "numeric"])
        .assert_success();

    let add_run = env.run(["add", "Work session", "--value", "Hours=2.5"]);
    add_run.assert_success();

    let item_id = common::extract_created_id(&add_run.stdout);
    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();

    insta::with_settings!({ filters => snapshot_filters() }, {
        insta::assert_snapshot!(show_run.stdout);
    });
}

#[test]
fn add_unknown_hashtag_feedback_appears_only_when_relevant() {
    let env = AgletEnv::new();
    let with_unknown = env.run(["add", "Has #unknownhashtag in body"]);
    with_unknown.assert_success();
    assert!(
        with_unknown.stdout.contains("unknown_hashtags="),
        "expected unknown_hashtags warning, got: {}",
        with_unknown.stdout
    );

    let without_hashtag = env.run(["add", "No hashtag here"]);
    without_hashtag.assert_success();
    assert!(
        !without_hashtag.stdout.contains("unknown_hashtags="),
        "did not expect unknown_hashtags warning, got: {}",
        without_hashtag.stdout
    );
}

#[test]
fn show_renders_provenance_for_subsumed_categories() {
    let env = AgletEnv::new();
    create_exclusive_category(&env, "Priority");
    create_child_category(&env, "Priority", "High");

    let item_id = create_item(&env, "Important task");
    assign_category(&env, &item_id, "High");

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();

    insta::with_settings!({ filters => snapshot_filters() }, {
        insta::assert_snapshot!(show_run.stdout);
    });
}

#[test]
fn edit_text_via_positional_argument() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Original text");

    env.run(["edit", &item_id, "Replaced text"])
        .assert_success();

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();
    assert!(
        show_run.stdout.contains("text:       Replaced text"),
        "expected new text in show output, got:\n{}",
        show_run.stdout
    );
    assert!(
        !show_run.stdout.contains("text:       Original text"),
        "old text should be gone, got:\n{}",
        show_run.stdout
    );
}

#[test]
fn edit_note_replace_overwrites_full_note() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Task");
    env.run(["edit", &item_id, "--note", "First note"])
        .assert_success();
    env.run(["edit", &item_id, "--note", "Replacement note"])
        .assert_success();

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();
    assert!(
        show_run.stdout.contains("note:       Replacement note"),
        "note should be replaced, got:\n{}",
        show_run.stdout
    );
    assert!(
        !show_run.stdout.contains("First note"),
        "old note text should be gone, got:\n{}",
        show_run.stdout
    );
}

#[test]
fn edit_append_note_concatenates_with_newline() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Task");
    env.run(["edit", &item_id, "--note", "Line one"])
        .assert_success();
    env.run(["edit", &item_id, "--append-note", "Line two"])
        .assert_success();

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();
    assert!(
        show_run.stdout.contains("Line one"),
        "first line should remain: {}",
        show_run.stdout
    );
    assert!(
        show_run.stdout.contains("Line two"),
        "appended line should be present: {}",
        show_run.stdout
    );
}

#[test]
fn edit_note_stdin_replaces_note_from_stdin() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Task");

    let assert = env
        .cmd()
        .args(["edit", &item_id, "--note-stdin"])
        .write_stdin("first\nsecond\n")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("updated"), "got: {}", stdout);

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();
    assert!(
        show_run.stdout.contains("first") && show_run.stdout.contains("second"),
        "stdin lines should be in note, got:\n{}",
        show_run.stdout
    );
}

#[test]
fn edit_clear_note_removes_note_entirely() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Task");
    env.run(["edit", &item_id, "--note", "Will be cleared"])
        .assert_success();
    env.run(["edit", &item_id, "--clear-note"]).assert_success();

    let show_run = env.run(["show", &item_id]);
    show_run.assert_success();
    assert!(
        !show_run.stdout.contains("Will be cleared"),
        "old note text should be gone, got:\n{}",
        show_run.stdout
    );
}

#[test]
fn edit_done_toggles_status() {
    let env = AgletEnv::new();
    create_category(&env, "Inbox");
    let item_id = create_item(&env, "Task to complete");
    assign_category(&env, &item_id, "Inbox");

    let mark_done = env.run(["edit", &item_id, "--done", "true"]);
    mark_done.assert_success();
    assert!(
        mark_done.stdout.contains("marked done"),
        "stdout should report done, got: {}",
        mark_done.stdout
    );

    let show_after_done = env.run(["show", &item_id]);
    show_after_done.assert_success();
    assert!(
        show_after_done.stdout.contains("status:     done"),
        "show should report done status, got:\n{}",
        show_after_done.stdout
    );

    let mark_undone = env.run(["edit", &item_id, "--done", "false"]);
    mark_undone.assert_success();
    assert!(
        mark_undone.stdout.contains("marked not-done"),
        "stdout should report not-done, got: {}",
        mark_undone.stdout
    );

    let show_after_undone = env.run(["show", &item_id]);
    show_after_undone.assert_success();
    assert!(
        show_after_undone.stdout.contains("status:     open"),
        "show should report open status, got:\n{}",
        show_after_undone.stdout
    );
}

#[test]
fn edit_when_then_clear_when() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Task with date");

    env.run(["edit", &item_id, "--when", "2026-07-15"])
        .assert_success();
    let show_with = env.run(["show", &item_id]);
    show_with.assert_success();
    assert!(
        show_with.stdout.contains("when:       2026-07-15T00:00:00"),
        "show should display set when, got:\n{}",
        show_with.stdout
    );

    env.run(["edit", &item_id, "--clear-when"]).assert_success();
    let show_cleared = env.run(["show", &item_id]);
    show_cleared.assert_success();
    assert!(
        show_cleared.stdout.contains("when:       -"),
        "show should display cleared when, got:\n{}",
        show_cleared.stdout
    );
}

#[test]
fn edit_recurrence_then_clear_recurrence() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Daily task");
    env.run(["edit", &item_id, "--when", "2026-07-15"])
        .assert_success();

    env.run(["edit", &item_id, "--recurrence", "daily"])
        .assert_success();
    let show_with = env.run(["show", &item_id]);
    show_with.assert_success();
    assert!(
        show_with.stdout.to_lowercase().contains("daily")
            || show_with.stdout.contains("recurrence"),
        "show should display recurrence, got:\n{}",
        show_with.stdout
    );

    env.run(["edit", &item_id, "--clear-recurrence"])
        .assert_success();
    let show_cleared = env.run(["show", &item_id]);
    show_cleared.assert_success();
    assert!(
        !show_cleared
            .stdout
            .to_lowercase()
            .contains("recurrence: daily"),
        "recurrence should be cleared, got:\n{}",
        show_cleared.stdout
    );
}

#[test]
fn short_uuid_prefix_works_for_show_and_edit() {
    let env = AgletEnv::new();
    let item_id = create_item(&env, "Prefix lookup test");
    let prefix: String = item_id.chars().take(8).collect();

    let show_run = env.run(["show", &prefix]);
    show_run.assert_success();
    assert!(
        show_run.stdout.contains(&item_id),
        "show by prefix should resolve to the full UUID, got:\n{}",
        show_run.stdout
    );

    env.run(["edit", &prefix, "Updated via prefix"])
        .assert_success();
    let after = env.run(["show", &item_id]);
    after.assert_success();
    assert!(
        after.stdout.contains("text:       Updated via prefix"),
        "edit by prefix should update text, got:\n{}",
        after.stdout
    );
}

#[test]
fn edit_unknown_item_fails_with_nonzero_exit() {
    let env = AgletEnv::new();
    let run = env.run(["edit", "deadbeef", "--note", "x"]);
    run.assert_failure();
    let lower = run.stderr.to_lowercase();
    assert!(
        lower.contains("invalid item id")
            || lower.contains("no item found")
            || lower.contains("not found"),
        "stderr should explain missing item, got: {}",
        run.stderr
    );
}
