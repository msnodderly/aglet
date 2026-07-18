    use super::{
        blocked_item_ids, build_markdown_export, build_numeric_filters, category_name_map, cmd_add,
        cmd_category, cmd_claim, cmd_edit, cmd_import, cmd_link, cmd_list, cmd_release, cmd_unlink,
        cmd_view, compare_items_by_sort_keys, describe_category_action,
        duplicate_category_create_error, indexed_category_action_row, item_link_section_lines,
        parse_csv_decimals, parse_decimal_value, parse_sort_spec, parse_when_datetime_input,
        parsed_when_feedback_line, read_note_from_stdin, reject_items_with_any_categories,
        render_compact_item_table, render_section_column_table, retain_items_by_dependency_state,
        retain_items_matching_numeric_filters, retain_items_with_all_categories,
        retain_items_with_any_categories, section_summary_entries, section_summary_line,
        tui_launch_debug, unknown_hashtag_feedback_line, view_by_name, view_category_alias_rows,
        write_output_allow_broken_pipe, write_stdout_allow_broken_pipe, CategoryCommand, Cli,
        CliColumnKind, CliSortDirection, CliSortField, CliSortKey, CliSummaryFn, Command,
        ConditionMatchModeArg, DateSourceArg, ImportCommand, LinkCommand, ListFilters,
        NumericFilter, NumericPredicate, OutputFormatArg, TableStyle, UnlinkCommand,
        ViewAliasCommand, ViewColumnCommand, ViewCommand, ViewSectionCommand,
    };
    use aglet_core::aglet::Aglet;
    use aglet_core::matcher::SubstringClassifier;
    use aglet_core::model::ConditionMatchMode;
    use aglet_core::model::{
        Action, Category, CategoryId, CategoryValueKind, Column, ColumnKind, Condition,
        CriterionMode, DateCompareOp, DateMatcher, DateSource, Item, NumericFormat, Query, Section,
        SummaryFn, View,
    };
    use aglet_core::store::Store;
    use clap::{CommandFactory, Parser};
    use jiff::civil::date;
    use rust_decimal::Decimal;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::io::{self, Cursor, Write};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    fn assert_help_docs_for_command_tree(cmd: &clap::Command) {
        if cmd.get_name() != "aglet" {
            let about = cmd
                .get_about()
                .or(cmd.get_long_about())
                .map(|value| value.to_string())
                .unwrap_or_default();
            assert!(
                !about.trim().is_empty(),
                "command '{}' is missing help/description text",
                cmd.get_name()
            );
        }

        for arg in cmd.get_arguments() {
            if arg.get_id().as_str() == "help" {
                continue;
            }
            let help = arg
                .get_help()
                .or(arg.get_long_help())
                .map(|value| value.to_string())
                .unwrap_or_default();
            assert!(
                !help.trim().is_empty(),
                "argument '{}' on command '{}' is missing help text",
                arg.get_id(),
                cmd.get_name()
            );
        }

        for subcommand in cmd.get_subcommands() {
            assert_help_docs_for_command_tree(subcommand);
        }
    }

    #[test]
    fn duplicate_category_error_includes_assign_guidance_and_parent_context() {
        let id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid");
        let msg = duplicate_category_create_error("Priority", Some("Project X"), Some(id));
        assert!(msg.contains("already exists"));
        assert!(msg.contains("Category names are global"));
        assert!(msg.contains("under parent \"Project X\""));
        assert!(msg.contains("aglet category assign <item-id> \"Priority\""));
        assert!(msg.contains("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[test]
    fn parsed_when_feedback_line_includes_datetime_when_present() {
        let when = date(2026, 2, 24).at(15, 0, 0, 0);

        let line = parsed_when_feedback_line(Some(when)).expect("expected line");
        assert_eq!(line, "parsed_when=2026-02-24T15:00:00");
    }

    #[test]
    fn parsed_when_feedback_line_omits_output_when_absent() {
        assert_eq!(parsed_when_feedback_line(None), None);
    }

    #[test]
    fn parse_when_datetime_input_supports_date_only_at_midnight() {
        let parsed = parse_when_datetime_input("2026-02-20").expect("parse date-only");
        assert_eq!(parsed, date(2026, 2, 20).at(0, 0, 0, 0));
    }

    #[test]
    fn clap_parses_add_with_when_categories_and_values() {
        let cli = Cli::try_parse_from([
            "aglet",
            "add",
            "DRZ Payment",
            "--when",
            "2026-02-20",
            "--category",
            "Budget",
            "--category",
            "Vendor",
            "--value",
            "Cost=245.96",
        ])
        .expect("parse cli");

        match cli.command {
            Some(Command::Add {
                text,
                when,
                categories,
                values,
                ..
            }) => {
                assert_eq!(text, "DRZ Payment");
                assert_eq!(when.as_deref(), Some("2026-02-20"));
                assert_eq!(categories, vec!["Budget".to_string(), "Vendor".to_string()]);
                assert_eq!(values, vec!["Cost=245.96".to_string()]);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_root_command_is_aglet() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_name(), "aglet");
    }

    #[test]
    fn no_subcommand_launches_tui_without_debug() {
        let cli = Cli::try_parse_from(["aglet"]).expect("parse CLI");
        assert!(cli.command.is_none());
        assert_eq!(tui_launch_debug(&cli.command), Some(false));
    }

    #[test]
    fn clap_parses_tui_with_debug() {
        let cli = Cli::try_parse_from(["aglet", "tui", "--debug"]).expect("parse CLI");

        match cli.command {
            Some(Command::Tui { debug }) => assert!(debug),
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn cmd_add_assigns_when_categories_and_numeric_values() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let budget = Category::new("Budget".to_string());
        let vendor = Category::new("Vendor".to_string());
        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&budget).expect("create budget");
        store.create_category(&vendor).expect("create vendor");
        store.create_category(&cost).expect("create cost");

        cmd_add(
            &aglet,
            "DRZ Payment".to_string(),
            Some("monthly payment".to_string()),
            Some("2026-02-20".to_string()),
            vec!["Budget".to_string(), "Vendor".to_string()],
            vec!["Cost=245.96".to_string()],
        )
        .expect("add item");

        let items = store.list_items().expect("list items");
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.text, "DRZ Payment");
        assert_eq!(item.when_date, Some(date(2026, 2, 20).at(0, 0, 0, 0)));
        assert!(item.assignments.contains_key(&budget.id));
        assert!(item.assignments.contains_key(&vendor.id));
        assert_eq!(
            item.assignments
                .get(&cost.id)
                .and_then(|assignment| assignment.numeric_value),
            Some(Decimal::new(24596, 2))
        );
    }

    #[test]
    fn cmd_edit_sets_and_clears_when() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create item");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            None,
            None,
            false,
            None,
            Some("2026-03-01".to_string()),
            false,
            None,
            false,
        )
        .expect("set when");
        let updated = store.get_item(item.id).expect("load item");
        assert_eq!(updated.when_date, Some(date(2026, 3, 1).at(0, 0, 0, 0)));

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            true,
            None,
            false,
        )
        .expect("clear when");
        let cleared = store.get_item(item.id).expect("load cleared item");
        assert_eq!(cleared.when_date, None);
    }

    #[test]
    fn unknown_hashtag_feedback_line_includes_unknown_tokens() {
        let line = unknown_hashtag_feedback_line(&["office".to_string(), "someday".to_string()]);
        assert_eq!(
            line.as_deref(),
            Some("warning: unknown_hashtags=office,someday")
        );
    }

    #[test]
    fn unknown_hashtag_feedback_line_omits_when_no_unknown_tokens() {
        assert_eq!(unknown_hashtag_feedback_line(&[]), None);
    }

    #[test]
    fn parse_decimal_value_accepts_commas() {
        assert_eq!(
            parse_decimal_value("1,234.50").unwrap(),
            Decimal::new(123450, 2)
        );
    }

    #[test]
    fn parse_decimal_value_rejects_empty() {
        assert!(parse_decimal_value("   ").is_err());
    }

    #[test]
    fn view_category_alias_rows_sort_and_skip_blank_aliases() {
        let alpha = Uuid::new_v4();
        let beta = Uuid::new_v4();
        let gamma = Uuid::new_v4();

        let mut view = View::new("Aliases".to_string());
        view.category_aliases.insert(alpha, "A".to_string());
        view.category_aliases.insert(beta, "   ".to_string());
        view.category_aliases.insert(gamma, "G".to_string());

        let category_names = HashMap::from([
            (alpha, "Alpha".to_string()),
            (beta, "Beta".to_string()),
            (gamma, "gamma".to_string()),
        ]);

        let rows = view_category_alias_rows(&view, &category_names);
        assert_eq!(rows.len(), 2, "blank aliases are omitted");
        assert_eq!(rows[0].category_name, "Alpha");
        assert_eq!(rows[0].alias, "A");
        assert_eq!(rows[1].category_name, "gamma");
        assert_eq!(rows[1].alias, "G");
    }

    #[test]
    fn view_category_alias_rows_shows_deleted_category_fallback() {
        let missing = Uuid::new_v4();
        let mut view = View::new("Aliases".to_string());
        view.category_aliases
            .insert(missing, "Archived".to_string());

        let rows = view_category_alias_rows(&view, &HashMap::new());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category_name, format!("(deleted:{missing})"));
        assert_eq!(rows[0].alias, "Archived");
    }

    #[test]
    fn clap_parses_claim_with_item_id() {
        let cli = Cli::try_parse_from(["aglet", "claim", "123e4567-e89b-12d3-a456-426614174000"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Claim { item_id }) => {
                assert_eq!(item_id, "123e4567-e89b-12d3-a456-426614174000");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_ready_command() {
        let cli = Cli::try_parse_from(["aglet", "ready", "--sort", "item", "--format", "json"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Ready { sort, format }) => {
                assert_eq!(sort, vec!["item".to_string()]);
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_category_set_condition_mode() {
        let cli = Cli::try_parse_from(["aglet", "category", "set-condition-mode", "Budget", "all"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Category {
                command: CategoryCommand::SetConditionMode { name, mode },
            }) => {
                assert_eq!(name, "Budget");
                assert_eq!(mode, ConditionMatchModeArg::All);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_release_alias() {
        let cli = Cli::try_parse_from(["aglet", "unclaim", "123e4567-e89b-12d3-a456-426614174000"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Release { item_id }) => {
                assert_eq!(item_id, "123e4567-e89b-12d3-a456-426614174000");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_category_add_date_condition() {
        let cli = Cli::try_parse_from([
            "aglet",
            "category",
            "add-date-condition",
            "Overdue",
            "--source",
            "when",
            "--before",
            "today",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::Category {
                command:
                    CategoryCommand::AddDateCondition {
                        name,
                        source,
                        before,
                        on,
                        after,
                        at_or_before,
                        at_or_after,
                        from,
                        through,
                    },
            }) => {
                assert_eq!(name, "Overdue");
                assert_eq!(source, DateSourceArg::When);
                assert_eq!(before.as_deref(), Some("today"));
                assert!(on.is_none());
                assert!(after.is_none());
                assert!(at_or_before.is_none());
                assert!(at_or_after.is_none());
                assert!(from.is_none());
                assert!(through.is_none());
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn cmd_claim_fails_when_item_is_already_claimed() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = Category::new("Ready".to_string());
        store.create_category(&ready).expect("create ready");
        let in_progress = Category::new("In Progress".to_string());
        store
            .create_category(&in_progress)
            .expect("create in-progress");
        store
            .set_workflow_config(&aglet_core::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .expect("set workflow");

        let item = Item::new("Claim target".to_string());
        store.create_item(&item).expect("create item");
        aglet
            .assign_item_manual(
                item.id,
                in_progress.id,
                Some("manual:test.assign".to_string()),
            )
            .expect("seed in-progress");

        let err = cmd_claim(&aglet, &store, item.id.to_string()).expect_err("claim should fail");
        assert!(err.contains("already claimed"));
    }

    #[test]
    fn cmd_claim_assigns_claim_category_and_keeps_ready_category() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = Category::new("Ready".to_string());
        store.create_category(&ready).expect("create ready");
        let in_progress = Category::new("In Progress".to_string());
        store
            .create_category(&in_progress)
            .expect("create in-progress");
        store
            .set_workflow_config(&aglet_core::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .expect("set workflow");

        let item = Item::new("Claim target".to_string());
        store.create_item(&item).expect("create item");
        aglet
            .assign_item_manual(item.id, ready.id, Some("manual:test.assign".to_string()))
            .expect("seed ready");

        cmd_claim(&aglet, &store, item.id.to_string()).expect("claim should succeed");

        let assignments = store
            .get_assignments_for_item(item.id)
            .expect("load assignments");
        assert!(assignments.contains_key(&in_progress.id));
        assert!(assignments.contains_key(&ready.id));
    }

    #[test]
    fn cmd_release_removes_claim_category() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = Category::new("Ready".to_string());
        store.create_category(&ready).expect("create ready");
        let in_progress = Category::new("In Progress".to_string());
        store
            .create_category(&in_progress)
            .expect("create in-progress");
        store
            .set_workflow_config(&aglet_core::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .expect("set workflow");

        let item = Item::new("Claim target".to_string());
        store.create_item(&item).expect("create item");
        aglet
            .assign_item_manual(item.id, ready.id, Some("manual:test.assign".to_string()))
            .expect("seed ready");
        aglet.claim_item_workflow(item.id).expect("claim");

        cmd_release(&aglet, &store, item.id.to_string()).expect("release should succeed");

        let assignments = store
            .get_assignments_for_item(item.id)
            .expect("load assignments");
        assert!(!assignments.contains_key(&in_progress.id));
        assert!(assignments.contains_key(&ready.id));
    }

    #[test]
    fn clap_parses_link_depends_on_subcommand() {
        let cli = Cli::try_parse_from([
            "aglet",
            "link",
            "depends-on",
            "123e4567-e89b-12d3-a456-426614174000",
            "123e4567-e89b-12d3-a456-426614174001",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::Link {
                command:
                    LinkCommand::DependsOn {
                        item_id,
                        depends_on_item_id,
                    },
            }) => {
                assert_eq!(item_id, "123e4567-e89b-12d3-a456-426614174000");
                assert_eq!(depends_on_item_id, "123e4567-e89b-12d3-a456-426614174001");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_top_level_unlink_depends_on_subcommand() {
        let cli =
            Cli::try_parse_from(["aglet", "unlink", "depends-on", "a", "b"]).expect("parse CLI");

        match cli.command {
            Some(Command::Unlink {
                command:
                    UnlinkCommand::DependsOn {
                        item_id,
                        depends_on_item_id,
                    },
            }) => {
                assert_eq!(item_id, "a");
                assert_eq!(depends_on_item_id, "b");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_sort_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--sort",
            "item:desc",
            "--sort",
            "Priority:asc",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { sort, .. }) => {
                assert_eq!(
                    sort,
                    vec!["item:desc".to_string(), "Priority:asc".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_category_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--category",
            "Feature request",
            "--category",
            "Ready",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { category, .. }) => {
                assert_eq!(
                    category,
                    vec!["Feature request".to_string(), "Ready".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn list_help_documents_repeated_category_and_semantics() {
        let mut cmd = Cli::command();
        let list_cmd = cmd
            .find_subcommand_mut("list")
            .expect("list subcommand should exist");
        let category_arg = list_cmd
            .get_arguments()
            .find(|arg| arg.get_id().as_str() == "category")
            .expect("list --category argument should exist");
        let help = category_arg
            .get_help()
            .expect("list --category should have help text")
            .to_string();

        assert!(help.contains("repeat for AND"));
        assert!(help.contains("ALL specified categories"));
    }

    #[test]
    fn clap_parses_list_with_repeated_any_category_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--any-category",
            "Aglet",
            "--any-category",
            "NeoNV",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { any_category, .. }) => {
                assert_eq!(any_category, vec!["Aglet".to_string(), "NeoNV".to_string()]);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_exclude_category_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--exclude-category",
            "Complete",
            "--exclude-category",
            "Deferred",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List {
                exclude_category, ..
            }) => {
                assert_eq!(
                    exclude_category,
                    vec!["Complete".to_string(), "Deferred".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_blocked_flag() {
        let cli = Cli::try_parse_from(["aglet", "list", "--blocked"]).expect("parse CLI");

        match cli.command {
            Some(Command::List { blocked, .. }) => assert!(blocked),
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_not_blocked_flag() {
        let cli = Cli::try_parse_from(["aglet", "list", "--not-blocked"]).expect("parse CLI");

        match cli.command {
            Some(Command::List { not_blocked, .. }) => assert!(not_blocked),
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_eq_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--value-eq",
            "Complexity",
            "2",
            "--value-eq",
            "Cost",
            "10",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_eq, .. }) => {
                assert_eq!(
                    value_eq,
                    vec![
                        "Complexity".to_string(),
                        "2".to_string(),
                        "Cost".to_string(),
                        "10".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_in_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--value-in",
            "Complexity",
            "1,2",
            "--value-in",
            "Cost",
            "10,20",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_in, .. }) => {
                assert_eq!(
                    value_in,
                    vec![
                        "Complexity".to_string(),
                        "1,2".to_string(),
                        "Cost".to_string(),
                        "10,20".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_max_flags() {
        let cli = Cli::try_parse_from([
            "aglet",
            "list",
            "--value-max",
            "Complexity",
            "2",
            "--value-max",
            "Cost",
            "100",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_max, .. }) => {
                assert_eq!(
                    value_max,
                    vec![
                        "Complexity".to_string(),
                        "2".to_string(),
                        "Cost".to_string(),
                        "100".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn list_help_includes_numeric_filter_examples() {
        let mut cmd = Cli::command();
        let list_cmd = cmd
            .find_subcommand_mut("list")
            .expect("list subcommand should exist");
        let help = list_cmd.render_help().to_string();
        assert!(help.contains("If `--view` is omitted"));
        assert!(help.contains("Numeric value filter examples:"));
        assert!(help.contains("--value-in Complexity 1,2"));
        assert!(help.contains("--value-max Complexity 2"));
    }

    #[test]
    fn clap_help_docs_cover_all_commands_and_arguments() {
        let cmd = Cli::command();
        assert_help_docs_for_command_tree(&cmd);
    }

    #[test]
    fn clap_parses_view_show_with_sort_flag() {
        let cli = Cli::try_parse_from(["aglet", "view", "show", "All Items", "--sort", "when"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command:
                    ViewCommand::Show {
                        name, sort, format, ..
                    },
            }) => {
                assert_eq!(name, "All Items");
                assert_eq!(sort, vec!["when".to_string()]);
                assert_eq!(format, OutputFormatArg::Table);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_json_format() {
        let cli = Cli::try_parse_from(["aglet", "list", "--format", "json"]).expect("parse CLI");

        match cli.command {
            Some(Command::List { format, .. }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_search_with_json_format() {
        let cli =
            Cli::try_parse_from(["aglet", "search", "foo", "--format", "json"]).expect("parse CLI");

        match cli.command {
            Some(Command::Search { format, .. }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_search_with_blocked_flag() {
        let cli = Cli::try_parse_from(["aglet", "search", "foo", "--blocked"]).expect("parse");

        match cli.command {
            Some(Command::Search { blocked, .. }) => assert!(blocked),
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_show_with_blocked_flag() {
        let cli = Cli::try_parse_from(["aglet", "view", "show", "All Items", "--blocked"])
            .expect("parse");

        match cli.command {
            Some(Command::View {
                command: ViewCommand::Show { blocked, .. },
            }) => assert!(blocked),
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_show_with_json_format() {
        let cli = Cli::try_parse_from(["aglet", "view", "show", "All Items", "--format", "json"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command: ViewCommand::Show { format, .. },
            }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_create_with_hide_dependent_items_flag() {
        let cli =
            Cli::try_parse_from(["aglet", "view", "create", "Focus", "--hide-dependent-items"])
                .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command:
                    ViewCommand::Create {
                        name,
                        hide_unmatched,
                        hide_dependent_items,
                        ..
                    },
            }) => {
                assert_eq!(name, "Focus");
                assert!(!hide_unmatched);
                assert!(hide_dependent_items);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_export_with_view_and_include_links() {
        let cli =
            Cli::try_parse_from(["aglet", "export", "--view", "All Items", "--include-links"])
                .expect("parse CLI");

        match cli.command {
            Some(Command::Export {
                view,
                include_links,
            }) => {
                assert_eq!(view.as_deref(), Some("All Items"));
                assert!(include_links);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_edit_with_hide_dependent_items_option() {
        let cli = Cli::try_parse_from([
            "aglet",
            "view",
            "edit",
            "Focus",
            "--hide-dependent-items",
            "true",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command:
                    ViewCommand::Edit {
                        name,
                        hide_dependent_items,
                        ..
                    },
            }) => {
                assert_eq!(name, "Focus");
                assert_eq!(hide_dependent_items, Some(true));
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_clone_command() {
        let cli =
            Cli::try_parse_from(["aglet", "view", "clone", "Source", "Target"]).expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command:
                    ViewCommand::Clone {
                        source_name,
                        new_name,
                    },
            }) => {
                assert_eq!(source_name, "Source");
                assert_eq!(new_name, "Target");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn export_help_includes_examples_and_link_flag() {
        let mut cmd = Cli::command();
        let export_cmd = cmd
            .find_subcommand_mut("export")
            .expect("export subcommand should exist");
        let help = export_cmd.render_help().to_string();
        assert!(help.contains("--view <VIEW>"));
        assert!(help.contains("--include-links"));
        assert!(help.contains("aglet export --view \"All Items\""));
    }

    struct AlwaysBrokenPipeWriter;

    impl Write for AlwaysBrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_stdout_allow_broken_pipe_treats_broken_pipe_as_success() {
        let mut writer = AlwaysBrokenPipeWriter;
        let result = write_output_allow_broken_pipe(&mut writer, "test");
        assert!(result.is_ok(), "broken pipe should be handled as success");
    }

    struct AlwaysPermissionDeniedWriter;

    impl Write for AlwaysPermissionDeniedWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "permission denied",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_stdout_allow_broken_pipe_preserves_non_broken_pipe_errors() {
        let mut writer = AlwaysPermissionDeniedWriter;
        let err = write_output_allow_broken_pipe(&mut writer, "test")
            .expect_err("non-broken-pipe errors must be returned");
        assert!(
            err.contains("failed writing to stdout"),
            "error should include output context"
        );
    }

    #[test]
    fn write_stdout_allow_broken_pipe_handles_real_stdout_path() {
        let result = write_stdout_allow_broken_pipe("");
        assert!(result.is_ok(), "empty stdout write should succeed");
    }

    #[test]
    fn markdown_export_full_db_is_deterministic_and_includes_metadata() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let topic = Category::new("Topic".to_string());
        store.create_category(&topic).expect("create category");

        let mut beta = Item::new("beta item".to_string());
        beta.note = Some("second note".to_string());
        let mut alpha = Item::new("Alpha item".to_string());
        alpha.note = Some("first note".to_string());
        store.create_item(&beta).expect("create beta");
        store.create_item(&alpha).expect("create alpha");
        aglet
            .assign_item_manual(alpha.id, topic.id, Some("test:assign".to_string()))
            .expect("assign topic");

        let output = build_markdown_export(&store, None, false).expect("export markdown");
        assert!(output.starts_with("# Items\n"));
        assert!(output.contains("- ID: `"));
        assert!(output.contains("- Status: `open`"));
        assert!(output.contains("- Categories: Topic"));
        assert!(output.contains("```text\nfirst note\n```"));

        let alpha_idx = output.find("## Alpha item").expect("alpha section");
        let beta_idx = output.find("## beta item").expect("beta section");
        assert!(alpha_idx < beta_idx, "items should sort by text then id");
    }

    #[test]
    fn markdown_export_view_scope_limits_results() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        store.create_category(&status).expect("create status");
        let mut ready = Category::new("Ready".to_string());
        ready.parent = Some(status.id);
        store.create_category(&ready).expect("create ready");
        let mut deferred = Category::new("Deferred".to_string());
        deferred.parent = Some(status.id);
        store.create_category(&deferred).expect("create deferred");

        let ready_item = Item::new("Ready task".to_string());
        let deferred_item = Item::new("Deferred task".to_string());
        store.create_item(&ready_item).expect("create ready item");
        store
            .create_item(&deferred_item)
            .expect("create deferred item");
        aglet
            .assign_item_manual(ready_item.id, ready.id, Some("test:assign".to_string()))
            .expect("assign ready");
        aglet
            .assign_item_manual(
                deferred_item.id,
                deferred.id,
                Some("test:assign".to_string()),
            )
            .expect("assign deferred");

        let mut view = View::new("Ready Only".to_string());
        view.criteria.set_criterion(CriterionMode::And, ready.id);
        store.create_view(&view).expect("create view");

        let output =
            build_markdown_export(&store, Some("Ready Only"), false).expect("export markdown");
        assert!(output.starts_with("# Ready Only\n"));
        assert!(output.contains("Ready task"));
        assert!(!output.contains("Deferred task"));
    }

    #[test]
    fn markdown_export_include_links_adds_relationship_sections() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");
        aglet
            .link_items_depends_on(a.id, b.id)
            .expect("create dependency");

        let output = build_markdown_export(&store, None, true).expect("export markdown");
        assert!(output.contains("- Links:"));
        assert!(output.contains("prereqs:"));
        assert!(output.contains("Task B"));
    }

    #[test]
    fn cmd_view_clone_copies_source_configuration() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let category = Category::new("Area".to_string());
        store.create_category(&category).expect("create category");

        let mut source = View::new("Planning".to_string());
        source
            .criteria
            .set_criterion(CriterionMode::And, category.id);
        source.show_unmatched = false;
        source.unmatched_label = "Other".to_string();
        source.sections.push(Section {
            title: "Area".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::from([category.id]),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&source).expect("create source");

        cmd_view(
            &aglet,
            &store,
            ViewCommand::Clone {
                source_name: "Planning".to_string(),
                new_name: "Planning Copy".to_string(),
            },
        )
        .expect("clone view");

        let source_after = store.get_view(source.id).expect("source still exists");
        assert_eq!(source_after.name, "Planning");
        let cloned = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|view| view.name == "Planning Copy")
            .expect("clone exists");
        assert_ne!(cloned.id, source.id);
        assert_eq!(cloned.criteria.criteria, source.criteria.criteria);
        assert_eq!(cloned.show_unmatched, source.show_unmatched);
        assert_eq!(cloned.unmatched_label, source.unmatched_label);
        assert_eq!(cloned.sections.len(), source.sections.len());
    }

    #[test]
    fn cmd_view_rename_rejects_all_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let err = cmd_view(
            &aglet,
            &store,
            ViewCommand::Rename {
                name: "All Items".to_string(),
                new_name: "Renamed".to_string(),
            },
        )
        .expect_err("rename should fail");
        assert!(err.contains("cannot modify system view"));
    }

    #[test]
    fn cmd_view_delete_rejects_all_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let err = cmd_view(
            &aglet,
            &store,
            ViewCommand::Delete {
                name: "All Items".to_string(),
            },
        )
        .expect_err("delete should fail");
        assert!(err.contains("cannot modify system view"));
    }

    #[test]
    fn cmd_view_edit_sets_hide_dependent_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let view = View::new("Focus".to_string());
        let view_id = view.id;
        store.create_view(&view).expect("create view");

        cmd_view(
            &aglet,
            &store,
            ViewCommand::Edit {
                name: "Focus".to_string(),
                hide_unmatched: None,
                hide_dependent_items: Some(true),
            },
        )
        .expect("edit view");

        let updated = store.get_view(view_id).expect("load updated view");
        assert!(updated.hide_dependent_items);
    }

    #[test]
    fn section_summary_entries_supports_all_summary_functions() {
        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        let status = Category::new("Status".to_string());

        let mut item_a = Item::new("A".to_string());
        item_a.assignments.insert(
            cost.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Manual,
                assigned_at: jiff::Timestamp::now(),
                sticky: true,
                origin: None,
                explanation: None,
                numeric_value: Some(Decimal::new(100, 0)),
            },
        );
        let mut item_b = Item::new("B".to_string());
        item_b.assignments.insert(
            cost.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Manual,
                assigned_at: jiff::Timestamp::now(),
                sticky: true,
                origin: None,
                explanation: None,
                numeric_value: Some(Decimal::new(250, 0)),
            },
        );
        let item_c = Item::new("C".to_string());
        let items = vec![item_a, item_b, item_c];

        let mut view = View::new("Summary".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Sum),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Avg),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Min),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Max),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Count),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: status.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Sum),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::None),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: None,
                },
            ],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });

        let categories = vec![cost.clone(), status];
        let category_names = HashMap::from([(cost.id, "Cost".to_string())]);
        let entries = section_summary_entries(&view, 0, &items, &categories, &category_names);
        assert_eq!(
            entries,
            vec![
                "Cost(sum)=350".to_string(),
                "Cost(avg)=175".to_string(),
                "Cost(min)=100".to_string(),
                "Cost(max)=250".to_string(),
                "Cost(count)=2".to_string(),
            ]
        );
    }

    #[test]
    fn render_section_column_table_renders_values_totals_and_aliases() {
        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        cost.numeric_format = Some(aglet_core::model::NumericFormat {
            decimal_places: 2,
            currency_symbol: Some("$".to_string()),
            use_thousands_separator: true,
        });
        let dangling_id = CategoryId::new_v4();

        let mut view = View::new("Finance".to_string());
        view.category_aliases.insert(cost.id, "Amount".to_string());
        view.sections.push(Section {
            title: "Bills".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: cost.id,
                    width: 12,
                    summary_fn: Some(SummaryFn::Sum),
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: dangling_id,
                    width: 8,
                    summary_fn: None,
                },
            ],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });

        let mut first = Item::new("Insurance".to_string());
        first.assignments.insert(
            cost.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Manual,
                assigned_at: jiff::Timestamp::now(),
                sticky: true,
                origin: None,
                explanation: None,
                numeric_value: Some(rust_decimal::Decimal::new(123456, 2)),
            },
        );
        let mut second = Item::new("Registration".to_string());
        second.assignments.insert(
            cost.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Manual,
                assigned_at: jiff::Timestamp::now(),
                sticky: true,
                origin: None,
                explanation: None,
                numeric_value: Some(rust_decimal::Decimal::new(50000, 2)),
            },
        );

        let categories = vec![cost.clone()];
        let category_names = category_name_map(&categories);
        let items = vec![first, second];
        let table =
            render_section_column_table(&view, 0, &items, &category_names, &[], &categories)
                .expect("columns configured");

        assert!(
            table.contains("Amount"),
            "alias should be honored in headers: {table}"
        );
        assert!(
            table.contains("(deleted category)"),
            "dangling heading should print (deleted category), not a UUID: {table}"
        );
        assert!(
            table.contains("$1,234.56") && table.contains("$500.00"),
            "numeric cells should use the category's currency format: {table}"
        );
        assert!(
            table.contains("TOTAL") && table.contains("$1,734.56 (sum)"),
            "summary columns should render a formatted TOTAL row: {table}"
        );
        assert!(
            !table.contains(&dangling_id.to_string()),
            "raw UUIDs should not leak into the table: {table}"
        );
    }

    #[test]
    fn render_section_column_table_falls_back_when_no_columns() {
        let mut view = View::new("Plain".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        assert!(
            render_section_column_table(&view, 0, &[], &HashMap::new(), &[], &[]).is_none(),
            "sections without columns keep the generic table"
        );
    }

    #[test]
    fn render_compact_item_table_one_line_rows_with_leaf_categories() {
        let finance = Category::new("Finance".to_string());
        let mut bills = Category::new("Bills".to_string());
        bills.parent = Some(finance.id);
        let mut finance_with_child = finance.clone();
        finance_with_child.children = vec![bills.id];

        let mut item = Item::new("Pay insurance".to_string());
        item.note = Some("policy #123".to_string());
        item.when_date = Some(date(2026, 6, 12).at(0, 0, 0, 0));
        item.assignments.insert(
            bills.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Manual,
                assigned_at: jiff::Timestamp::now(),
                sticky: true,
                origin: None,
                explanation: None,
                numeric_value: None,
            },
        );
        item.assignments.insert(
            finance.id,
            aglet_core::model::Assignment {
                source: aglet_core::model::AssignmentSource::Subsumption,
                assigned_at: jiff::Timestamp::now(),
                sticky: false,
                origin: None,
                explanation: None,
                numeric_value: None,
            },
        );

        let categories = vec![finance_with_child, bills.clone()];
        let category_names = category_name_map(&categories);
        let items = vec![item.clone()];
        let table = render_compact_item_table(&items, &category_names, &[], &categories);
        let lines: Vec<&str> = table.lines().collect();

        assert!(
            lines[0].contains("DONE?") && !lines[0].contains("STATUS"),
            "header should use the honest DONE? column: {table}"
        );
        assert_eq!(
            lines.len(),
            3,
            "header + separator + one row per item (no continuation lines): {table}"
        );
        let row = lines[2];
        let id8: String = item.id.to_string().chars().take(8).collect();
        assert!(
            row.starts_with(&id8),
            "row should lead with the 8-char id: {row}"
        );
        assert!(
            row.contains("2026-06-12") && !row.contains("00:00"),
            "midnight when renders date-only: {row}"
        );
        assert!(
            row.contains("Pay insurance \u{266A}"),
            "note presence shows as a glyph: {row}"
        );
        assert!(
            row.contains("[Bills]") && !row.contains("Finance"),
            "categories list direct leaves only (no subsumed parents): {row}"
        );
    }

    #[test]
    fn render_compact_item_table_empty_prints_no_items() {
        assert_eq!(
            render_compact_item_table(&[], &HashMap::new(), &[], &[]),
            "(no items)\n"
        );
    }

    #[test]
    fn section_summary_line_is_none_when_no_summary_columns_are_configured() {
        let status = Category::new("Status".to_string());
        let mut view = View::new("Summary".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: status.id,
                width: 12,
                summary_fn: None,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });

        let items = vec![Item::new("A".to_string())];
        let categories = vec![status];
        let category_names = HashMap::new();
        assert_eq!(
            section_summary_line(&view, 0, &items, &categories, &category_names),
            None
        );
    }

    #[test]
    fn cmd_view_set_summary_updates_column_summary_fn() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create cost");

        let mut view = View::new("TestView".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: cost.id,
                width: 12,
                summary_fn: None,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        cmd_view(
            &aglet,
            &store,
            ViewCommand::SetSummary {
                name: "TestView".to_string(),
                section: 0,
                column: "Cost".to_string(),
                func: CliSummaryFn::Sum,
            },
        )
        .expect("set-summary should succeed");

        let updated = store.get_view(view.id).expect("get view");
        assert_eq!(
            updated.sections[0].columns[0].summary_fn,
            Some(SummaryFn::Sum)
        );
    }

    #[test]
    fn cmd_view_set_summary_errors_on_missing_column() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut view = View::new("TestView".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let result = cmd_view(
            &aglet,
            &store,
            ViewCommand::SetSummary {
                name: "TestView".to_string(),
                section: 0,
                column: "Nonexistent".to_string(),
                func: CliSummaryFn::Sum,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn cmd_category_format_updates_numeric_category_format() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create cost");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::Format {
                name: "Cost".to_string(),
                decimals: Some(2),
                currency: Some("$".to_string()),
                clear_currency: false,
                thousands: true,
                no_thousands: false,
            },
        )
        .expect("format category");

        let updated = store.get_category(cost.id).expect("load cost");
        assert_eq!(
            updated.numeric_format,
            Some(NumericFormat {
                decimal_places: 2,
                currency_symbol: Some("$".to_string()),
                use_thousands_separator: true,
            })
        );
    }

    #[test]
    fn cmd_category_add_condition_creates_profile_condition() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let urgent = Category::new("Urgent".to_string());
        let project = Category::new("Project Alpha".to_string());
        let escalated = Category::new("Escalated".to_string());
        store.create_category(&urgent).expect("create urgent");
        store.create_category(&project).expect("create project");
        store.create_category(&escalated).expect("create escalated");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddCondition {
                name: "Escalated".to_string(),
                and_categories: vec!["Urgent".to_string(), "Project Alpha".to_string()],
                not_categories: Vec::new(),
                or_categories: Vec::new(),
            },
        )
        .expect("add condition");

        let updated = store.get_category(escalated.id).expect("load escalated");
        assert_eq!(updated.conditions.len(), 1);
        match &updated.conditions[0] {
            Condition::Profile { criteria } => {
                let and_ids: Vec<_> = criteria.and_category_ids().collect();
                assert_eq!(and_ids, vec![urgent.id, project.id]);
                assert_eq!(criteria.not_category_ids().count(), 0);
            }
            other => panic!("expected Profile condition, got {:?}", other),
        }
    }

    #[test]
    fn cmd_category_add_condition_with_not_criteria() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        let delegated = Category::new("Delegated".to_string());
        let my_tasks = Category::new("My-Tasks".to_string());
        store.create_category(&work).expect("create");
        store.create_category(&delegated).expect("create");
        store.create_category(&my_tasks).expect("create");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddCondition {
                name: "My-Tasks".to_string(),
                and_categories: vec!["Work".to_string()],
                not_categories: vec!["Delegated".to_string()],
                or_categories: Vec::new(),
            },
        )
        .expect("add condition");

        let updated = store.get_category(my_tasks.id).expect("load");
        assert_eq!(updated.conditions.len(), 1);
        match &updated.conditions[0] {
            Condition::Profile { criteria } => {
                let and_ids: Vec<_> = criteria.and_category_ids().collect();
                let not_ids: Vec<_> = criteria.not_category_ids().collect();
                assert_eq!(and_ids, vec![work.id]);
                assert_eq!(not_ids, vec![delegated.id]);
            }
            other => panic!("expected Profile condition, got {:?}", other),
        }
    }

    #[test]
    fn cmd_category_add_condition_rejects_empty_criteria() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cat = Category::new("Test".to_string());
        store.create_category(&cat).expect("create");

        let result = cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddCondition {
                name: "Test".to_string(),
                and_categories: Vec::new(),
                not_categories: Vec::new(),
                or_categories: Vec::new(),
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least one criterion"));
    }

    #[test]
    fn cmd_category_add_date_condition_creates_date_condition() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let overdue = Category::new("Overdue".to_string());
        store.create_category(&overdue).expect("create overdue");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddDateCondition {
                name: "Overdue".to_string(),
                source: DateSourceArg::When,
                on: None,
                before: Some("today".to_string()),
                after: None,
                at_or_before: None,
                at_or_after: None,
                from: None,
                through: None,
            },
        )
        .expect("add date condition");

        let updated = store.get_category(overdue.id).expect("load overdue");
        assert_eq!(updated.conditions.len(), 1);
        match &updated.conditions[0] {
            Condition::Date { source, matcher } => {
                assert_eq!(*source, DateSource::When);
                assert_eq!(
                    *matcher,
                    DateMatcher::Compare {
                        op: DateCompareOp::Before,
                        value: aglet_core::model::DateValueExpr::Today,
                    }
                );
            }
            other => panic!("expected Date condition, got {:?}", other),
        }
    }

    #[test]
    fn cmd_category_set_condition_mode_updates_category() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let budget = Category::new("Moto Budget 2025".to_string());
        store.create_category(&budget).expect("create budget");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::SetConditionMode {
                name: "Moto Budget 2025".to_string(),
                mode: ConditionMatchModeArg::All,
            },
        )
        .expect("set condition mode");

        let updated = store.get_category(budget.id).expect("load");
        assert_eq!(updated.condition_match_mode, ConditionMatchMode::All);
    }

    #[test]
    fn cmd_category_remove_condition_removes_by_index() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let urgent = Category::new("Urgent".to_string());
        let p0 = Category::new("P0".to_string());
        let escalated = Category::new("Escalated".to_string());
        store.create_category(&urgent).expect("create");
        store.create_category(&p0).expect("create");
        store.create_category(&escalated).expect("create");

        // Add two conditions
        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddCondition {
                name: "Escalated".to_string(),
                and_categories: vec!["Urgent".to_string()],
                not_categories: Vec::new(),
                or_categories: Vec::new(),
            },
        )
        .expect("add condition 1");
        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddCondition {
                name: "Escalated".to_string(),
                and_categories: vec!["P0".to_string()],
                not_categories: Vec::new(),
                or_categories: Vec::new(),
            },
        )
        .expect("add condition 2");

        let before = store.get_category(escalated.id).expect("load");
        assert_eq!(before.conditions.len(), 2);

        // Remove the first condition (1-based)
        cmd_category(
            &aglet,
            &store,
            CategoryCommand::RemoveCondition {
                name: "Escalated".to_string(),
                index: 1,
            },
        )
        .expect("remove condition");

        let after = store.get_category(escalated.id).expect("load");
        assert_eq!(after.conditions.len(), 1);
        // The remaining condition should be the P0 one
        match &after.conditions[0] {
            Condition::Profile { criteria } => {
                let and_ids: Vec<_> = criteria.and_category_ids().collect();
                assert_eq!(and_ids, vec![p0.id]);
            }
            other => panic!("expected Profile condition, got {:?}", other),
        }
    }

    #[test]
    fn cmd_category_remove_condition_rejects_out_of_range_index() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cat = Category::new("Test".to_string());
        store.create_category(&cat).expect("create");

        let result = cmd_category(
            &aglet,
            &store,
            CategoryCommand::RemoveCondition {
                name: "Test".to_string(),
                index: 1,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of range"));
    }

    #[test]
    fn cmd_category_add_action_creates_assign_action() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = Category::new("Escalated".to_string());
        let notify = Category::new("Notify".to_string());
        store.create_category(&source).expect("create source");
        store.create_category(&notify).expect("create notify");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddAction {
                name: "Escalated".to_string(),
                assign_categories: vec!["Notify".to_string()],
                remove_categories: Vec::new(),
                assign_numeric: None,
                value: None,
                set_when: None,
                mark_done: false,
                delete: false,
            },
        )
        .expect("add action");

        let updated = store.get_category(source.id).expect("load source");
        assert_eq!(updated.actions.len(), 1);
        match &updated.actions[0] {
            Action::Assign { targets } => {
                assert_eq!(targets.len(), 1);
                assert!(targets.contains(&notify.id));
            }
            other => panic!("expected Assign action, got {:?}", other),
        }
    }

    #[test]
    fn describe_category_action_sorts_targets_and_uses_kind_label() {
        let alpha = Category::new("Alpha".to_string());
        let zed = Category::new("Zed".to_string());
        let category_names =
            HashMap::from([(zed.id, zed.name.clone()), (alpha.id, alpha.name.clone())]);
        let action = Action::Assign {
            targets: HashSet::from([zed.id, alpha.id]),
        };

        let desc = describe_category_action(&action, &category_names);

        assert_eq!(desc, "Assign [Alpha, Zed]");
    }

    #[test]
    fn indexed_category_action_row_is_one_based() {
        let notify = Category::new("Notify".to_string());
        let category_names = HashMap::from([(notify.id, notify.name.clone())]);
        let action = Action::Remove {
            targets: HashSet::from([notify.id]),
        };

        let row = indexed_category_action_row(1, &action, &category_names);

        assert_eq!(row, "2. Remove [Notify]");
    }

    #[test]
    fn cmd_category_add_action_rejects_mixed_kinds() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = Category::new("Escalated".to_string());
        let notify = Category::new("Notify".to_string());
        let low = Category::new("Low".to_string());
        store.create_category(&source).expect("create source");
        store.create_category(&notify).expect("create notify");
        store.create_category(&low).expect("create low");

        let result = cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddAction {
                name: "Escalated".to_string(),
                assign_categories: vec!["Notify".to_string()],
                remove_categories: vec!["Low".to_string()],
                assign_numeric: None,
                value: None,
                set_when: None,
                mark_done: false,
                delete: false,
            },
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("specify exactly one action kind"));
    }

    #[test]
    fn cmd_category_add_action_rejects_self_target() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = Category::new("Escalated".to_string());
        store.create_category(&source).expect("create source");

        let result = cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddAction {
                name: "Escalated".to_string(),
                assign_categories: vec!["Escalated".to_string()],
                remove_categories: Vec::new(),
                assign_numeric: None,
                value: None,
                set_when: None,
                mark_done: false,
                delete: false,
            },
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("cannot target itself in an action"));
    }

    #[test]
    fn cmd_category_remove_action_removes_by_index() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = Category::new("Escalated".to_string());
        let notify = Category::new("Notify".to_string());
        let low = Category::new("Low".to_string());
        store.create_category(&source).expect("create source");
        store.create_category(&notify).expect("create notify");
        store.create_category(&low).expect("create low");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddAction {
                name: "Escalated".to_string(),
                assign_categories: vec!["Notify".to_string()],
                remove_categories: Vec::new(),
                assign_numeric: None,
                value: None,
                set_when: None,
                mark_done: false,
                delete: false,
            },
        )
        .expect("add action 1");
        cmd_category(
            &aglet,
            &store,
            CategoryCommand::AddAction {
                name: "Escalated".to_string(),
                assign_categories: Vec::new(),
                remove_categories: vec!["Low".to_string()],
                assign_numeric: None,
                value: None,
                set_when: None,
                mark_done: false,
                delete: false,
            },
        )
        .expect("add action 2");

        cmd_category(
            &aglet,
            &store,
            CategoryCommand::RemoveAction {
                name: "Escalated".to_string(),
                index: 1,
            },
        )
        .expect("remove action");

        let updated = store.get_category(source.id).expect("load source");
        assert_eq!(updated.actions.len(), 1);
        match &updated.actions[0] {
            Action::Remove { targets } => {
                assert!(targets.contains(&low.id));
            }
            other => panic!("expected Remove action, got {:?}", other),
        }
    }

    #[test]
    fn cmd_view_authoring_commands_update_view_incrementally() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let budget_2025 = Category::new("Moto Budget 2025".to_string());
        let budget_2026 = Category::new("Moto Budget 2026".to_string());
        let cost = {
            let mut category = Category::new("Cost".to_string());
            category.value_kind = CategoryValueKind::Numeric;
            category
        };
        store.create_category(&budget_2025).expect("create 2025");
        store.create_category(&budget_2026).expect("create 2026");
        store.create_category(&cost).expect("create cost");

        cmd_view(
            &aglet,
            &store,
            ViewCommand::Create {
                name: "Combined".to_string(),
                include: Vec::new(),
                or_include: vec![
                    "Moto Budget 2025".to_string(),
                    "Moto Budget 2026".to_string(),
                ],
                exclude: Vec::new(),
                hide_unmatched: true,
                hide_dependent_items: false,
            },
        )
        .expect("create view");
        cmd_view(
            &aglet,
            &store,
            ViewCommand::Section {
                command: ViewSectionCommand::Add {
                    name: "Combined".to_string(),
                    title: "All Expenses".to_string(),
                    include: Vec::new(),
                    or_include: vec![
                        "Moto Budget 2025".to_string(),
                        "Moto Budget 2026".to_string(),
                    ],
                    exclude: Vec::new(),
                    show_children: false,
                },
            },
        )
        .expect("add section");
        cmd_view(
            &aglet,
            &store,
            ViewCommand::Column {
                command: ViewColumnCommand::Add {
                    name: "Combined".to_string(),
                    section: 0,
                    column: "Cost".to_string(),
                    kind: Some(CliColumnKind::Standard),
                    width: Some(12),
                    summary: Some(CliSummaryFn::Sum),
                },
            },
        )
        .expect("add column");
        cmd_view(
            &aglet,
            &store,
            ViewCommand::Alias {
                command: ViewAliasCommand::Set {
                    name: "Combined".to_string(),
                    category: "Cost".to_string(),
                    alias: "Amount".to_string(),
                },
            },
        )
        .expect("set alias");
        cmd_view(
            &aglet,
            &store,
            ViewCommand::SetItemLabel {
                name: "Combined".to_string(),
                label: Some("Expense".to_string()),
                clear: false,
            },
        )
        .expect("set item label");
        cmd_view(
            &aglet,
            &store,
            ViewCommand::SetRemoveFromView {
                name: "Combined".to_string(),
                categories: vec!["Moto Budget 2025".to_string()],
                clear: false,
            },
        )
        .expect("set remove from view");

        let view = view_by_name(&store, "Combined").expect("load view");
        let or_category_ids: HashSet<_> = view.criteria.or_category_ids().collect();
        assert_eq!(
            or_category_ids,
            HashSet::from([budget_2025.id, budget_2026.id])
        );
        assert_eq!(view.sections.len(), 1);
        assert_eq!(view.sections[0].title, "All Expenses");
        assert_eq!(view.sections[0].columns.len(), 1);
        assert_eq!(view.sections[0].columns[0].heading, cost.id);
        assert_eq!(view.sections[0].columns[0].summary_fn, Some(SummaryFn::Sum));
        assert_eq!(
            view.category_aliases.get(&cost.id).map(String::as_str),
            Some("Amount")
        );
        assert_eq!(view.item_column_label.as_deref(), Some("Expense"));
        assert_eq!(
            view.remove_from_view_unassign,
            HashSet::from([budget_2025.id])
        );
    }

    fn temp_test_path(prefix: &str, extension: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}.{extension}"))
    }

    #[test]
    fn cmd_import_csv_creates_items_categories_and_values() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let budget = Category::new("Moto Budget 2026".to_string());
        store.create_category(&budget).expect("create budget");

        let csv_path = temp_test_path("aglet-cli-import", "csv");
        fs::write(
            &csv_path,
            "Date,Vendor,Category,Expense,Cost,Note\n2026-02-20,YCRS,Track,YCRS,4000,School day\n2026-02-23,2wtdw,DRZ4SM,ECU Flash,369,\n",
        )
        .expect("write csv");

        let result = cmd_import(
            &aglet,
            &store,
            ImportCommand::Csv {
                path: csv_path.clone(),
                title_col: "Expense".to_string(),
                date_col: Some("Date".to_string()),
                note_col: Some("Note".to_string()),
                category_cols: vec!["Category".to_string()],
                category_parent: Some("Budget Tags".to_string()),
                category_separator: ",".to_string(),
                vendor_cols: vec!["Vendor=Vendor".to_string()],
                value_cols: vec!["Cost=Cost".to_string()],
                assign: vec!["Moto Budget 2026".to_string()],
                dry_run: false,
            },
        );
        let _ = fs::remove_file(&csv_path);
        result.expect("import csv");

        let items = store.list_items().expect("list items");
        assert_eq!(items.len(), 2);
        let ycrs = items
            .iter()
            .find(|item| item.text == "YCRS")
            .expect("YCRS item");
        assert_eq!(ycrs.when_date, Some(date(2026, 2, 20).at(0, 0, 0, 0)));
        assert_eq!(ycrs.note.as_deref(), Some("School day"));

        let categories = store.get_hierarchy().expect("hierarchy");
        let budget_tags_id = categories
            .iter()
            .find(|category| category.name == "Budget Tags")
            .map(|category| category.id)
            .expect("budget tags exists");
        let vendor_parent_id = categories
            .iter()
            .find(|category| category.name == "Vendor")
            .map(|category| category.id)
            .expect("vendor exists");
        let track_id = categories
            .iter()
            .find(|category| category.name == "Track")
            .map(|category| category.id)
            .expect("track exists");
        let ycrs_vendor_id = categories
            .iter()
            .find(|category| category.name == "YCRS")
            .map(|category| category.id)
            .expect("ycrs vendor exists");
        let cost_id = categories
            .iter()
            .find(|category| category.name == "Cost")
            .map(|category| category.id)
            .expect("cost exists");
        assert_eq!(
            categories
                .iter()
                .find(|category| category.id == track_id)
                .and_then(|category| category.parent),
            Some(budget_tags_id)
        );
        assert_eq!(
            categories
                .iter()
                .find(|category| category.id == ycrs_vendor_id)
                .and_then(|category| category.parent),
            Some(vendor_parent_id)
        );
        assert!(ycrs.assignments.contains_key(&budget.id));
        assert!(ycrs.assignments.contains_key(&track_id));
        assert!(ycrs.assignments.contains_key(&ycrs_vendor_id));
        assert_eq!(
            ycrs.assignments
                .get(&cost_id)
                .and_then(|assignment| assignment.numeric_value),
            Some(Decimal::new(4000, 0))
        );
    }

    #[test]
    fn cmd_import_csv_reuses_existing_category_names_even_with_requested_parent() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let budget = Category::new("Moto Budget 2026".to_string());
        let track = Category::new("Track".to_string());
        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&budget).expect("create budget");
        store.create_category(&track).expect("create track");
        store.create_category(&cost).expect("create cost");

        let csv_path = temp_test_path("aglet-cli-import-reuse", "csv");
        fs::write(
            &csv_path,
            "Date,Vendor,Category,Expense,Cost\n2026-02-20,YCRS,Track,YCRS,4000\n",
        )
        .expect("write csv");

        let result = cmd_import(
            &aglet,
            &store,
            ImportCommand::Csv {
                path: csv_path.clone(),
                title_col: "Expense".to_string(),
                date_col: Some("Date".to_string()),
                note_col: None,
                category_cols: vec!["Category".to_string()],
                category_parent: Some("Budget Tags".to_string()),
                category_separator: ",".to_string(),
                vendor_cols: vec!["Vendor=Vendor".to_string()],
                value_cols: vec!["Cost=Cost".to_string()],
                assign: vec!["Moto Budget 2026".to_string()],
                dry_run: false,
            },
        );
        let _ = fs::remove_file(&csv_path);
        result.expect("import csv");

        let imported = store.list_items().expect("list items");
        assert_eq!(imported.len(), 1);
        assert!(imported[0].assignments.contains_key(&track.id));
    }

    #[test]
    fn blocked_item_ids_marks_open_dependency_as_blocked() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let dependency = Item::new("Dependency".to_string());
        let dependent = Item::new("Dependent".to_string());
        store.create_item(&dependency).expect("create dependency");
        store.create_item(&dependent).expect("create dependent");
        aglet
            .link_items_depends_on(dependent.id, dependency.id)
            .expect("link depends-on");

        let items = store.list_items().expect("list items");
        let blocked = blocked_item_ids(&store, &items).expect("blocked ids");
        assert!(blocked.contains(&dependent.id));
        assert!(!blocked.contains(&dependency.id));
    }

    #[test]
    fn parse_sort_spec_supports_when_and_direction_suffix() {
        let categories = vec![Category::new("Priority".to_string())];
        let when_key = parse_sort_spec("when:desc", &categories).expect("parse when desc");
        assert_eq!(
            when_key,
            CliSortKey {
                field: CliSortField::WhenDate,
                direction: CliSortDirection::Desc
            }
        );
        let item_key = parse_sort_spec("item", &categories).expect("parse item default");
        assert_eq!(
            item_key,
            CliSortKey {
                field: CliSortField::ItemText,
                direction: CliSortDirection::Asc
            }
        );
    }

    #[test]
    fn compare_items_by_sort_keys_numeric_missing_values_are_last() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create cost");

        let ten = Item::new("Ten".to_string());
        let missing = Item::new("Missing".to_string());
        let five = Item::new("Five".to_string());
        store.create_item(&ten).expect("create ten");
        store.create_item(&missing).expect("create missing");
        store.create_item(&five).expect("create five");

        aglet
            .assign_item_numeric_manual(
                ten.id,
                cost.id,
                Decimal::new(10, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign ten");
        aglet
            .assign_item_numeric_manual(
                five.id,
                cost.id,
                Decimal::new(5, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign five");

        let categories = store.get_hierarchy().expect("hierarchy");
        let key_asc = CliSortKey {
            field: CliSortField::Category(cost.id),
            direction: CliSortDirection::Asc,
        };
        let key_desc = CliSortKey {
            field: CliSortField::Category(cost.id),
            direction: CliSortDirection::Desc,
        };

        let mut rows = store.list_items().expect("list items");
        rows.sort_by(|left, right| {
            compare_items_by_sort_keys(left, right, &[key_asc], &categories)
        });
        let asc_texts: Vec<String> = rows.iter().map(|item| item.text.clone()).collect();
        assert_eq!(
            asc_texts,
            vec!["Five".to_string(), "Ten".to_string(), "Missing".to_string()]
        );

        rows.sort_by(|left, right| {
            compare_items_by_sort_keys(left, right, &[key_desc], &categories)
        });
        let desc_texts: Vec<String> = rows.iter().map(|item| item.text.clone()).collect();
        assert_eq!(
            desc_texts,
            vec!["Ten".to_string(), "Five".to_string(), "Missing".to_string()]
        );
    }

    #[test]
    fn retain_items_with_all_categories_enforces_and_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let issue_type = Category::new("Issue type".to_string());
        let status = Category::new("Status".to_string());
        store
            .create_category(&issue_type)
            .expect("create issue_type");
        store.create_category(&status).expect("create status");

        let both = Item::new("Both".to_string());
        let one = Item::new("One".to_string());
        let none = Item::new("None".to_string());
        store.create_item(&both).expect("create both");
        store.create_item(&one).expect("create one");
        store.create_item(&none).expect("create none");

        aglet
            .assign_item_manual(both.id, issue_type.id, Some("test:assign".to_string()))
            .expect("assign both issue_type");
        aglet
            .assign_item_manual(both.id, status.id, Some("test:assign".to_string()))
            .expect("assign both status");
        aglet
            .assign_item_manual(one.id, issue_type.id, Some("test:assign".to_string()))
            .expect("assign one issue_type");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_all_categories(&mut rows, &[issue_type.id, status.id]);

        let remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        assert_eq!(remaining_texts, vec!["Both".to_string()]);
    }

    #[test]
    fn retain_items_with_any_categories_enforces_or_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let aglet_cat = Category::new("Aglet".to_string());
        let neonv = Category::new("NeoNV".to_string());
        let other = Category::new("Project3".to_string());
        store.create_category(&aglet_cat).expect("create aglet");
        store.create_category(&neonv).expect("create neonv");
        store.create_category(&other).expect("create project3");

        let aglet_item = Item::new("Aglet item".to_string());
        let neonv_item = Item::new("NeoNV item".to_string());
        let other_item = Item::new("Project3 item".to_string());
        store.create_item(&aglet_item).expect("create aglet item");
        store.create_item(&neonv_item).expect("create neonv item");
        store
            .create_item(&other_item)
            .expect("create project3 item");

        aglet
            .assign_item_manual(aglet_item.id, aglet_cat.id, Some("test:assign".to_string()))
            .expect("assign aglet");
        aglet
            .assign_item_manual(neonv_item.id, neonv.id, Some("test:assign".to_string()))
            .expect("assign neonv");
        aglet
            .assign_item_manual(other_item.id, other.id, Some("test:assign".to_string()))
            .expect("assign project3");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_any_categories(&mut rows, &[aglet_cat.id, neonv.id]);

        let mut remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        remaining_texts.sort();
        assert_eq!(
            remaining_texts,
            vec!["Aglet item".to_string(), "NeoNV item".to_string()]
        );
    }

    #[test]
    fn dependency_state_filter_transitions_when_links_are_added_or_removed() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");

        let mut rows = store.list_items().expect("list initial");
        retain_items_by_dependency_state(&store, &mut rows, true).expect("filter initial blocked");
        assert!(rows.is_empty(), "no links means nothing is blocked");

        aglet
            .link_items_depends_on(blocked.id, blocker.id)
            .expect("link depends-on");
        let mut rows = store.list_items().expect("list linked");
        retain_items_by_dependency_state(&store, &mut rows, true).expect("filter linked blocked");
        assert_eq!(
            rows.into_iter().map(|item| item.text).collect::<Vec<_>>(),
            vec!["Blocked".to_string()]
        );

        aglet
            .unlink_items_depends_on(blocked.id, blocker.id)
            .expect("unlink depends-on");
        let mut rows = store.list_items().expect("list unlinked");
        retain_items_by_dependency_state(&store, &mut rows, true).expect("filter unlinked blocked");
        assert!(
            rows.is_empty(),
            "removing dependency link clears blocked state"
        );
    }

    #[test]
    fn parse_csv_decimals_rejects_empty_value_token() {
        let err = parse_csv_decimals("1,,2", "Complexity").expect_err("should fail");
        assert_eq!(
            err,
            "invalid --value-in for category 'Complexity': empty value in CSV list"
        );
    }

    #[test]
    fn build_numeric_filters_rejects_unknown_category() {
        let categories = vec![Category::new("Complexity".to_string())];
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            dependency_state_filter: None,
            value_eq: vec!["Nope".to_string(), "2".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&categories, &filters).expect_err("should fail");
        assert_eq!(err, "category not found: Nope");
    }

    #[test]
    fn build_numeric_filters_rejects_tag_category() {
        let categories = vec![Category::new("Status".to_string())];
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            dependency_state_filter: None,
            value_eq: vec!["Status".to_string(), "2".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&categories, &filters).expect_err("should fail");
        assert_eq!(
            err,
            "category 'Status' is not Numeric; numeric value filters require a Numeric category"
        );
    }

    #[test]
    fn build_numeric_filters_rejects_malformed_decimal() {
        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            dependency_state_filter: None,
            value_eq: vec!["Complexity".to_string(), "abc".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&[complexity], &filters).expect_err("should fail");
        assert!(err.contains("invalid decimal value 'abc'"));
    }

    #[test]
    fn retain_items_matching_numeric_filters_handles_eq_in_max_and_missing_values() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store
            .create_category(&complexity)
            .expect("create complexity");

        let one = Item::new("One".to_string());
        let two = Item::new("Two".to_string());
        let five = Item::new("Five".to_string());
        let missing = Item::new("Missing".to_string());
        store.create_item(&one).expect("create one");
        store.create_item(&two).expect("create two");
        store.create_item(&five).expect("create five");
        store.create_item(&missing).expect("create missing");

        aglet
            .assign_item_numeric_manual(
                one.id,
                complexity.id,
                Decimal::new(1, 0),
                Some("test:set".to_string()),
            )
            .expect("set one");
        aglet
            .assign_item_numeric_manual(
                two.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set two");
        aglet
            .assign_item_numeric_manual(
                five.id,
                complexity.id,
                Decimal::new(5, 0),
                Some("test:set".to_string()),
            )
            .expect("set five");

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Eq(Decimal::new(2, 0)),
            }],
        );
        assert_eq!(
            rows.into_iter().map(|i| i.text).collect::<Vec<_>>(),
            vec!["Two".to_string()]
        );

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Max(Decimal::new(2, 0)),
            }],
        );
        let mut max_texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        max_texts.sort();
        assert_eq!(max_texts, vec!["One".to_string(), "Two".to_string()]);

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::In(vec![Decimal::new(1, 0), Decimal::new(5, 0)]),
            }],
        );
        let mut in_texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        in_texts.sort();
        assert_eq!(in_texts, vec!["Five".to_string(), "One".to_string()]);
    }

    #[test]
    fn numeric_filters_compose_with_category_include_and_exclude_filters() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let project = Category::new("Aglet".to_string());
        let done = Category::new("Complete".to_string());
        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store.create_category(&project).expect("create project");
        store.create_category(&done).expect("create complete");
        store
            .create_category(&complexity)
            .expect("create complexity");

        let include_keep = Item::new("IncludeKeep".to_string());
        let include_drop_value = Item::new("IncludeDropValue".to_string());
        let include_drop_excluded = Item::new("IncludeDropExcluded".to_string());
        store
            .create_item(&include_keep)
            .expect("create include keep");
        store
            .create_item(&include_drop_value)
            .expect("create include drop value");
        store
            .create_item(&include_drop_excluded)
            .expect("create include drop excluded");

        for item_id in [
            include_keep.id,
            include_drop_value.id,
            include_drop_excluded.id,
        ] {
            aglet
                .assign_item_manual(item_id, project.id, Some("test:assign".to_string()))
                .expect("assign project");
        }
        aglet
            .assign_item_manual(
                include_drop_excluded.id,
                done.id,
                Some("test:assign".to_string()),
            )
            .expect("assign complete");
        aglet
            .assign_item_numeric_manual(
                include_keep.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set keep");
        aglet
            .assign_item_numeric_manual(
                include_drop_value.id,
                complexity.id,
                Decimal::new(5, 0),
                Some("test:set".to_string()),
            )
            .expect("set drop value");
        aglet
            .assign_item_numeric_manual(
                include_drop_excluded.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set excluded");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_all_categories(&mut rows, &[project.id]);
        reject_items_with_any_categories(&mut rows, &[done.id]);
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Max(Decimal::new(2, 0)),
            }],
        );

        assert_eq!(
            rows.into_iter().map(|i| i.text).collect::<Vec<_>>(),
            vec!["IncludeKeep".to_string()]
        );
    }

    #[test]
    fn multiple_numeric_filters_use_and_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store
            .create_category(&complexity)
            .expect("create complexity");

        let one = Item::new("One".to_string());
        let two = Item::new("Two".to_string());
        let three = Item::new("Three".to_string());
        store.create_item(&one).expect("create one");
        store.create_item(&two).expect("create two");
        store.create_item(&three).expect("create three");
        aglet
            .assign_item_numeric_manual(
                one.id,
                complexity.id,
                Decimal::new(1, 0),
                Some("test:set".to_string()),
            )
            .expect("set one");
        aglet
            .assign_item_numeric_manual(
                two.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set two");
        aglet
            .assign_item_numeric_manual(
                three.id,
                complexity.id,
                Decimal::new(3, 0),
                Some("test:set".to_string()),
            )
            .expect("set three");

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[
                NumericFilter {
                    category_id: complexity.id,
                    category_name: "Complexity".to_string(),
                    predicate: NumericPredicate::In(vec![
                        Decimal::new(1, 0),
                        Decimal::new(2, 0),
                        Decimal::new(3, 0),
                    ]),
                },
                NumericFilter {
                    category_id: complexity.id,
                    category_name: "Complexity".to_string(),
                    predicate: NumericPredicate::Max(Decimal::new(2, 0)),
                },
            ],
        );
        let mut texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        texts.sort();
        assert_eq!(texts, vec!["One".to_string(), "Two".to_string()]);
    }

    #[test]
    fn reject_items_with_any_categories_enforces_not_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let complete = Category::new("Complete".to_string());
        let in_progress = Category::new("In Progress".to_string());
        store.create_category(&complete).expect("create complete");
        store
            .create_category(&in_progress)
            .expect("create in-progress");

        let done_item = Item::new("Done item".to_string());
        let active_item = Item::new("Active item".to_string());
        store.create_item(&done_item).expect("create done item");
        store.create_item(&active_item).expect("create active item");

        aglet
            .assign_item_manual(done_item.id, complete.id, Some("test:assign".to_string()))
            .expect("assign complete");
        aglet
            .assign_item_manual(
                active_item.id,
                in_progress.id,
                Some("test:assign".to_string()),
            )
            .expect("assign in-progress");

        let mut rows = store.list_items().expect("list items");
        reject_items_with_any_categories(&mut rows, &[complete.id]);

        let remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        assert_eq!(remaining_texts, vec!["Active item".to_string()]);
    }

    #[test]
    fn item_link_section_lines_include_prereqs_blocks_and_related() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        let c = Item::new("Task C".to_string());
        let d = Item::new("Task D".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");
        store.create_item(&c).expect("create c");
        store.create_item(&d).expect("create d");

        aglet
            .link_items_depends_on(a.id, b.id)
            .expect("link depends-on");
        aglet.link_items_blocks(c.id, a.id).expect("link blocks");
        aglet.link_items_related(a.id, d.id).expect("link related");

        let lines = item_link_section_lines(&store, a.id).expect("render link lines");
        assert!(lines.iter().any(|line| line == "prereqs:"));
        assert!(lines
            .iter()
            .any(|line| line == "dependents (blocks): (none)" || line == "dependents (blocks):"));
        assert!(lines.iter().any(|line| line == "related:"));
        assert!(lines.iter().any(|line| line.contains("Task B")));
        assert!(lines.iter().any(|line| line.contains("Task C")));
        assert!(lines.iter().any(|line| line.contains("Task D")));
    }

    #[test]
    fn cmd_edit_append_note_to_empty() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            Some("first note".to_string()),
            None,
            false,
            None,
            None,
            false,
            None,
            false,
        )
        .expect("append to empty");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("first note"));
    }

    #[test]
    fn cmd_edit_append_note_to_existing() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("existing note".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            Some("appended text".to_string()),
            None,
            false,
            None,
            None,
            false,
            None,
            false,
        )
        .expect("append to existing");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(
            updated.note.as_deref(),
            Some("existing note\nappended text")
        );
    }

    #[test]
    fn cmd_edit_append_note_multiline() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("line one".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            Some("line two\nline three".to_string()),
            None,
            false,
            None,
            None,
            false,
            None,
            false,
        )
        .expect("append multiline");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(
            updated.note.as_deref(),
            Some("line one\nline two\nline three")
        );
    }

    #[test]
    fn cmd_edit_append_note_rejects_with_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let result = cmd_edit(
            &aglet,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            Some("replace".to_string()),
            Some("append".to_string()),
            None,
            false,
            None,
            None,
            false,
            None,
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn cmd_edit_append_note_rejects_with_clear_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let result = cmd_edit(
            &aglet,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            None,
            Some("append".to_string()),
            None,
            true,
            None,
            None,
            false,
            None,
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn read_note_from_stdin_reads_multiline_payload() {
        let mut reader = Cursor::new("line one\nline two\n");
        let note = read_note_from_stdin(&mut reader).expect("read note");
        assert_eq!(note, "line one\nline two\n");
    }

    #[test]
    fn cmd_edit_note_stdin_replaces_existing_note() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("old note".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            None,
            Some("stdin note\nnext line".to_string()),
            false,
            None,
            None,
            false,
            None,
            false,
        )
        .expect("replace from stdin");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("stdin note\nnext line"));
    }

    #[test]
    fn cmd_edit_note_stdin_rejects_with_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let result = cmd_edit(
            &aglet,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            Some("replace".to_string()),
            None,
            Some("stdin".to_string()),
            false,
            None,
            None,
            false,
            None,
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn cmd_edit_note_stdin_empty_is_noop() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("existing".to_string());
        let previous_modified_at = item.modified_at;
        store.create_item(&item).expect("create");

        cmd_edit(
            &aglet,
            item.id.to_string(),
            None,
            None,
            None,
            Some(String::new()),
            false,
            None,
            None,
            false,
            None,
            false,
        )
        .expect("empty stdin no-op");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("existing"));
        assert_eq!(updated.modified_at, previous_modified_at);
    }

    #[test]
    fn cmd_unlink_removes_dependency_link() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        aglet
            .link_items_depends_on(a.id, b.id)
            .expect("link depends-on");

        cmd_unlink(
            &aglet,
            UnlinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: b.id.to_string(),
            },
        )
        .expect("unlink via canonical command");
        assert!(store
            .list_dependency_ids_for_item(a.id)
            .expect("list dependencies")
            .is_empty());
    }

    // ── cmd_link ───────────────────────────────────────────────────────────────

    #[test]
    fn cmd_link_depends_on_creates_dependency() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        cmd_link(
            &aglet,
            LinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: b.id.to_string(),
            },
        )
        .expect("cmd_link DependsOn should succeed");

        let deps = store.list_dependency_ids_for_item(a.id).expect("list");
        assert!(deps.contains(&b.id), "a should depend-on b");
    }

    #[test]
    fn cmd_link_blocks_creates_inverse_dependency() {
        // "A blocks B" means B depends-on A.
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Blocker".to_string());
        let b = Item::new("Blocked".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        cmd_link(
            &aglet,
            LinkCommand::Blocks {
                blocker_item_id: a.id.to_string(),
                blocked_item_id: b.id.to_string(),
            },
        )
        .expect("cmd_link Blocks should succeed");

        // "A blocks B" is stored as "B depends-on A"
        let deps = store.list_dependency_ids_for_item(b.id).expect("list");
        assert!(
            deps.contains(&a.id),
            "b should depend-on a after 'a blocks b'"
        );
    }

    #[test]
    fn cmd_link_related_creates_symmetric_link() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        cmd_link(
            &aglet,
            LinkCommand::Related {
                item_a_id: a.id.to_string(),
                item_b_id: b.id.to_string(),
            },
        )
        .expect("cmd_link Related should succeed");

        // Related links are symmetric — both items should report the other.
        let related_a = store.list_related_ids_for_item(a.id).expect("list a");
        let related_b = store.list_related_ids_for_item(b.id).expect("list b");
        assert!(related_a.contains(&b.id), "a should see b as related");
        assert!(related_b.contains(&a.id), "b should see a as related");
    }

    #[test]
    fn cmd_link_depends_on_self_returns_error() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        store.create_item(&a).expect("create a");

        let result = cmd_link(
            &aglet,
            LinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: a.id.to_string(),
            },
        );
        assert!(result.is_err(), "self depends-on link should be rejected");
    }

    #[test]
    fn cmd_link_depends_on_cycle_returns_error() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("A".to_string());
        let b = Item::new("B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        // A depends-on B
        cmd_link(
            &aglet,
            LinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: b.id.to_string(),
            },
        )
        .expect("first link should succeed");

        // B depends-on A would create a cycle
        let result = cmd_link(
            &aglet,
            LinkCommand::DependsOn {
                item_id: b.id.to_string(),
                depends_on_item_id: a.id.to_string(),
            },
        );
        assert!(result.is_err(), "cyclic dependency should be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("cycle"),
            "error should mention cycle, got: {msg}"
        );
    }

    #[test]
    fn cmd_link_blocks_cycle_returns_error() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("A".to_string());
        let b = Item::new("B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        cmd_link(
            &aglet,
            LinkCommand::Blocks {
                blocker_item_id: a.id.to_string(),
                blocked_item_id: b.id.to_string(),
            },
        )
        .expect("first blocks link should succeed");

        let result = cmd_link(
            &aglet,
            LinkCommand::Blocks {
                blocker_item_id: b.id.to_string(),
                blocked_item_id: a.id.to_string(),
            },
        );
        assert!(result.is_err(), "cyclic blocks link should be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("cycle"),
            "error should mention cycle, got: {msg}"
        );
    }

    #[test]
    fn cmd_link_depends_on_is_idempotent() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        let cmd = || {
            cmd_link(
                &aglet,
                LinkCommand::DependsOn {
                    item_id: a.id.to_string(),
                    depends_on_item_id: b.id.to_string(),
                },
            )
        };

        cmd().expect("first link");
        cmd().expect("second link — should not error");

        let deps = store.list_dependency_ids_for_item(a.id).expect("list");
        assert_eq!(deps.len(), 1, "only one dependency should exist");
    }

    fn empty_list_filters() -> ListFilters {
        ListFilters {
            all_categories: vec![],
            any_categories: vec![],
            exclude_categories: vec![],
            dependency_state_filter: None,
            value_eq: vec![],
            value_in: vec![],
            value_max: vec![],
            include_done: false,
        }
    }

    #[test]
    fn cmd_list_defaults_to_all_items_view() {
        let store = Store::open_memory().expect("store");
        // Store::open_memory creates the "All Items" system view automatically.
        // Create a second view that would sort first alphabetically.
        let custom = View::new("AAA View".to_string());
        store.create_view(&custom).expect("create view");

        let item = Item::new("Hello".to_string());
        store.create_item(&item).expect("create item");

        // Running cmd_list without a view name should succeed and use "All Items".
        let result = cmd_list(
            &store,
            None,
            empty_list_filters(),
            vec![],
            OutputFormatArg::Table,
            TableStyle::Verbose,
        );
        assert!(result.is_ok(), "cmd_list should succeed: {result:?}");
    }

    #[test]
    fn cmd_list_prefers_all_items_over_alphabetically_first_view() {
        let store = Store::open_memory().expect("store");
        // "All Items" is created by Store::open_memory. Create a view that
        // sorts before it alphabetically.
        let earlier = View::new("AAA Earlier".to_string());
        store.create_view(&earlier).expect("create view");

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create item");

        // cmd_list with no --view should use "All Items", not "AAA Earlier".
        // "All Items" has the default section that shows items;
        // "AAA Earlier" has no matching criteria so it would show nothing useful.
        let result = cmd_list(
            &store,
            None,
            empty_list_filters(),
            vec![],
            OutputFormatArg::Table,
            TableStyle::Verbose,
        );
        assert!(
            result.is_ok(),
            "cmd_list should prefer All Items: {result:?}"
        );
    }

    #[test]
    fn cmd_list_explicit_view_overrides_default() {
        let store = Store::open_memory().expect("store");
        let custom = View::new("Custom".to_string());
        store.create_view(&custom).expect("create view");

        let item = Item::new("Explicit view test".to_string());
        store.create_item(&item).expect("create item");

        let result = cmd_list(
            &store,
            Some("Custom".to_string()),
            empty_list_filters(),
            vec![],
            OutputFormatArg::Table,
            TableStyle::Verbose,
        );
        assert!(result.is_ok(), "explicit --view should work: {result:?}");
    }

    // ── cmd_add ────────────────────────────────────────────────────────────────

    #[test]
    fn cmd_add_rejects_empty_text() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let err = cmd_add(&aglet, "".to_string(), None, None, vec![], vec![])
            .expect_err("empty text should be rejected");
        assert!(err.contains("text cannot be empty"), "error was: {err}");
    }

    #[test]
    fn cmd_add_rejects_whitespace_only_text() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let err = cmd_add(&aglet, "   ".to_string(), None, None, vec![], vec![])
            .expect_err("whitespace-only text should be rejected");
        assert!(err.contains("text cannot be empty"), "error was: {err}");
    }

    // ── edit --text flag ───────────────────────────────────────────────────────

    #[test]
    fn clap_edit_rejects_unknown_text_flag() {
        // The edit command only accepts text as a positional argument; --text is
        // not a recognised flag and clap should reject it.
        let result = Cli::try_parse_from([
            "aglet",
            "edit",
            "123e4567-e89b-12d3-a456-426614174000",
            "--text",
            "some text",
        ]);
        assert!(
            result.is_err(),
            "--text should not be a recognised flag for edit"
        );
    }

    // ── cmd_unlink idempotency ─────────────────────────────────────────────────

    #[test]
    fn cmd_unlink_is_idempotent_for_nonexistent_link() {
        // Unlinking a dependency that was never created should succeed silently
        // (idempotent behaviour confirmed by the CLI demo exercise).
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        // No link was ever created between a and b; unlink should still succeed.
        cmd_unlink(
            &aglet,
            UnlinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: b.id.to_string(),
            },
        )
        .expect("unlink of nonexistent link should succeed");
    }

    // ── cmd_claim missing workflow ─────────────────────────────────────────────

    #[test]
    fn cmd_claim_fails_when_no_workflow_configured() {
        // cmd_claim requires a workflow to be configured in the store.
        // Without one, it should fail with an informative error.
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("Some task".to_string());
        store.create_item(&item).expect("create item");

        let err = cmd_claim(&aglet, &store, item.id.to_string())
            .expect_err("claim should fail when no workflow is configured");
        assert!(
            !err.is_empty(),
            "expected a non-empty error message, got: {err}"
        );
    }
