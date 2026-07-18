    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use jiff::civil::{Date, DateTime};
    use jiff::Timestamp;
    use rust_decimal::Decimal;

    use super::Aglet;
    use crate::classification::{
        ClassificationConfig, LiteralClassificationMode, OllamaProviderSettings, OllamaTransport,
        SemanticClassificationMode, SuggestionStatus, PROVIDER_ID_IMPLICIT_STRING,
        PROVIDER_ID_OLLAMA_OPENAI_COMPAT,
    };
    use crate::error::AgletError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentSource, Category, CategoryId, CategoryValueKind, Condition,
        CriterionMode, Item, ItemId, ItemLinkKind, Query, Section, View, WhenBucket,
        RESERVED_CATEGORY_NAME_DONE, RESERVED_CATEGORY_NAME_WHEN,
    };
    use crate::query::{resolve_view, resolve_when_bucket};
    use crate::store::Store;

    fn category(name: &str, implicit: bool) -> Category {
        let mut category = Category::new(name.to_string());
        category.enable_implicit_string = implicit;
        category
    }

    fn child_category(name: &str, parent: CategoryId, implicit: bool) -> Category {
        let mut category = category(name, implicit);
        category.parent = Some(parent);
        category
    }

    fn section(title: &str) -> Section {
        Section {
            title: title.to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        }
    }

    fn view(name: &str) -> View {
        View::new(name.to_string())
    }

    fn manual_assignment(origin: &str) -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some(origin.to_string()),
            explanation: None,
            numeric_value: None,
        }
    }

    fn date(y: i16, m: i8, d: i8) -> Date {
        Date::new(y, m, d).expect("valid date")
    }

    fn datetime(y: i16, m: i8, d: i8, h: i8, min: i8) -> DateTime {
        date(y, m, d).at(h, min, 0, 0)
    }

    fn when_category_id(store: &Store) -> CategoryId {
        store
            .get_hierarchy()
            .expect("hierarchy available")
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_WHEN)
            })
            .expect("reserved When category exists")
            .id
    }

    fn category_id_by_name(store: &Store, name: &str) -> Option<CategoryId> {
        store
            .get_hierarchy()
            .expect("hierarchy available")
            .into_iter()
            .find(|category| category.name.eq_ignore_ascii_case(name))
            .map(|category| category.id)
    }

    fn make_item(store: &Store, text: &str) -> ItemId {
        let item = Item::new(text.to_string());
        let id = item.id;
        store.create_item(&item).unwrap();
        id
    }

    #[derive(Default)]
    struct FakeOllamaTransport {
        response: Option<String>,
    }

    impl OllamaTransport for FakeOllamaTransport {
        fn complete(
            &self,
            _settings: &OllamaProviderSettings,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> crate::error::Result<Option<String>> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn create_item_triggers_classification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let sarah = category("Sarah", true);
        store.create_category(&sarah).unwrap();

        let item = Item::new("Sarah's meeting".to_string());
        let result = aglet.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&sarah.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&sarah.id));
    }

    #[test]
    fn create_item_triggers_classification_from_also_match_term() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut phone_calls = category("Phone Calls", true);
        phone_calls.match_category_name = false;
        phone_calls.also_match = vec!["dial".to_string(), "ring".to_string()];
        store.create_category(&phone_calls).unwrap();

        let item = Item::new("Dial Sarah tomorrow".to_string());
        let result = aglet.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&phone_calls.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&phone_calls.id));
    }

    #[test]
    fn create_item_does_not_match_literal_category_name_when_disabled() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut person = category("Person", true);
        person.match_category_name = false;
        person.also_match = vec!["bob".to_string(), "sally".to_string()];
        store.create_category(&person).unwrap();

        let person_item = Item::new("Person".to_string());
        let person_result = aglet.create_item(&person_item).unwrap();
        assert!(!person_result.new_assignments.contains(&person.id));

        let bob_item = Item::new("Call Bob tomorrow".to_string());
        let bob_result = aglet.create_item(&bob_item).unwrap();
        assert!(bob_result.new_assignments.contains(&person.id));
    }

    #[test]
    fn create_item_triggers_classification_from_suffix_normalized_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let calls = category("Call", true);
        store.create_category(&calls).unwrap();

        let item = Item::new("Calling vendors".to_string());
        let result = aglet.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&calls.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&calls.id));
    }

    #[test]
    fn create_item_in_suggest_review_mode_queues_pending_suggestions_without_assigning() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::SuggestReview,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store
            .set_classification_config(&cfg)
            .expect("persist config");

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Book travel next Tuesday".to_string());
        let result = aglet
            .create_item_with_reference_date(&item, date(2026, 3, 20))
            .unwrap();
        assert_eq!(result.new_assignments.len(), 1);
        let when_id = store
            .get_hierarchy()
            .expect("load hierarchy")
            .into_iter()
            .find(|category| category.name == RESERVED_CATEGORY_NAME_WHEN)
            .expect("reserved When category present")
            .id;
        assert!(result.new_assignments.contains(&when_id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
        assert!(assignments.contains_key(&when_id));

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .expect("list pending suggestions");
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn create_item_with_classification_disabled_skips_implicit_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cfg = ClassificationConfig {
            enabled: false,
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store
            .set_classification_config(&cfg)
            .expect("persist config");

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Travel to Seattle".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
    }

    #[test]
    fn literal_auto_apply_and_semantic_off_keeps_current_deterministic_behavior() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        store.create_category(&work).unwrap();

        let item = Item::new("Work trip planning".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn literal_auto_apply_and_semantic_review_can_run_together() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.9,"rationale":"trip planning"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        let travel = category("Travel", false);
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let item = Item::new("Work trip planning".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&travel.id));

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].provider_id, PROVIDER_ID_OLLAMA_OPENAI_COMPAT);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn semantic_review_does_not_queue_duplicate_for_already_assigned_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.95,"rationale":"travel intent"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Conference travel planning".to_string());
        let result = aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&travel.id));
        assert_eq!(result.semantic_candidates_seen, 1);
        assert_eq!(result.semantic_candidates_queued_review, 0);
        assert_eq!(result.semantic_candidates_skipped_already_assigned, 1);

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(
            pending.is_empty(),
            "should not queue semantic duplicate for already-assigned category"
        );
    }

    #[test]
    fn semantic_review_skips_exclusive_sibling_when_other_child_already_assigned() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Normal","confidence":0.95,"rationale":"default priority"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        aglet.create_category(&priority).unwrap();

        let mut high = child_category("High", priority.id, false);
        high.enable_semantic_classification = true;
        aglet.create_category(&high).unwrap();

        let mut normal = child_category("Normal", priority.id, false);
        normal.enable_semantic_classification = true;
        aglet.create_category(&normal).unwrap();

        let item = Item::new("TUI task".to_string());
        aglet.create_item(&item).unwrap();
        aglet.assign_item_manual(item.id, high.id, None).unwrap();

        let result = aglet
            .process_item_save(item.id, jiff::Zoned::now().date(), false)
            .unwrap();

        assert_eq!(result.semantic_candidates_seen, 1);
        assert_eq!(result.semantic_candidates_queued_review, 0);
        assert_eq!(result.semantic_candidates_skipped_already_assigned, 0);
        assert_eq!(result.semantic_candidates_skipped_unavailable, 1);

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(
            pending.is_empty(),
            "should not queue semantic suggestion for an exclusive sibling"
        );
    }

    #[test]
    fn semantic_review_keeps_highest_confidence_suggestion_per_exclusive_family() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"High","confidence":0.78,"rationale":"important but not urgent"},{"category":"Normal","confidence":0.88,"rationale":"routine feature work"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        aglet.create_category(&priority).unwrap();

        let critical = child_category("Critical", priority.id, false);
        aglet.create_category(&critical).unwrap();
        let mut high = child_category("High", priority.id, false);
        high.enable_semantic_classification = true;
        aglet.create_category(&high).unwrap();
        let mut normal = child_category("Normal", priority.id, false);
        normal.enable_semantic_classification = true;
        aglet.create_category(&normal).unwrap();
        let low = child_category("Low", priority.id, false);
        aglet.create_category(&low).unwrap();

        let item = Item::new("feature to run external automations on assign/unassign".to_string());
        let result = aglet.create_item(&item).unwrap();

        assert_eq!(result.semantic_candidates_seen, 2);
        assert_eq!(result.semantic_candidates_queued_review, 1);
        assert_eq!(result.semantic_candidates_skipped_unavailable, 1);

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == normal.id
        ));
        assert_eq!(pending[0].confidence, Some(0.88));
    }

    #[test]
    fn semantic_mode_can_run_without_literal_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.7,"rationale":"travel intent"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Plan a trip soon".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn literal_and_semantic_suggest_review_queue_both_category_suggestions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.88,"rationale":"trip planning"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::SuggestReview,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        let travel = category("Travel", false);
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let item = Item::new("Work trip planning".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&travel.id));

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|suggestion| {
            suggestion.provider_id == PROVIDER_ID_IMPLICIT_STRING
                && matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == work.id
                )
        }));
        assert!(pending.iter().any(|suggestion| {
            suggestion.provider_id == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
                && matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == travel.id
                )
        }));
    }

    #[test]
    fn semantic_matching_is_independent_from_implicit_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.85,"rationale":"trip intent"}]}"#
                    .to_string(),
            ),
        });
        let aglet = Aglet::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut travel = category("Travel", false);
        travel.enable_semantic_classification = true;
        store.create_category(&travel).unwrap();

        let item = Item::new("Need flights and a hotel".to_string());
        aglet.create_item(&item).unwrap();

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn create_item_prefers_earlier_child_when_multiple_exclusive_derived_rules_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let bug = category("Bug", true);
        store.create_category(&bug).unwrap();

        let mut tui = category("TUI", true);
        store.create_category(&tui).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();

        let mut critical = child_category("Critical", priority.id, false);
        let mut critical_criteria = Query::default();
        critical_criteria.set_criterion(CriterionMode::And, bug.id);
        critical_criteria.set_criterion(CriterionMode::And, tui.id);
        critical.conditions.push(Condition::Profile {
            criteria: Box::new(critical_criteria),
        });
        store.create_category(&critical).unwrap();

        let mut high = child_category("High", priority.id, false);
        let mut high_criteria = Query::default();
        high_criteria.set_criterion(CriterionMode::And, tui.id);
        high.conditions.push(Condition::Profile {
            criteria: Box::new(high_criteria),
        });
        store.create_category(&high).unwrap();

        let mut low = child_category("Low", priority.id, false);
        let mut low_criteria = Query::default();
        low_criteria.set_criterion(CriterionMode::And, tui.id);
        low.conditions.push(Condition::Profile {
            criteria: Box::new(low_criteria),
        });
        store.create_category(&low).unwrap();

        tui.actions.push(Action::Assign {
            targets: HashSet::from([high.id]),
        });
        store.update_category(&tui).unwrap();

        let item = Item::new("Bug in TUI".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&bug.id));
        assert!(assignments.contains_key(&tui.id));
        assert!(
            assignments.contains_key(&critical.id),
            "earliest matching child should win the exclusive family"
        );
        assert!(
            !assignments.contains_key(&high.id),
            "later derived sibling should be suppressed"
        );
        assert!(
            !assignments.contains_key(&low.id),
            "later derived sibling should be suppressed"
        );
    }

    #[test]
    fn update_item_keeps_manual_exclusive_choice_over_later_auto_classified_action() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut tui = category("TUI", true);
        store.create_category(&tui).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();

        let high = child_category("High", priority.id, false);
        let low = child_category("Low", priority.id, false);
        store.create_category(&high).unwrap();
        store.create_category(&low).unwrap();

        tui.actions.push(Action::Assign {
            targets: HashSet::from([high.id]),
        });
        store.update_category(&tui).unwrap();

        let item = Item::new("plain task".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, low.id, Some("manual:test".to_string()))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "TUI task".to_string();
        updated.modified_at = Timestamp::now();
        aglet.update_item(&updated).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&tui.id));
        assert!(
            assignments.contains_key(&low.id),
            "manual exclusive choice should survive later derived action assignments"
        );
        assert!(
            !assignments.contains_key(&high.id),
            "derived action should not replace the existing manual family choice"
        );
    }

    #[test]
    fn when_parser_follows_literal_policy_and_is_skipped_when_literal_mode_is_off() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store.set_classification_config(&cfg).unwrap();

        let item = Item::new("Book travel next Tuesday".to_string());
        let result = aglet
            .create_item_with_reference_date(&item, date(2026, 3, 20))
            .unwrap();
        assert!(result.new_assignments.is_empty());

        let when_id = store
            .get_hierarchy()
            .expect("load hierarchy")
            .into_iter()
            .find(|category| category.name == RESERVED_CATEGORY_NAME_WHEN)
            .expect("reserved When category present")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&when_id));
    }

    #[test]
    #[ignore = "requires local Ollama with a mistral-compatible model running"]
    fn local_ollama_smoke_test_current_item_review_flow() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.ollama.base_url = "http://127.0.0.1:11434/v1".to_string();
        cfg.ollama.model = "mistral".to_string();
        cfg.ollama.timeout_secs = 30;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut work = category("Work", true);
        work.enable_semantic_classification = false;
        let mut travel = category("Travel", false);
        travel.enable_semantic_classification = true;
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let mut item = Item::new("Work trip planning for conference travel".to_string());
        item.note = Some("Book flights, hotel, and local transport".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&work.id),
            "expected literal Work assignment to auto-apply"
        );
        assert!(
            !assignments.contains_key(&travel.id),
            "semantic Travel should queue for review before acceptance"
        );

        let pending = aglet
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(
            pending.iter().any(|suggestion| {
                suggestion.provider_id == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
                    && matches!(
                        suggestion.assignment,
                        crate::classification::CandidateAssignment::Category(category_id)
                            if category_id == travel.id
                    )
            }),
            "expected a pending Ollama travel suggestion, got: {pending:?}"
        );

        let travel_suggestion = pending
            .iter()
            .find(|suggestion| {
                matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == travel.id
                )
            })
            .expect("travel suggestion present");

        aglet
            .accept_classification_suggestion(travel_suggestion.id)
            .unwrap();

        let reloaded_assignments = store.get_assignments_for_item(item.id).unwrap();
        let travel_assignment = reloaded_assignments
            .get(&travel.id)
            .expect("travel assignment should exist after acceptance");
        assert_eq!(
            travel_assignment.source,
            AssignmentSource::SuggestionAccepted
        );

        let reloaded_suggestion = store
            .get_classification_suggestion(travel_suggestion.id)
            .unwrap()
            .expect("accepted suggestion exists");
        assert_eq!(reloaded_suggestion.status, SuggestionStatus::Accepted);
    }

    #[test]
    fn create_item_hashtag_matches_existing_categories_without_creating_hash_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();
        let high = child_category("High", priority.id, true);
        store.create_category(&high).unwrap();
        let follow_up = category("Follow-up", true);
        store.create_category(&follow_up).unwrap();

        let item = Item::new("Hashtag parsing test #high #FOLLOW-UP".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&high.id));
        assert!(assignments.contains_key(&priority.id));
        assert!(assignments.contains_key(&follow_up.id));

        assert!(category_id_by_name(&store, "#high").is_none());
        assert!(category_id_by_name(&store, "#follow-up").is_none());
    }

    #[test]
    fn create_item_unknown_hashtag_does_not_auto_create_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("Unknown hashtag behavior test #office".to_string());
        let _ = aglet.create_item(&item).unwrap();

        assert!(category_id_by_name(&store, "Office").is_none());
        assert!(category_id_by_name(&store, "#office").is_none());

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.is_empty());
    }

    #[test]
    fn update_item_triggers_reclassification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let urgent = category("Urgent", true);
        store.create_category(&urgent).unwrap();

        let item = Item::new("normal task".to_string());
        aglet.create_item(&item).unwrap();
        assert!(!store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&urgent.id));

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "Urgent task".to_string();
        updated.modified_at = Timestamp::now();

        let result = aglet.update_item(&updated).unwrap();
        assert!(result.new_assignments.contains(&urgent.id));
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&urgent.id));
    }

    #[test]
    fn create_item_parses_date_and_sets_when_provenance() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("next Tuesday at 3pm".to_string());
        aglet
            .create_item_with_reference_date(&item, date(2026, 2, 18))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 24, 15, 0)));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::AutoClassified);
        assert_eq!(when_assignment.origin.as_deref(), Some("nlp:date"));
    }

    #[test]
    fn update_item_parses_new_date_text_and_sets_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("plain task".to_string());
        aglet
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "today at noon".to_string();
        updated.modified_at = Timestamp::now();

        aglet
            .update_item_with_reference_date(&updated, date(2026, 2, 16))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 16, 12, 0)));
    }

    #[test]
    fn update_item_without_parse_does_not_auto_clear_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("tomorrow".to_string());
        aglet
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "just notes now".to_string();
        updated.modified_at = Timestamp::now();

        aglet
            .update_item_with_reference_date(&updated, date(2026, 2, 16))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 17, 0, 0)));
    }

    #[test]
    fn update_item_note_only_does_not_reparse_relative_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let item = Item::new("tomorrow".to_string());
        aglet
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.note = Some("added note text".to_string());
        updated.modified_at = Timestamp::now();

        aglet
            .update_item_with_reference_date(&updated, date(2026, 2, 20))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(
            loaded.when_date,
            Some(datetime(2026, 2, 17, 0, 0)),
            "note-only edits should not reparse relative date text"
        );
    }

    #[test]
    fn set_item_when_date_assigns_reserved_when_with_manual_provenance() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("plain item".to_string());
        store.create_item(&item).unwrap();

        let target_when = datetime(2026, 2, 20, 9, 30);
        aglet
            .set_item_when_date(
                item.id,
                Some(target_when),
                Some("manual:test.when-edit".to_string()),
            )
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(target_when));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::Manual);
        assert_eq!(
            when_assignment.origin.as_deref(),
            Some("manual:test.when-edit")
        );
        assert!(when_assignment.sticky);
    }

    #[test]
    fn set_item_when_date_uses_default_manual_origin_when_none_is_provided() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("plain item".to_string());
        store.create_item(&item).unwrap();

        aglet
            .set_item_when_date(item.id, Some(datetime(2026, 2, 20, 9, 30)), None)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::Manual);
        assert_eq!(when_assignment.origin.as_deref(), Some("manual:when"));
    }

    #[test]
    fn set_item_when_date_none_clears_datetime_and_reserved_when_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("tomorrow".to_string());
        aglet
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&when_id));

        aglet
            .set_item_when_date(item.id, None, Some("manual:test.when-clear".to_string()))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, None);
        assert!(
            !store
                .get_assignments_for_item(item.id)
                .unwrap()
                .contains_key(&when_id),
            "clearing when_date should unassign reserved When"
        );
    }

    #[test]
    fn parsed_when_date_places_item_in_expected_bucket() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let reference_date = date(2026, 2, 16);

        let item = Item::new("today at noon".to_string());
        aglet
            .create_item_with_reference_date(&item, reference_date)
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        let bucket = resolve_when_bucket(loaded.when_date, reference_date);
        assert_eq!(bucket, WhenBucket::Today);
    }

    #[test]
    fn create_category_triggers_retroactive_classification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let sarah_item = Item::new("Sarah's meeting".to_string());
        let bob_item = Item::new("Bob's lunch".to_string());
        store.create_item(&sarah_item).unwrap();
        store.create_item(&bob_item).unwrap();

        let sarah = category("Sarah", true);
        let result = aglet.create_category(&sarah).unwrap();
        assert_eq!(result.processed_items, 2);
        assert_eq!(result.affected_items, 1);

        let sarah_assignments = store.get_assignments_for_item(sarah_item.id).unwrap();
        let bob_assignments = store.get_assignments_for_item(bob_item.id).unwrap();
        assert!(sarah_assignments.contains_key(&sarah.id));
        assert!(!bob_assignments.contains_key(&sarah.id));
    }

    #[test]
    fn update_category_triggers_reclassification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let foo = category("Foo", true);
        aglet.create_category(&foo).unwrap();

        let existing = Item::new("meeting with Foo".to_string());
        aglet.create_item(&existing).unwrap();
        assert!(store
            .get_assignments_for_item(existing.id)
            .unwrap()
            .contains_key(&foo.id));

        let mut renamed = store.get_category(foo.id).unwrap();
        renamed.name = "Bar".to_string();
        let update_result = aglet.update_category(&renamed).unwrap();
        assert_eq!(update_result.processed_items, 1);

        let existing_after = store.get_assignments_for_item(existing.id).unwrap();
        assert!(existing_after.contains_key(&foo.id));

        let new_item = Item::new("meeting with Bar".to_string());
        aglet.create_item(&new_item).unwrap();
        let new_assignments = store.get_assignments_for_item(new_item.id).unwrap();
        assert!(new_assignments.contains_key(&foo.id));
    }

    #[test]
    fn add_category_action_does_not_retroactively_fire_for_existing_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = category("Escalated", false);
        let notify = category("Notify", false);
        aglet.create_category(&source).unwrap();
        aglet.create_category(&notify).unwrap();

        let item = Item::new("Task".to_string());
        aglet.create_item(&item).unwrap();
        aglet.assign_item_manual(item.id, source.id, None).unwrap();

        let (_index, result) = aglet
            .add_category_action(
                source.id,
                Action::Assign {
                    targets: HashSet::from([notify.id]),
                },
            )
            .unwrap();

        assert!(result.processed_items >= 1);
        assert!(!store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&notify.id));
    }

    #[test]
    fn add_category_action_rejects_self_target() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let source = category("Escalated", false);
        aglet.create_category(&source).unwrap();

        let err = aglet
            .add_category_action(
                source.id,
                Action::Assign {
                    targets: HashSet::from([source.id]),
                },
            )
            .unwrap_err();

        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn manual_assignment_triggers_cascade() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let urgent = category("Urgent", false);
        store.create_category(&urgent).unwrap();

        let mut escalated = category("Escalated", false);
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, urgent.id);
        escalated.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        store.create_category(&escalated).unwrap();

        let item = Item::new("Task".to_string());
        store.create_item(&item).unwrap();

        let result = aglet
            .assign_item_manual(item.id, urgent.id, Some("manual:user".to_string()))
            .unwrap();
        assert!(result.new_assignments.contains(&escalated.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&urgent.id).unwrap().source,
            AssignmentSource::Manual
        );
        assert!(assignments.contains_key(&escalated.id));
    }

    #[test]
    fn manual_assignment_applies_subsumption_to_all_ancestors() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();
        let frabulator = child_category("Frabulator", project_y.id, false);
        store.create_category(&frabulator).unwrap();

        let item = Item::new("Talk to Sarah".to_string());
        store.create_item(&item).unwrap();

        aglet
            .assign_item_manual(item.id, frabulator.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments
                .get(&frabulator.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Manual)
        );
        assert_eq!(
            assignments
                .get(&project_y.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn preview_manual_category_toggle_uses_reprocessed_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Talk to Sarah".to_string());
        store.create_item(&item).unwrap();

        let preview = aglet
            .preview_manual_category_toggle(item.id, project_y.id)
            .unwrap();

        assert_eq!(
            preview
                .assignments
                .get(&project_y.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Manual)
        );
        assert_eq!(
            preview
                .assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption),
            "preview should include the same subsumption ancestor the real assign path creates"
        );
        assert!(
            !store
                .get_assignments_for_item(item.id)
                .unwrap()
                .contains_key(&work.id),
            "preview should not mutate the real store"
        );
    }

    #[test]
    fn manual_unassign_blocks_removing_ancestor_while_descendant_assigned() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        let err = aglet.unassign_item_manual(item.id, work.id).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
        let message = err.to_string();
        assert!(message.contains("cannot remove category"));
        assert!(message.contains("Project Y"));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
    }

    #[test]
    fn manual_unassign_removes_live_subsumption_ancestor() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let ancestor = assignments.get(&work.id).unwrap();
        assert_eq!(ancestor.source, AssignmentSource::Subsumption);
        assert!(!ancestor.sticky);

        aglet.unassign_item_manual(item.id, project_y.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&project_y.id));
        assert!(
            !assignments.contains_key(&work.id),
            "subsumption ancestor should auto-break once the supporting descendant is removed"
        );
    }

    #[test]
    fn manual_unassign_allows_removing_leaf_then_parent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        aglet.unassign_item_manual(item.id, project_y.id).unwrap();
        aglet.unassign_item_manual(item.id, work.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&project_y.id));
        assert!(!assignments.contains_key(&work.id));
    }

    #[test]
    fn manual_assignment_enforces_exclusive_siblings() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();

        let high = child_category("High", priority.id, false);
        let medium = child_category("Medium", priority.id, false);
        store.create_category(&high).unwrap();
        store.create_category(&medium).unwrap();

        let item = Item::new("Finish report".to_string());
        store.create_item(&item).unwrap();

        aglet
            .assign_item_manual(item.id, high.id, Some("manual:user".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, medium.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&high.id));
        assert!(assignments.contains_key(&medium.id));
    }

    #[test]
    fn claim_item_manual_rejects_when_precondition_category_is_already_assigned() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut status = category("Status", false);
        status.is_exclusive = true;
        store.create_category(&status).unwrap();
        let in_progress = child_category("In Progress", status.id, false);
        let complete = child_category("Complete", status.id, false);
        store.create_category(&in_progress).unwrap();
        store.create_category(&complete).unwrap();

        let item = Item::new("Task".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, complete.id, Some("manual:test".to_string()))
            .unwrap();

        let err = aglet
            .claim_item_manual(
                item.id,
                in_progress.id,
                &[in_progress.id, complete.id],
                Some("manual:test.claim".to_string()),
            )
            .expect_err("claim should fail when complete is assigned");
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
        let msg = err.to_string();
        assert!(msg.contains("claim precondition failed"));
        assert!(msg.contains("Complete"));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&complete.id));
        assert!(!assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn claim_item_manual_race_allows_only_one_winner() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("aglet-claim-race-{nanos}.ag"));

        let (item_id, ready_id, in_progress_id, complete_id) = {
            let store = Store::open(&db_path).expect("open temp db");
            let classifier = SubstringClassifier;
            let aglet = Aglet::new(&store, &classifier);

            let mut status = category("Status", false);
            status.is_exclusive = true;
            store.create_category(&status).expect("create status");
            let ready = child_category("Ready", status.id, false);
            let in_progress = child_category("In Progress", status.id, false);
            let complete = child_category("Complete", status.id, false);
            store.create_category(&ready).expect("create ready");
            store
                .create_category(&in_progress)
                .expect("create in progress");
            store.create_category(&complete).expect("create complete");

            let item = Item::new("Concurrent claim target".to_string());
            store.create_item(&item).expect("create item");
            aglet
                .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
                .expect("assign ready");
            (item.id, ready.id, in_progress.id, complete.id)
        };

        let barrier = Arc::new(Barrier::new(2));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let db_path = db_path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let store = Store::open(&db_path).expect("open raced store");
                let classifier = SubstringClassifier;
                let aglet = Aglet::new(&store, &classifier);
                barrier.wait();
                aglet
                    .claim_item_manual(
                        item_id,
                        in_progress_id,
                        &[in_progress_id, complete_id],
                        Some("manual:test.claim".to_string()),
                    )
                    .map(|_| ())
                    .map_err(|err| err.to_string())
            }));
        }

        let outcomes: Vec<Result<(), String>> = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread result"))
            .collect();

        let success_count = outcomes.iter().filter(|result| result.is_ok()).count();
        assert_eq!(success_count, 1, "exactly one claim should succeed");
        let failure_messages: Vec<&str> = outcomes
            .iter()
            .filter_map(|result| result.as_ref().err().map(String::as_str))
            .collect();
        assert_eq!(failure_messages.len(), 1);
        assert!(
            failure_messages[0].contains("claim precondition failed"),
            "expected precondition failure, got: {}",
            failure_messages[0]
        );

        let verify_store = Store::open(&db_path).expect("open verify store");
        let assignments = verify_store
            .get_assignments_for_item(item_id)
            .expect("load assignments");
        assert!(assignments.contains_key(&in_progress_id));
        assert!(!assignments.contains_key(&ready_id));
        assert!(!assignments.contains_key(&complete_id));

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
    }

    #[test]
    fn assign_item_numeric_manual_sets_payload_and_subsumption_ancestor_has_none() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let project = category("Project", false);
        store.create_category(&project).unwrap();
        let mut cost = child_category("Cost", project.id, false);
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).unwrap();

        let item = Item::new("Vendor invoice".to_string());
        store.create_item(&item).unwrap();

        aglet
            .assign_item_numeric_manual(
                item.id,
                cost.id,
                Decimal::new(24596, 2),
                Some("manual:test".to_string()),
            )
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&cost.id).and_then(|a| a.numeric_value),
            Some(Decimal::new(24596, 2))
        );
        assert_eq!(
            assignments.get(&project.id).and_then(|a| a.numeric_value),
            None
        );
        assert_eq!(
            assignments.get(&project.id).map(|a| a.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn assign_item_numeric_manual_rejects_non_numeric_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let tag = category("TagOnly", false);
        store.create_category(&tag).unwrap();

        let item = Item::new("Test".to_string());
        store.create_item(&item).unwrap();

        let err = aglet
            .assign_item_numeric_manual(item.id, tag.id, Decimal::new(10, 0), None)
            .unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
        assert!(err.to_string().contains("not Numeric"));
    }

    #[test]
    fn manual_assignment_rejects_duplicate_category_names() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_x = child_category("Project X", work.id, false);
        store.create_category(&project_x).unwrap();

        let mut work_priority = child_category("Priority", work.id, false);
        work_priority.is_exclusive = true;
        aglet.create_category(&work_priority).unwrap();

        let mut project_priority = child_category("Priority", project_x.id, false);
        project_priority.is_exclusive = true;
        let err = aglet.create_category(&project_priority).unwrap_err();
        assert!(matches!(err, AgletError::DuplicateName { .. }));
    }

    #[test]
    fn manual_assignment_enforces_exclusivity_per_priority_branch() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_x = child_category("Project X", work.id, false);
        store.create_category(&project_x).unwrap();

        let mut work_priority = child_category("Priority", work.id, false);
        work_priority.is_exclusive = true;
        store.create_category(&work_priority).unwrap();
        let work_high = child_category("High", work_priority.id, false);
        let work_medium = child_category("Medium", work_priority.id, false);
        store.create_category(&work_high).unwrap();
        store.create_category(&work_medium).unwrap();

        let mut project_priority = child_category("Project X Priority", project_x.id, false);
        project_priority.is_exclusive = true;
        store.create_category(&project_priority).unwrap();
        let project_high = child_category("Project X High", project_priority.id, false);
        let project_medium = child_category("Project X Medium", project_priority.id, false);
        store.create_category(&project_high).unwrap();
        store.create_category(&project_medium).unwrap();

        let item = Item::new("Prepare sprint plan".to_string());
        store.create_item(&item).unwrap();

        aglet
            .assign_item_manual(item.id, work_high.id, Some("manual:user".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, project_high.id, Some("manual:user".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, work_medium.id, Some("manual:user".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, project_medium.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work_high.id));
        assert!(assignments.contains_key(&work_medium.id));
        assert!(!assignments.contains_key(&project_high.id));
        assert!(assignments.contains_key(&project_medium.id));
    }

    #[test]
    fn engine_error_does_not_prevent_store_mutation() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut stages = Vec::new();
        for index in 1..=11 {
            let stage = category(&format!("Stage{index}"), false);
            store.create_category(&stage).unwrap();
            stages.push(stage);
        }

        for index in 0..10 {
            let mut stage = store.get_category(stages[index].id).unwrap();
            let mut criteria = Query::default();
            criteria.set_criterion(CriterionMode::And, stages[index + 1].id);
            stage.conditions = vec![Condition::Profile {
                criteria: Box::new(criteria),
            }];
            store.update_category(&stage).unwrap();
        }

        let mut trigger = category("Trigger", true);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([stages[10].id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("Trigger this chain".to_string());
        let err = aglet.create_item(&item).unwrap_err();
        match err {
            AgletError::InvalidOperation { message } => {
                assert!(message.contains("exceeded 10 passes"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.text, "Trigger this chain");
    }

    #[test]
    fn manual_assignment_fires_category_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([archive.id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("plain unrelated text".to_string());
        aglet.create_item(&item).unwrap();

        let result = aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&trigger.id));
        assert!(
            assignments.contains_key(&archive.id),
            "actions fire on manual assignment, per the Agenda paradigm"
        );
        assert_eq!(
            assignments.get(&archive.id).unwrap().source,
            AssignmentSource::Action
        );
        assert!(result.new_assignments.contains(&archive.id));
    }

    #[test]
    fn manual_numeric_assignment_fires_category_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let big_ticket = category("Big ticket", false);
        store.create_category(&big_ticket).unwrap();

        let mut cost = category("Cost", false);
        cost.value_kind = CategoryValueKind::Numeric;
        cost.actions.push(Action::Assign {
            targets: HashSet::from([big_ticket.id]),
        });
        store.create_category(&cost).unwrap();

        let item = Item::new("buy a boat".to_string());
        aglet.create_item(&item).unwrap();

        aglet
            .assign_item_numeric_manual(item.id, cost.id, Decimal::new(500, 0), None)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&big_ticket.id),
            "numeric manual assignment is an assignment event"
        );
    }

    #[test]
    fn mark_item_done_fires_done_category_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let done_id = category_id_by_name(&store, RESERVED_CATEGORY_NAME_DONE).unwrap();
        let mut done = store.get_category(done_id).unwrap();
        done.actions.push(Action::Assign {
            targets: HashSet::from([archive.id]),
        });
        store.update_category(&done).unwrap();

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("finish the report".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        aglet.mark_item_done(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&archive.id),
            "Done category actions fire when the user marks an item done"
        );
    }

    #[test]
    fn action_chain_cascades_through_targets_on_create() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let charlie = category("Charlie", false);
        store.create_category(&charlie).unwrap();

        let mut bravo = category("Bravo", false);
        bravo.actions.push(Action::Assign {
            targets: HashSet::from([charlie.id]),
        });
        store.create_category(&bravo).unwrap();

        let mut alpha = category("Alpha", true);
        alpha.actions.push(Action::Assign {
            targets: HashSet::from([bravo.id]),
        });
        store.create_category(&alpha).unwrap();

        let item = Item::new("alpha task".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&alpha.id));
        assert!(assignments.contains_key(&bravo.id));
        assert!(
            assignments.contains_key(&charlie.id),
            "an action-created assignment is itself an assignment event; chains cascade"
        );
    }

    #[test]
    fn manual_assignment_action_chain_cascades() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let charlie = category("Charlie", false);
        store.create_category(&charlie).unwrap();

        let mut bravo = category("Bravo", false);
        bravo.actions.push(Action::Assign {
            targets: HashSet::from([charlie.id]),
        });
        store.create_category(&bravo).unwrap();

        let mut alpha = category("Alpha", false);
        alpha.actions.push(Action::Assign {
            targets: HashSet::from([bravo.id]),
        });
        store.create_category(&alpha).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, alpha.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&bravo.id));
        assert!(assignments.contains_key(&charlie.id));
    }

    #[test]
    fn reassigning_existing_category_does_not_refire_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([archive.id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();
        aglet.unassign_item_manual(item.id, archive.id).unwrap();

        // Re-assigning the already-assigned trigger is not a new assignment
        // event, so its actions must not re-fire.
        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&archive.id),
            "actions are edge-triggered: no new assignment event, no re-fire"
        );
    }

    #[test]
    fn item_edit_does_not_refire_actions_for_existing_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([archive.id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();
        aglet.unassign_item_manual(item.id, archive.id).unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "edited task".to_string();
        updated.modified_at = Timestamp::now();
        aglet.update_item(&updated).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&archive.id),
            "editing an item must not re-fire actions for assignments that already existed"
        );
    }

    #[test]
    fn remove_action_clears_manual_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let inbox = category("Inbox", false);
        store.create_category(&inbox).unwrap();

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::Remove {
            targets: HashSet::from([inbox.id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, inbox.id, Some("manual:test".to_string()))
            .unwrap();

        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&inbox.id),
            "Remove actions clear manual assignments too (product decision #45)"
        );
    }

    #[test]
    fn manual_unassign_of_automatch_sticks_via_veto() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let meetings = category("Meetings", true);
        store.create_category(&meetings).unwrap();

        let item = Item::new("Team meetings tomorrow".to_string());
        aglet.create_item(&item).unwrap();
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&meetings.id));

        aglet.unassign_item_manual(item.id, meetings.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&meetings.id),
            "manual unassign records a veto; the engine must not re-assign"
        );
        assert!(store
            .get_vetoes_for_item(item.id)
            .unwrap()
            .contains(&meetings.id));

        // Even a later text edit (full re-match) must respect the veto.
        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "Team meetings moved to Friday".to_string();
        updated.modified_at = Timestamp::now();
        aglet.update_item(&updated).unwrap();
        assert!(!store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&meetings.id));
    }

    #[test]
    fn manual_reassign_clears_veto() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let meetings = category("Meetings", true);
        store.create_category(&meetings).unwrap();

        let item = Item::new("Team meetings tomorrow".to_string());
        aglet.create_item(&item).unwrap();
        aglet.unassign_item_manual(item.id, meetings.id).unwrap();
        assert!(!store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&meetings.id));

        aglet
            .assign_item_manual(item.id, meetings.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&meetings.id));
        assert!(
            store.get_vetoes_for_item(item.id).unwrap().is_empty(),
            "manual re-assignment clears the veto"
        );
    }

    #[test]
    fn veto_blocks_action_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([archive.id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        store
            .add_assignment_veto(item.id, archive.id, Some("manual:test"))
            .unwrap();

        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&trigger.id));
        assert!(
            !assignments.contains_key(&archive.id),
            "vetoed categories are not assignable by actions"
        );
    }

    #[test]
    fn removing_manual_assignment_records_no_veto() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();
        aglet.unassign_item_manual(item.id, work.id).unwrap();

        assert!(
            store.get_vetoes_for_item(item.id).unwrap().is_empty(),
            "removing one's own manual assignment is symmetric, not a veto"
        );
    }

    #[test]
    fn veto_does_not_block_subsumption() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let projects = category("Projects", false);
        store.create_category(&projects).unwrap();
        let alpha = child_category("Alpha", projects.id, false);
        store.create_category(&alpha).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        store
            .add_assignment_veto(item.id, projects.id, Some("manual:test"))
            .unwrap();

        aglet
            .assign_item_manual(item.id, alpha.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&projects.id),
            "an assigned descendant still implies its ancestors"
        );
    }

    #[test]
    fn assign_numeric_action_assigns_with_value() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cost = category("Cost", false);
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).unwrap();

        let mut trigger = category("Standard order", false);
        trigger.actions.push(Action::AssignNumeric {
            target: cost.id,
            value: Decimal::new(100, 0),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("order the usual".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let cost_assignment = assignments
            .get(&cost.id)
            .expect("numeric action should assign the target");
        assert_eq!(cost_assignment.numeric_value, Some(Decimal::new(100, 0)));
        assert_eq!(cost_assignment.source, AssignmentSource::Action);
    }

    #[test]
    fn set_when_action_stamps_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut this_week = category("Schedule soon", false);
        this_week.actions.push(Action::SetWhen {
            value: aglet_core_test_date_expr_tomorrow(),
        });
        store.create_category(&this_week).unwrap();

        let item = Item::new("plain task".to_string());
        aglet.create_item(&item).unwrap();
        assert!(store.get_item(item.id).unwrap().when_date.is_none());

        aglet
            .assign_item_manual(item.id, this_week.id, Some("manual:test".to_string()))
            .unwrap();

        let after = store.get_item(item.id).unwrap();
        let when = after.when_date.expect("SetWhen action stamps the When date");
        let tomorrow = jiff::Zoned::now()
            .date()
            .checked_add(jiff::Span::new().days(1))
            .unwrap();
        assert_eq!(when.date(), tomorrow);
    }

    #[test]
    fn mark_done_action_marks_item_done() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut archive = category("Archive", false);
        archive.actions.push(Action::MarkDone);
        store.create_category(&archive).unwrap();

        let item = Item::new("finished thing".to_string());
        aglet.create_item(&item).unwrap();

        aglet
            .assign_item_manual(item.id, archive.id, Some("manual:test".to_string()))
            .unwrap();

        let after = store.get_item(item.id).unwrap();
        assert!(after.is_done, "MarkDone action marks the item done");
        assert!(after.done_date.is_some());
    }

    #[test]
    fn delete_action_requires_allow_flag_and_logs_deletion() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let trash = category("Trash", false);
        store.create_category(&trash).unwrap();

        // Attaching Delete without the flag is rejected.
        let err = aglet
            .add_category_action(trash.id, Action::Delete)
            .unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));

        let mut trash = store.get_category(trash.id).unwrap();
        trash.allow_delete_action = true;
        store.update_category(&trash).unwrap();
        aglet
            .add_category_action(trash.id, Action::Delete)
            .unwrap();

        let item = Item::new("ephemeral note".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, trash.id, Some("manual:test".to_string()))
            .unwrap();

        assert!(
            store.get_item(item.id).is_err(),
            "Delete action removes the item"
        );
        let log = store.list_deleted_items().unwrap();
        let entry = log
            .iter()
            .find(|entry| entry.item_id == item.id)
            .expect("deletion is logged");
        assert_eq!(entry.deleted_by, "action:Trash");
        assert_eq!(entry.text, "ephemeral note");
    }

    #[test]
    fn special_action_cascades_terminate() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        // Done itself carries a MarkDone action: firing it on an already-done
        // item must be a no-op, not an infinite loop.
        let done_id = category_id_by_name(&store, RESERVED_CATEGORY_NAME_DONE).unwrap();
        let mut done = store.get_category(done_id).unwrap();
        done.actions.push(Action::MarkDone);
        store.update_category(&done).unwrap();

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("loop bait".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        aglet.mark_item_done(item.id).unwrap();
        let after = store.get_item(item.id).unwrap();
        assert!(after.is_done);
    }

    fn aglet_core_test_date_expr_tomorrow() -> crate::model::DateValueExpr {
        crate::model::DateValueExpr::Tomorrow
    }

    #[test]
    fn set_when_cascade_delete_does_not_error_pending_specials() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        // Doomed: date condition matching today, carrying a Delete action.
        let mut doomed = category("Doomed", false);
        doomed.allow_delete_action = true;
        doomed.conditions.push(Condition::Date {
            source: aglet_core_test_when_source(),
            matcher: crate::model::DateMatcher::Compare {
                op: crate::model::DateCompareOp::AtOrAfter,
                value: crate::model::DateValueExpr::Today,
            },
        });
        doomed.actions.push(Action::Delete);
        store.create_category(&doomed).unwrap();

        // Trigger stamps When (making Doomed match in the follow-up cascade)
        // AND requests MarkDone — which must not error after the nested
        // cascade has already deleted the item.
        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::SetWhen {
            value: crate::model::DateValueExpr::Tomorrow,
        });
        trigger.actions.push(Action::MarkDone);
        store.create_category(&trigger).unwrap();

        let item = Item::new("short-lived task".to_string());
        aglet.create_item(&item).unwrap();

        aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .expect("pending MarkDone/Delete specials on a deleted item must not error");

        assert!(
            store.get_item(item.id).is_err(),
            "item was deleted by the cascade"
        );
        let log = store.list_deleted_items().unwrap();
        assert!(
            log.iter().any(|entry| entry.item_id == item.id),
            "cascade deletion is logged"
        );
    }

    fn aglet_core_test_when_source() -> crate::model::DateSource {
        crate::model::DateSource::When
    }

    #[test]
    fn category_change_bulk_run_applies_special_effects() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let archive = category("Archive", false);
        store.create_category(&archive).unwrap();

        let item = Item::new("finish the report".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();
        assert!(!store.get_item(item.id).unwrap().is_done);

        // Adding the rule triggers a bulk reevaluation; the new Archive
        // assignment fires MarkDone, whose effect must actually apply.
        let mut archive = store.get_category(archive.id).unwrap();
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, work.id);
        archive.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        archive.actions.push(Action::MarkDone);
        aglet.update_category(&archive).unwrap();

        let after = store.get_item(item.id).unwrap();
        assert!(
            after.assignments.contains_key(&archive.id),
            "bulk run assigns Archive"
        );
        assert!(
            after.is_done,
            "bulk-run action effects (MarkDone) are applied, not dropped"
        );
    }

    #[test]
    fn special_cascade_depth_cap_reports_warning() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let date_cat = |name: &str, day: i32, next_day: i32| {
            let mut cat = category(name, false);
            cat.conditions.push(Condition::Date {
                source: crate::model::DateSource::When,
                matcher: crate::model::DateMatcher::Compare {
                    op: crate::model::DateCompareOp::On,
                    value: crate::model::DateValueExpr::DaysFromToday(day),
                },
            });
            cat.actions.push(Action::SetWhen {
                value: crate::model::DateValueExpr::DaysFromToday(next_day),
            });
            cat
        };

        let mut trigger = category("Trigger", false);
        trigger.actions.push(Action::SetWhen {
            value: crate::model::DateValueExpr::DaysFromToday(1),
        });
        store.create_category(&trigger).unwrap();
        store.create_category(&date_cat("Hop1", 1, 2)).unwrap();
        store.create_category(&date_cat("Hop2", 2, 3)).unwrap();
        store.create_category(&date_cat("Hop3", 3, 4)).unwrap();
        store.create_category(&date_cat("Hop4", 4, 5)).unwrap();

        let item = Item::new("cascade bait".to_string());
        aglet.create_item(&item).unwrap();

        let result = aglet
            .assign_item_manual(item.id, trigger.id, Some("manual:test".to_string()))
            .unwrap();

        assert!(
            !result.warnings.is_empty(),
            "depth-capped special effects must surface a warning, got none"
        );
        assert!(
            result.warnings[0].contains("cut short"),
            "warning text: {:?}",
            result.warnings
        );
    }

    #[test]
    fn rejecting_suggestion_records_veto() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let meetings = category("Meetings", false);
        store.create_category(&meetings).unwrap();

        let item = Item::new("plan the offsite".to_string());
        aglet.create_item(&item).unwrap();

        let suggestion = crate::classification::ClassificationSuggestion {
            id: uuid::Uuid::new_v4(),
            item_id: item.id,
            assignment: crate::classification::CandidateAssignment::Category(meetings.id),
            provider_id: "test-provider".to_string(),
            model: None,
            confidence: None,
            rationale: None,
            context_hash: "test".to_string(),
            item_revision_hash: "test".to_string(),
            status: SuggestionStatus::Pending,
            created_at: Timestamp::now(),
            decided_at: None,
        };
        store.upsert_suggestion(&suggestion).unwrap();

        aglet.reject_classification_suggestion(suggestion.id).unwrap();

        assert!(
            store
                .get_vetoes_for_item(item.id)
                .unwrap()
                .contains(&meetings.id),
            "rejection records a veto (product decision #46)"
        );

        // The vetoed category must not come back via text matching either.
        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "plan the meetings offsite".to_string();
        updated.modified_at = Timestamp::now();
        aglet.update_item(&updated).unwrap();
        assert!(
            !store
                .get_assignments_for_item(item.id)
                .unwrap()
                .contains_key(&meetings.id),
            "veto blocks literal auto-apply after rejection"
        );
    }

    #[test]
    fn numeric_reassignment_updates_value_through_intent_upgrade() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut cost = category("Cost", false);
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).unwrap();

        let item = Item::new("estimate".to_string());
        aglet.create_item(&item).unwrap();

        aglet
            .assign_item_numeric_manual(item.id, cost.id, Decimal::new(100, 0), None)
            .unwrap();
        let result = aglet
            .assign_item_numeric_manual(item.id, cost.id, Decimal::new(200, 0), None)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&cost.id).unwrap().numeric_value,
            Some(Decimal::new(200, 0)),
            "intent upgrade persists the changed value"
        );
        assert!(
            !result
                .assignment_events
                .iter()
                .any(|event| event.category_id == cost.id),
            "re-assignment is not a new assignment event"
        );
    }

    #[test]
    fn end_to_end_workflow_runs_automatically() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let events = category("Events", false);
        aglet.create_category(&events).unwrap();

        let calendar = child_category("Calendar", events.id, false);
        aglet.create_category(&calendar).unwrap();

        let mut meetings = category("Meetings", true);
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        aglet.create_category(&meetings).unwrap();

        let item = Item::new("Team meetings tomorrow".to_string());
        aglet.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&meetings.id).unwrap().source,
            AssignmentSource::AutoClassified
        );
        assert_eq!(
            assignments.get(&calendar.id).unwrap().source,
            AssignmentSource::Action
        );
        assert_eq!(
            assignments.get(&events.id).unwrap().source,
            AssignmentSource::Subsumption
        );
    }

    #[test]
    fn insert_item_in_section_assigns_section_and_view_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section.on_insert_assign.insert(urgent.id);

        aglet
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&urgent.id));
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
        assert_eq!(
            assignments
                .get(&urgent.id)
                .and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
    }

    #[test]
    fn insert_item_in_section_assigns_section_criteria_include_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let p0 = category("P0", false);
        store.create_category(&p0).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let current_view = view("Board");
        let mut current_section = section("P0");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, p0.id);

        aglet
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&p0.id));
        assert_eq!(
            assignments.get(&p0.id).and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
    }

    #[test]
    fn insert_item_in_section_triggers_engine_cascade() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();

        let mut escalated = category("Escalated", false);
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, work.id);
        criteria.set_criterion(CriterionMode::And, urgent.id);
        escalated.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        store.create_category(&escalated).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section.on_insert_assign.insert(urgent.id);

        let result = aglet
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        assert!(result.new_assignments.contains(&escalated.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&escalated.id));
    }

    #[test]
    fn insert_item_in_section_applies_subsumption_for_manual_section_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("Project Y Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, project_y.id);
        let mut current_section = section("Project Y");
        current_section.on_insert_assign.insert(project_y.id);

        aglet
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn move_between_sections_uses_structural_diff_without_manual_remove_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let ready = category("Ready", false);
        let in_progress = category("In Progress", false);
        let personal = category("Personal", false);
        for category in [&work, &ready, &in_progress, &personal] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [work.id, ready.id, personal.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut current_view = view("Work Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut source_section = section("Ready");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, ready.id);
        let mut target_section = section("In Progress");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, in_progress.id);

        aglet
            .move_item_between_sections(item.id, &current_view, &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&ready.id));
        assert!(assignments.contains_key(&in_progress.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn move_between_sections_honors_on_remove_side_effects() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = category("Ready", false);
        let in_progress = category("In Progress", false);
        let needs_review = category("Needs Review", false);
        for category in [&ready, &in_progress, &needs_review] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [ready.id, needs_review.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Ready");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, ready.id);
        source_section.on_remove_unassign.insert(needs_review.id);

        let mut target_section = section("In Progress");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, in_progress.id);

        aglet
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&ready.id));
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&needs_review.id));
    }

    #[test]
    fn move_between_sections_preserves_overlapping_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let shared = category("Shared", false);
        let urgent = category("Urgent", false);
        let next = category("Next", false);
        for category in [&shared, &urgent, &next] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [shared.id, urgent.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Urgent");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, shared.id);
        source_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);

        let mut target_section = section("Next");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, shared.id);
        target_section
            .criteria
            .set_criterion(CriterionMode::And, next.id);

        aglet
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&shared.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&next.id));
    }

    #[test]
    fn move_between_generated_subsections_swaps_child_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let parent = category("Project", false);
        store.create_category(&parent).unwrap();
        let alpha = child_category("Alpha", parent.id, false);
        let beta = child_category("Beta", parent.id, false);
        store.create_category(&alpha).unwrap();
        store.create_category(&beta).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [parent.id, alpha.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Project");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, parent.id);
        source_section.on_insert_assign.insert(alpha.id);

        let mut target_section = section("Project");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, parent.id);
        target_section.on_insert_assign.insert(beta.id);

        aglet
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&parent.id));
        assert!(!assignments.contains_key(&alpha.id));
        assert!(assignments.contains_key(&beta.id));
    }

    #[test]
    fn remove_from_section_preserves_view_criteria_and_strips_structural_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, urgent.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);

        aglet
            .remove_item_from_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn remove_from_section_honors_on_remove_side_effects() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        let lane_marker = category("Lane Marker", false);
        let review_flag = category("Needs Review", false);
        for category in [&work, &urgent, &lane_marker, &review_flag] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [work.id, urgent.id, lane_marker.id, review_flag.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut current_view = view("Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);
        current_section.on_insert_assign.insert(lane_marker.id);
        current_section.on_remove_unassign.insert(review_flag.id);

        aglet
            .remove_item_from_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(!assignments.contains_key(&lane_marker.id));
        assert!(!assignments.contains_key(&review_flag.id));
    }

    #[test]
    fn remove_from_view_unassigns_view_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("My Work");
        current_view.remove_from_view_unassign.insert(work.id);

        aglet.remove_item_from_view(item.id, &current_view).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn unmatched_insert_uses_view_criteria_include() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);

        aglet
            .insert_item_in_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("edit:view.insert")
        );
    }

    #[test]
    fn unmatched_insert_applies_subsumption_for_view_include_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("Project Y Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, project_y.id);

        aglet
            .insert_item_in_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn unmatched_remove_uses_view_remove_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("My Work");
        current_view.remove_from_view_unassign.insert(work.id);

        aglet
            .remove_item_from_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn insert_item_in_section_is_idempotent_for_existing_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:user".to_string()))
            .unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Work");
        current_section.on_insert_assign.insert(work.id);

        aglet
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("manual:user")
        );
    }

    #[test]
    fn remove_from_view_triggers_engine_even_with_no_unassign_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let trigger = category("Trigger", true);
        store.create_category(&trigger).unwrap();

        let item = Item::new("trigger task".to_string());
        store.create_item(&item).unwrap();

        let current_view = view("Any");
        aglet.remove_item_from_view(item.id, &current_view).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&trigger.id));
    }

    #[test]
    fn db_backed_setup_with_items_categories_views_and_assignments_resolves_filters() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();

        let mut project_atlas = child_category("Project Atlas", work.id, true);
        project_atlas.enable_implicit_string = true;
        aglet.create_category(&project_atlas).unwrap();

        let mut miguel = child_category("Miguel", work.id, true);
        miguel.enable_implicit_string = true;
        aglet.create_category(&miguel).unwrap();

        let mut alice = child_category("Alice", work.id, true);
        alice.enable_implicit_string = true;
        aglet.create_category(&alice).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        aglet.create_category(&priority).unwrap();
        let high = child_category("High", priority.id, false);
        aglet.create_category(&high).unwrap();

        let collaborative = Item::new(
            "Project Atlas: Miguel and Alice triage defects tomorrow at noon".to_string(),
        );
        aglet.create_item(&collaborative).unwrap();
        aglet
            .assign_item_manual(collaborative.id, high.id, Some("manual:test".to_string()))
            .unwrap();

        let solo = Item::new("Project Atlas: Miguel draft rollout checklist".to_string());
        aglet.create_item(&solo).unwrap();
        aglet
            .assign_item_manual(solo.id, high.id, Some("manual:test".to_string()))
            .unwrap();

        let collaborative_assignments = store.get_assignments_for_item(collaborative.id).unwrap();
        assert!(collaborative_assignments.contains_key(&project_atlas.id));
        assert!(collaborative_assignments.contains_key(&work.id));
        assert!(collaborative_assignments.contains_key(&miguel.id));
        assert!(collaborative_assignments.contains_key(&alice.id));
        assert!(collaborative_assignments.contains_key(&high.id));

        let mut view = view("Miguel Without Alice");
        view.criteria.set_criterion(CriterionMode::And, work.id);
        view.criteria.set_criterion(CriterionMode::And, miguel.id);
        view.criteria.set_criterion(CriterionMode::Not, alice.id);
        store.create_view(&view).unwrap();

        let persisted_view = store.get_view(view.id).unwrap();
        let items = store.list_items().unwrap();
        let categories = store.get_hierarchy().unwrap();
        let result = resolve_view(&persisted_view, &items, &categories, date(2026, 2, 16));

        assert!(result.sections.is_empty());
        let unmatched = result.unmatched.expect("unmatched group is enabled");
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].id, solo.id);
    }

    #[test]
    fn mark_item_done_sets_done_fields_and_assigns_done_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let item = Item::new("Ship SLC".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let _result = aglet.mark_item_done(item.id).unwrap();
        let loaded = store.get_item(item.id).unwrap();
        assert!(loaded.is_done);
        assert!(loaded.done_date.is_some());

        let done_category_id = store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_DONE)
            })
            .expect("Done category exists")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&done_category_id));
        assert_eq!(
            assignments
                .get(&done_category_id)
                .and_then(|assignment| assignment.origin.as_deref()),
            Some("manual:done")
        );
    }

    #[test]
    fn claim_item_workflow_assigns_claim_target_for_ready_item() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = category("Ready", false);
        store.create_category(&ready).unwrap();
        let in_progress = category("In Progress", false);
        store.create_category(&in_progress).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Claim me".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();

        aglet.claim_item_workflow(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn claim_item_workflow_honors_exclusive_status_parent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut status = category("Status", false);
        status.is_exclusive = true;
        store.create_category(&status).unwrap();

        let ready = child_category("Ready", status.id, false);
        store.create_category(&ready).unwrap();
        let in_progress = child_category("In Progress", status.id, false);
        store.create_category(&in_progress).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Claim me exclusively".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();

        aglet.claim_item_workflow(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&ready.id));
    }

    #[test]
    fn mark_item_done_clears_workflow_claim_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = category("Ready", false);
        store.create_category(&ready).unwrap();
        let in_progress = category("In Progress", false);
        store.create_category(&in_progress).unwrap();
        let work = category("Work", false);
        store.create_category(&work).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Finish me".to_string());
        store.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();
        aglet.claim_item_workflow(item.id).unwrap();

        aglet.mark_item_done(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn mark_item_done_rejects_non_actionable_only_items() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let mut reference = category("Reference", false);
        reference.is_actionable = false;
        aglet.create_category(&reference).unwrap();

        let item = Item::new("Read policy document".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, reference.id, Some("manual:test".to_string()))
            .unwrap();

        let err = aglet.mark_item_done(item.id).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn toggle_item_done_unsets_done_state_and_done_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();

        let item = Item::new("Ship SLC".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        aglet.toggle_item_done(item.id).unwrap();
        assert!(store.get_item(item.id).unwrap().is_done);

        aglet.toggle_item_done(item.id).unwrap();
        let loaded = store.get_item(item.id).unwrap();
        assert!(!loaded.is_done);
        assert!(loaded.done_date.is_none());

        let done_category_id = store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_DONE)
            })
            .expect("Done category exists")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&done_category_id));
    }

    #[test]
    fn move_category_to_parent_reparents_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let left = category("Left", false);
        let right = category("Right", false);
        aglet.create_category(&left).unwrap();
        aglet.create_category(&right).unwrap();

        let child = child_category("Child", left.id, false);
        aglet.create_category(&child).unwrap();

        let result = aglet
            .move_category_to_parent(child.id, Some(right.id), None)
            .unwrap();
        assert!(result.processed_items >= result.affected_items);
        assert_eq!(store.get_category(child.id).unwrap().parent, Some(right.id));
    }

    #[test]
    fn move_category_within_parent_reorders_children() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let parent = category("Parent", false);
        aglet.create_category(&parent).unwrap();
        let alpha = child_category("Alpha", parent.id, false);
        let beta = child_category("Beta", parent.id, false);
        aglet.create_category(&alpha).unwrap();
        aglet.create_category(&beta).unwrap();

        aglet.move_category_within_parent(beta.id, -1).unwrap();

        let loaded_parent = store.get_category(parent.id).unwrap();
        assert_eq!(loaded_parent.children, vec![beta.id, alpha.id]);
    }

    #[test]
    fn link_items_depends_on_rejects_self_link() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let item_id = make_item(&store, "A");

        let err = aglet.link_items_depends_on(item_id, item_id).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_related_rejects_self_link() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let item_id = make_item(&store, "A");

        let err = aglet.link_items_related(item_id, item_id).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_stores_inverse_depends_on_edge() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let blocker = make_item(&store, "Blocker");
        let blocked = make_item(&store, "Blocked");

        let result = aglet.link_items_blocks(blocker, blocked).unwrap();
        assert!(result.created);
        assert!(store
            .item_link_exists(blocked, blocker, ItemLinkKind::DependsOn)
            .unwrap());
        assert_eq!(
            aglet.immediate_dependent_ids(blocker).unwrap(),
            vec![blocked]
        );
        assert_eq!(aglet.immediate_prereq_ids(blocked).unwrap(), vec![blocker]);
    }

    #[test]
    fn link_items_related_normalizes_pair_and_is_idempotent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let (low, high) = if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        };

        let first = aglet.link_items_related(high, low).unwrap();
        let second = aglet.link_items_related(low, high).unwrap();

        assert!(first.created);
        assert!(!second.created);
        assert!(store
            .item_link_exists(low, high, ItemLinkKind::Related)
            .unwrap());

        let count: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM item_links WHERE kind = 'related'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn link_items_depends_on_rejects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        aglet.link_items_depends_on(a, b).unwrap();
        let err = aglet.link_items_depends_on(b, a).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_depends_on_rejects_longer_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        aglet.link_items_depends_on(a, b).unwrap();
        aglet.link_items_depends_on(b, c).unwrap();
        let err = aglet.link_items_depends_on(c, a).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_rejects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        aglet.link_items_blocks(a, b).unwrap();
        let err = aglet.link_items_blocks(b, a).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_rejects_longer_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        aglet.link_items_blocks(a, b).unwrap();
        aglet.link_items_blocks(b, c).unwrap();
        let err = aglet.link_items_blocks(c, a).unwrap_err();
        assert!(matches!(err, AgletError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_related_allows_triangle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        assert!(aglet.link_items_related(a, b).unwrap().created);
        assert!(aglet.link_items_related(b, c).unwrap().created);
        assert!(aglet.link_items_related(c, a).unwrap().created);

        let links_a = aglet.immediate_related_ids(a).unwrap();
        let links_b = aglet.immediate_related_ids(b).unwrap();
        let links_c = aglet.immediate_related_ids(c).unwrap();
        assert_eq!(links_a.len(), 2);
        assert_eq!(links_b.len(), 2);
        assert_eq!(links_c.len(), 2);
    }

    #[test]
    fn unlink_items_blocks_and_related_are_idempotent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        aglet.link_items_blocks(a, b).unwrap();
        aglet.link_items_related(a, b).unwrap();

        aglet.unlink_items_blocks(a, b).unwrap();
        aglet.unlink_items_related(a, b).unwrap();
        // idempotent delete behavior delegated to Store
        aglet.unlink_items_blocks(a, b).unwrap();
        aglet.unlink_items_related(a, b).unwrap();

        assert!(aglet.immediate_dependent_ids(a).unwrap().is_empty());
        assert!(aglet.immediate_prereq_ids(b).unwrap().is_empty());
        assert!(aglet.immediate_related_ids(a).unwrap().is_empty());
        assert!(aglet.immediate_related_ids(b).unwrap().is_empty());
    }

    #[test]
    fn immediate_links_for_item_groups_prereqs_blocks_and_related() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");
        let d = make_item(&store, "D");

        aglet.link_items_depends_on(a, b).unwrap();
        aglet.link_items_blocks(a, c).unwrap(); // c depends-on a
        aglet.link_items_related(a, d).unwrap();

        let links = aglet.immediate_links_for_item(a).unwrap();
        assert_eq!(links.depends_on, vec![b]);
        assert_eq!(links.blocks, vec![c]);
        assert_eq!(links.related, vec![d]);
    }

    // ── normalize_related_pair ─────────────────────────────────────────────────

    #[test]
    fn normalize_related_pair_returns_lexicographic_order() {
        use uuid::Uuid;
        let a: ItemId = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let b: ItemId = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();

        // a < b lexicographically, so (a, b) should be unchanged.
        let (lo, hi) = Aglet::normalize_related_pair(a, b);
        assert_eq!(lo, a);
        assert_eq!(hi, b);

        // Reversed input should also produce (a, b).
        let (lo2, hi2) = Aglet::normalize_related_pair(b, a);
        assert_eq!(lo2, a);
        assert_eq!(hi2, b);
    }

    #[test]
    fn normalize_related_pair_is_idempotent() {
        use uuid::Uuid;
        let a: ItemId = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000000").unwrap();
        let b: ItemId = Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();

        let first = Aglet::normalize_related_pair(a, b);
        let second = Aglet::normalize_related_pair(first.0, first.1);
        assert_eq!(first, second);
    }

    // ── ensure_not_self_link ───────────────────────────────────────────────────

    #[test]
    fn ensure_not_self_link_rejects_identical_ids() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let id = make_item(&store, "Task");

        let result = aglet.ensure_not_self_link(id, id, "depends-on");
        assert!(result.is_err(), "self-link should be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("self-link"),
            "error message should mention self-link, got: {msg}"
        );
    }

    #[test]
    fn ensure_not_self_link_accepts_distinct_ids() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "Task A");
        let b = make_item(&store, "Task B");

        assert!(
            aglet.ensure_not_self_link(a, b, "depends-on").is_ok(),
            "distinct ids should be accepted"
        );
    }

    // ── ensure_depends_on_no_cycle ─────────────────────────────────────────────

    #[test]
    fn ensure_depends_on_no_cycle_detects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        // A depends-on B
        aglet.link_items_depends_on(a, b).unwrap();

        // Trying to make B depend-on A would create A→B→A cycle.
        let result = aglet.ensure_depends_on_no_cycle(b, a);
        assert!(result.is_err(), "direct cycle A→B→A should be detected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cycle"),
            "error should mention cycle, got: {msg}"
        );
    }

    #[test]
    fn ensure_depends_on_no_cycle_detects_transitive_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        // A→B, B→C
        aglet.link_items_depends_on(a, b).unwrap();
        aglet.link_items_depends_on(b, c).unwrap();

        // Trying to make C depend-on A would create A→B→C→A cycle.
        let result = aglet.ensure_depends_on_no_cycle(c, a);
        assert!(
            result.is_err(),
            "transitive cycle A→B→C→A should be detected"
        );
    }

    #[test]
    fn ensure_depends_on_no_cycle_allows_non_cyclic_dependency() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        aglet.link_items_depends_on(a, b).unwrap();

        // C→A is fine; there's no path from A back to C.
        assert!(
            aglet.ensure_depends_on_no_cycle(c, a).is_ok(),
            "non-cyclic dependency should be accepted"
        );
    }

    // ── preview_section_move ──────────────────────────────────────────────────

    /// Build a simple two-section view used across preview tests.
    ///
    /// View criteria: requires `view_cat`.
    /// Section A criteria: requires `cat_a`.
    /// Section B criteria: requires `cat_b`, on_insert_assign: `extra`.
    fn preview_test_setup() -> (
        View,
        Section,
        Section,
        CategoryId,
        CategoryId,
        CategoryId,
        CategoryId,
    ) {
        let view_cat_id = CategoryId::new_v4();
        let cat_a_id = CategoryId::new_v4();
        let cat_b_id = CategoryId::new_v4();
        let extra_id = CategoryId::new_v4();

        let mut v = view("Board");
        v.criteria.set_criterion(CriterionMode::And, view_cat_id);

        let mut sec_a = section("A");
        sec_a.criteria.set_criterion(CriterionMode::And, cat_a_id);

        let mut sec_b = section("B");
        sec_b.criteria.set_criterion(CriterionMode::And, cat_b_id);
        sec_b.on_insert_assign.insert(extra_id);

        (v, sec_a, sec_b, view_cat_id, cat_a_id, cat_b_id, extra_id)
    }

    #[test]
    fn preview_section_move_none_to_none_is_empty() {
        let v = view("Empty");
        let preview = Aglet::preview_section_move(&v, None, None);
        assert!(preview.to_assign.is_empty());
        assert!(preview.to_unassign.is_empty());
    }

    #[test]
    fn preview_section_move_none_to_section_assigns_insert_targets() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        let preview = Aglet::preview_section_move(&v, None, Some(&sec_a));

        // Moving from unmatched → section A should assign section A's criteria
        // (cat_a) plus the view's criteria (view_cat).
        assert!(preview.to_assign.contains(&cat_a_id), "should assign cat_a");
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "should assign view_cat"
        );
        assert!(preview.to_unassign.is_empty(), "nothing to unassign");
    }

    #[test]
    fn preview_section_move_section_to_none_unassigns_remove_targets() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        let preview = Aglet::preview_section_move(&v, Some(&sec_a), None);

        // section_remove_targets for sec_a: structural targets (cat_a) minus
        // view criteria (view_cat) — so only cat_a is unassigned.
        assert!(
            preview.to_unassign.contains(&cat_a_id),
            "should unassign cat_a"
        );
        // view_cat is preserved because it belongs to the view criteria.
        assert!(
            !preview.to_unassign.contains(&view_cat_id),
            "view_cat should be preserved"
        );
        assert!(preview.to_assign.is_empty(), "nothing to assign");
    }

    #[test]
    fn preview_section_move_between_sections_net_change() {
        let (v, sec_a, sec_b, view_cat_id, cat_a_id, cat_b_id, extra_id) = preview_test_setup();

        let preview = Aglet::preview_section_move(&v, Some(&sec_a), Some(&sec_b));

        // Moving A → B:
        //   to_assign  = section_insert_targets(view, sec_b) = {cat_b, extra, view_cat}
        //   to_unassign = section_structural_targets(sec_a)  = {cat_a}
        //                 minus preserve (insert targets of B, which doesn't include cat_a)
        //                 → {cat_a}
        // view_cat is NOT in to_unassign (structural targets only covers the source
        // section's own categories, not the view's), so there is no cancellation.
        assert!(preview.to_assign.contains(&cat_b_id), "should assign cat_b");
        assert!(
            preview.to_assign.contains(&extra_id),
            "should assign on_insert_assign extra"
        );
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "view_cat included in insert targets"
        );
        assert!(
            preview.to_unassign.contains(&cat_a_id),
            "should unassign cat_a"
        );
        assert!(
            !preview.to_unassign.contains(&view_cat_id),
            "view_cat should not be in to_unassign"
        );
    }

    #[test]
    fn preview_section_move_same_section_to_unassign_is_empty() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        // Moving to the same section: the preserve set (= insert targets of the
        // target) covers all of the structural targets of the source, so nothing
        // is unassigned.  The insert targets are still returned in to_assign
        // (they would be re-applied, which is a no-op when already assigned).
        let preview = Aglet::preview_section_move(&v, Some(&sec_a), Some(&sec_a));
        assert!(
            preview.to_unassign.is_empty(),
            "to_unassign should be empty when target is same section"
        );
        assert!(
            preview.to_assign.contains(&cat_a_id),
            "cat_a appears in to_assign (re-apply is safe)"
        );
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "view_cat appears in to_assign"
        );
    }

    #[test]
    fn preview_section_move_on_remove_unassign_included() {
        let v = view("Board");
        let extra_remove_id = CategoryId::new_v4();

        let mut sec = section("WithExtra");
        sec.on_remove_unassign.insert(extra_remove_id);

        let preview = Aglet::preview_section_move(&v, Some(&sec), None);

        assert!(
            preview.to_unassign.contains(&extra_remove_id),
            "on_remove_unassign should appear in to_unassign"
        );
    }

    // --- Recurrence / succession tests ---

    use crate::model::{RecurrenceFrequency, RecurrenceRule};

    fn weekly_rule() -> RecurrenceRule {
        RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        }
    }

    fn recurring_item(text: &str) -> Item {
        let mut item = Item::new(text.to_string());
        item.when_date = Some(jiff::civil::date(2026, 4, 6).at(9, 0, 0, 0)); // Monday
        item.recurrence_rule = Some(weekly_rule());
        item
    }

    #[test]
    fn mark_recurring_item_done_generates_successor() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let item = recurring_item("Weekly standup");
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let result = aglet.mark_item_done(item.id).unwrap();

        // Successor was created
        assert!(result.successor_item_id.is_some());
        let successor_id = result.successor_item_id.unwrap();
        let successor = store.get_item(successor_id).unwrap();

        // Successor fields
        assert_eq!(successor.text, "Weekly standup");
        assert!(!successor.is_done);
        assert!(successor.done_date.is_none());
        assert_eq!(
            successor.when_date,
            Some(jiff::civil::date(2026, 4, 13).at(9, 0, 0, 0))
        );
        assert_eq!(successor.recurrence_rule, Some(weekly_rule()));
        assert_eq!(successor.recurrence_parent_item_id, Some(item.id));

        // Completed item is still done
        let completed = store.get_item(item.id).unwrap();
        assert!(completed.is_done);
        assert!(completed.done_date.is_some());
    }

    #[test]
    fn successor_inherits_series_id() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let item = recurring_item("Weekly standup");
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let result = aglet.mark_item_done(item.id).unwrap();
        let successor = store.get_item(result.successor_item_id.unwrap()).unwrap();
        let completed = store.get_item(item.id).unwrap();

        // Both share the same series ID (lazy-created)
        assert!(completed.recurrence_series_id.is_some());
        assert_eq!(
            completed.recurrence_series_id,
            successor.recurrence_series_id
        );
    }

    #[test]
    fn successor_copies_sticky_manual_assignments_not_reserved() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let project = category("Project", false);
        aglet.create_category(&project).unwrap();
        let priority = category("High", false);
        aglet.create_category(&priority).unwrap();
        let item = recurring_item("Deploy");
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, project.id, Some("manual:test".to_string()))
            .unwrap();
        aglet
            .assign_item_manual(item.id, priority.id, Some("manual:test".to_string()))
            .unwrap();

        let result = aglet.mark_item_done(item.id).unwrap();
        let successor = store.get_item(result.successor_item_id.unwrap()).unwrap();

        // Manual assignments carried forward
        assert!(successor.assignments.contains_key(&project.id));
        assert!(successor.assignments.contains_key(&priority.id));

        // Done category NOT carried forward
        let done_cat = store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|c| c.name.eq_ignore_ascii_case("Done"))
            .unwrap()
            .id;
        assert!(!successor.assignments.contains_key(&done_cat));
    }

    #[test]
    fn non_recurring_done_has_no_successor() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let item = Item::new("One-time task".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let result = aglet.mark_item_done(item.id).unwrap();
        assert!(result.successor_item_id.is_none());
    }

    #[test]
    fn repeated_completion_creates_chain() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let item = recurring_item("Weekly standup");
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        // Complete first instance → successor B
        let result_a = aglet.mark_item_done(item.id).unwrap();
        let b_id = result_a.successor_item_id.unwrap();
        let b = store.get_item(b_id).unwrap();
        assert_eq!(b.recurrence_parent_item_id, Some(item.id));
        assert_eq!(
            b.when_date,
            Some(jiff::civil::date(2026, 4, 13).at(9, 0, 0, 0))
        );

        // B needs an actionable category to be marked done
        aglet
            .assign_item_manual(b_id, work.id, Some("manual:test".to_string()))
            .unwrap();

        // Complete B → successor C
        let result_b = aglet.mark_item_done(b_id).unwrap();
        let c_id = result_b.successor_item_id.unwrap();
        let c = store.get_item(c_id).unwrap();
        assert_eq!(c.recurrence_parent_item_id, Some(b_id));
        assert_eq!(
            c.when_date,
            Some(jiff::civil::date(2026, 4, 20).at(9, 0, 0, 0))
        );

        // All three share the same series ID
        let a = store.get_item(item.id).unwrap();
        let b = store.get_item(b_id).unwrap();
        assert_eq!(a.recurrence_series_id, b.recurrence_series_id);
        assert_eq!(b.recurrence_series_id, c.recurrence_series_id);
    }

    #[test]
    fn successor_note_is_copied() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let work = category("Work", false);
        aglet.create_category(&work).unwrap();
        let mut item = recurring_item("Standup");
        item.note = Some("Discuss blockers".to_string());
        aglet.create_item(&item).unwrap();
        aglet
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let result = aglet.mark_item_done(item.id).unwrap();
        let successor = store.get_item(result.successor_item_id.unwrap()).unwrap();
        assert_eq!(successor.note, Some("Discuss blockers".to_string()));
    }
