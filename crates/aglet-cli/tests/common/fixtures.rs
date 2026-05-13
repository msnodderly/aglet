//! Fixture helpers for building up test databases via the real CLI.
//!
//! Each helper wraps one `aglet` invocation and asserts success, so test
//! bodies stay focused on the behavior under test.

#![allow(dead_code)]

use super::{extract_created_id, AgletEnv};

/// Create an item via `aglet add <text>` and return its UUID.
pub fn create_item(env: &AgletEnv, text: &str) -> String {
    let run = env.run(["add", text]);
    run.assert_success();
    extract_created_id(&run.stdout)
}

/// Create an item with note + category assignments.
pub fn create_item_full(
    env: &AgletEnv,
    text: &str,
    note: Option<&str>,
    categories: &[&str],
) -> String {
    let mut args: Vec<String> = vec!["add".into(), text.into()];
    if let Some(note) = note {
        args.push("--note".into());
        args.push(note.into());
    }
    for cat in categories {
        args.push("--category".into());
        args.push((*cat).into());
    }
    let run = env.run(args);
    run.assert_success();
    extract_created_id(&run.stdout)
}

/// Create a top-level tag category.
pub fn create_category(env: &AgletEnv, name: &str) {
    env.run(["category", "create", name]).assert_success();
}

/// Create an exclusive parent category (child siblings are mutually exclusive).
pub fn create_exclusive_category(env: &AgletEnv, name: &str) {
    env.run(["category", "create", name, "--exclusive"])
        .assert_success();
}

/// Create a child category under `parent`.
pub fn create_child_category(env: &AgletEnv, parent: &str, name: &str) {
    env.run(["category", "create", name, "--parent", parent])
        .assert_success();
}

/// Assign `category` to `item_id` (full or short UUID).
pub fn assign_category(env: &AgletEnv, item_id: &str, category: &str) {
    env.run(["category", "assign", item_id, category])
        .assert_success();
}

/// Create a view with optional `--include` filters.
pub fn create_view(env: &AgletEnv, name: &str, includes: &[&str]) {
    let mut args: Vec<String> = vec!["view".into(), "create".into(), name.into()];
    for inc in includes {
        args.push("--include".into());
        args.push((*inc).into());
    }
    env.run(args).assert_success();
}

/// Add a `depends-on` link: `from` depends on `to` (so `to` blocks `from`).
pub fn link_depends_on(env: &AgletEnv, from: &str, to: &str) {
    env.run(["link", "depends-on", from, to]).assert_success();
}
