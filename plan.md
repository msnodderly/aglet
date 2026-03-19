# Classification And When Implementation Plan

## Intent

Implement a unified classification system for Aglet that:

- keeps eager interpretation as the default for normal item workflows
- supports both `auto_apply` and `suggest_review`
- treats fuzzy `When` extraction as part of the same system
- preserves the current cascade model for profile rules, actions, subsumption,
  and exclusivity
- adds async/background handling only where provider cost makes it necessary

This plan intentionally does **not** add:

- new per-category provider opt-in controls
- a detailed recalc-scope picker UI
- Catalyst-style application scripting/triggers beyond today's action model

## Working Assumptions

- The existing per-category `enable_implicit_string` checkbox remains and keeps
  meaning "this category participates in deterministic rule-based implicit
  matching."
- Database-wide settings control provider selection and assignment policy.
- Current item save/edit flows remain eager by default.
- Bulk and expensive provider-backed work may run in the background.

## Target Behavior

### Continuous behavior

- Save a new or edited item:
  - parse fuzzy `When`
  - generate rule/LLM candidates
  - auto-apply or queue suggestions according to policy
  - run structural cascades synchronously on accepted/applied assignments
- Manual category assignment:
  - apply immediately
  - run cascade engine immediately
- Manual `When` edit:
  - apply immediately
  - run cascade engine immediately

### Review behavior

- In `suggest_review` mode, pending category and `When` suggestions appear in a
  shared review queue.
- Accepting a suggestion applies it and then runs structural cascades.
- Rejecting a suggestion persists the rejection so it does not immediately return.

## Phase 0: Preserve Current Invariants

Before adding new model pieces, write down and keep the current invariants:

- manual assignment re-runs the engine
- structural cascades remain synchronous after accepted/applied assignments
- exclusive sibling cleanup still happens
- profile conditions still fire from manual and automatic assignments
- `When` assignment remains visible and queryable immediately after acceptance

No code change should weaken these invariants.

## Phase 1: Add Database Settings And Suggestion Storage

### 1.1 Add classification settings model

Add a new persisted config object under `app_settings`, similar to workflow
config.

Suggested type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClassificationConfig {
    pub enabled: bool,
    pub continuous_mode: ContinuousMode,
    pub run_on_item_save: bool,
    pub run_on_category_change: bool,
    pub enabled_providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContinuousMode {
    Off,
    AutoApply,
    SuggestReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_id: String,
    pub enabled: bool,
    pub mode: ProviderMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderMode {
    InlineIfCheap,
    Background,
}
```

Suggested default value:

```rust
impl Default for ClassificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            continuous_mode: ContinuousMode::AutoApply,
            run_on_item_save: true,
            run_on_category_change: true,
            enabled_providers: vec![
                ProviderConfig {
                    provider_id: "implicit_string".to_string(),
                    enabled: true,
                    mode: ProviderMode::InlineIfCheap,
                },
                ProviderConfig {
                    provider_id: "when_parser".to_string(),
                    enabled: true,
                    mode: ProviderMode::InlineIfCheap,
                },
            ],
        }
    }
}
```

### 1.2 Add suggestion persistence

Add a new table for pending/accepted/rejected suggestions.

Suggested migration:

```sql
CREATE TABLE classification_suggestions (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,              -- 'category' | 'when'
    category_id TEXT,                -- nullable when kind='when'
    when_value TEXT,                 -- nullable when kind='category'
    provider_id TEXT NOT NULL,
    model TEXT,
    confidence REAL,
    rationale TEXT,
    status TEXT NOT NULL,            -- 'pending' | 'accepted' | 'rejected' | 'superseded'
    context_hash TEXT NOT NULL,
    item_revision_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    decided_at TEXT
);

CREATE INDEX idx_classification_suggestions_item_id
    ON classification_suggestions (item_id);

CREATE INDEX idx_classification_suggestions_status
    ON classification_suggestions (status);
```

### 1.3 Add store APIs

Add store methods:

```rust
pub fn get_classification_config(&self) -> Result<ClassificationConfig>;
pub fn set_classification_config(&self, cfg: &ClassificationConfig) -> Result<()>;

pub fn list_pending_suggestions(&self) -> Result<Vec<ClassificationSuggestion>>;
pub fn list_pending_suggestions_for_item(
    &self,
    item_id: ItemId,
) -> Result<Vec<ClassificationSuggestion>>;
pub fn upsert_suggestion(&self, suggestion: &ClassificationSuggestion) -> Result<()>;
pub fn set_suggestion_status(
    &self,
    suggestion_id: Uuid,
    status: SuggestionStatus,
) -> Result<()>;
pub fn supersede_suggestions_for_item_revision(
    &self,
    item_id: ItemId,
    new_revision_hash: &str,
) -> Result<()>;
```

## Phase 2: Introduce Candidate Model And Provider Traits

### 2.1 Add candidate types

Add provider-facing candidate types in `agenda-core`.

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationCandidate {
    pub item_id: ItemId,
    pub assignment: CandidateAssignment,
    pub provider: String,
    pub model: Option<String>,
    pub confidence: Option<f32>,
    pub rationale: Option<String>,
    pub context_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CandidateAssignment {
    Category(CategoryId),
    When(NaiveDateTime),
}
```

### 2.2 Add request envelope

```rust
#[derive(Debug, Clone)]
pub struct ClassificationRequest {
    pub item_id: ItemId,
    pub text: String,
    pub note: Option<String>,
    pub when_date: Option<NaiveDateTime>,
    pub manual_category_ids: Vec<CategoryId>,
    pub visible_view_name: Option<String>,
    pub visible_section_title: Option<String>,
    pub numeric_values: Vec<(CategoryId, Decimal)>,
    pub candidate_categories: Vec<CategoryDescriptor>,
}
```

### 2.3 Add provider trait

Keep the trait simple at first.

```rust
pub trait ClassificationProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>>;
    fn is_cheap(&self) -> bool {
        false
    }
}
```

## Phase 3: Wrap Existing Rule And When Logic As Providers

### 3.1 Implicit string provider

Move current implicit-string behavior into a provider instead of baking it
directly into `process_item`.

```rust
pub struct ImplicitStringProvider<'a> {
    pub categories: &'a [Category],
    pub classifier: &'a dyn Classifier,
}

impl ClassificationProvider for ImplicitStringProvider<'_> {
    fn id(&self) -> &'static str {
        "implicit_string"
    }

    fn is_cheap(&self) -> bool {
        true
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>> {
        let mut out = Vec::new();
        let match_text = match request.note.as_deref() {
            Some(note) if !note.trim().is_empty() => format!("{} {}", request.text, note),
            _ => request.text.clone(),
        };

        for category in self.categories {
            if !category.enable_implicit_string {
                continue;
            }
            if self.classifier.classify(&match_text, &category.name).is_none() {
                continue;
            }
            out.push(ClassificationCandidate {
                item_id: request.item_id,
                assignment: CandidateAssignment::Category(category.id),
                provider: self.id().to_string(),
                model: None,
                confidence: Some(1.0),
                rationale: Some(format!("matched category name '{}'", category.name)),
                context_hash: "v1".to_string(),
            });
        }
        Ok(out)
    }
}
```

### 3.2 When parser provider

Wrap existing fuzzy date parsing as another provider.

```rust
pub struct WhenParserProvider {
    pub parser: BasicDateParser,
    pub reference_date: NaiveDate,
}

impl ClassificationProvider for WhenParserProvider {
    fn id(&self) -> &'static str {
        "when_parser"
    }

    fn is_cheap(&self) -> bool {
        true
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>> {
        let Some(parsed) = self.parser.parse(&request.text, self.reference_date) else {
            return Ok(Vec::new());
        };

        Ok(vec![ClassificationCandidate {
            item_id: request.item_id,
            assignment: CandidateAssignment::When(parsed.datetime),
            provider: self.id().to_string(),
            model: None,
            confidence: Some(1.0),
            rationale: Some(format!("parsed date expression '{}'", parsed.matched_text)),
            context_hash: "v1".to_string(),
        }])
    }
}
```

## Phase 4: Split Detection From Application

### 4.1 Add classification service

Create a new service that:

1. builds a `ClassificationRequest`
2. runs enabled providers
3. deduplicates candidates
4. routes candidates according to config
5. either applies them or stores them as pending suggestions

Suggested skeleton:

```rust
pub struct ClassificationService<'a> {
    store: &'a Store,
    providers: Vec<Box<dyn ClassificationProvider>>,
}

impl<'a> ClassificationService<'a> {
    pub fn classify_item(
        &self,
        item_id: ItemId,
        cfg: &ClassificationConfig,
    ) -> Result<ClassificationOutcome> {
        let item = self.store.get_item(item_id)?;
        let request = self.build_request(&item)?;

        let mut candidates = Vec::new();
        for provider in &self.providers {
            candidates.extend(provider.classify(&request)?);
        }

        self.route_candidates(item_id, candidates, cfg)
    }
}
```

### 4.2 Add application helpers

Category and `When` suggestions should apply through one path, then reuse the
existing cascade machinery.

```rust
pub enum AppliedSuggestion {
    Category(CategoryId),
    When(NaiveDateTime),
}

pub fn apply_suggestion(
    agenda: &Agenda<'_>,
    item_id: ItemId,
    suggestion: &ClassificationSuggestion,
) -> Result<ProcessItemResult> {
    match suggestion.assignment() {
        CandidateAssignment::Category(category_id) => {
            agenda.assign_item_manual(
                item_id,
                category_id,
                Some("suggestion:accepted".to_string()),
            )
        }
        CandidateAssignment::When(dt) => {
            agenda.set_item_when_date(
                item_id,
                Some(dt),
                Some("suggestion:accepted.when".to_string()),
            )
        }
    }
}
```

### 4.3 Preserve structural derivation

Do **not** move profile/actions/subsumption into the provider layer.

Practical rule:

- providers infer first-order candidates
- accepted/applied candidates go through existing agenda APIs
- existing agenda APIs trigger the current cascade engine

This preserves current behavior while giving better provenance and review UX.

## Phase 5: Update Provenance Model

### 5.1 Extend assignment source

Current sources are too coarse once suggestions exist.

Suggested extension:

```rust
pub enum AssignmentSource {
    Manual,
    AutoMatch,
    Action,
    Subsumption,
    SuggestionAccepted,
    AutoClassified,
}
```

### 5.2 Add origin payload discipline

Keep `origin` for compact human-readable provenance, but also persist provider
and model in suggestion records.

Suggested origin strings:

- `rule:implicit_string`
- `rule:when_parser`
- `suggestion:ollama:mistral-small`
- `suggestion:openai:gpt-5-mini`

## Phase 6: Wire The New Pipeline Into Agenda

### 6.1 Item create/edit

Replace the current direct rule/date invocation with:

1. save item
2. run cheap eager providers inline when enabled
3. apply or queue according to config
4. run existing cascade engine if something was applied

Suggested integration point:

```rust
pub fn update_item_with_reference_date(
    &self,
    item: &Item,
    reference_date: NaiveDate,
) -> Result<ProcessItemResult> {
    self.store.update_item(item)?;

    let cfg = self.store.get_classification_config()?;
    if !cfg.enabled || !cfg.run_on_item_save {
        return process_item(self.store, self.classifier, item.id);
    }

    let outcome = self.classification_service(reference_date)?
        .classify_item(item.id, &cfg)?;

    if outcome.applied_anything {
        process_item(self.store, self.classifier, item.id)
    } else {
        Ok(ProcessItemResult::default())
    }
}
```

### 6.2 Category create/update/reparent

Keep the eager default in product terms, but route actual execution by cost:

- rule-only and small DB: inline
- expensive provider path: background

Initial implementation can keep category-wide work synchronous for rule-only
providers and defer async category-wide jobs until Phase 8.

## Phase 7: TUI Review Queue And Settings

### 7.1 Add classification settings pane

Add a database-level classification settings pane or popup.

Minimum controls:

- enabled on/off
- continuous mode
- enabled providers

Possible app state:

```rust
pub struct ClassificationUiState {
    pub pending_count: usize,
    pub settings_open: bool,
    pub review_index: usize,
}
```

### 7.2 Add review queue mode

New mode:

```rust
pub enum Mode {
    // existing...
    ClassificationReview,
}
```

Review actions:

- `Enter`: accept
- `r`: reject
- `s`: skip
- `Esc`: close

### 7.3 Show pending indicator

Show a subtle indicator in normal mode when pending suggestions exist.

Possible footer/status copy:

```text
Status: Classification on (Suggest/Review). 3 pending suggestions.
```

### 7.4 Keep category checkbox

Do not remove the existing category checkbox. If desired, relabel:

- `Auto-match` -> `Rule Auto-match`

No new per-category LLM toggles in this phase.

## Phase 8: Background Jobs For Expensive Work

This phase is for LLM providers and bulk category-triggered scans.

### 8.1 Add job table

```sql
CREATE TABLE classification_jobs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,        -- 'item' | 'bulk'
    status TEXT NOT NULL,      -- 'pending' | 'running' | 'done' | 'failed'
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    error_text TEXT
);
```

### 8.2 Add worker loop

Start with a simple in-process worker in TUI/CLI sessions rather than a daemon.

```rust
pub fn pump_pending_classification_jobs(store: &Store) -> Result<()> {
    while let Some(job) = store.next_pending_classification_job()? {
        run_classification_job(store, &job)?;
    }
    Ok(())
}
```

### 8.3 LLM providers run here first

Do not block interactive keystrokes on:

- Ollama
- LM Studio
- OpenAI
- Anthropic

All of those should begin as background-only providers.

## Phase 9: Add Local And Hosted Providers

### 9.1 Ollama

Suggested request interface:

```rust
pub struct OllamaProvider {
    pub endpoint: String,
    pub model: String,
    pub timeout: Duration,
}
```

### 9.2 LM Studio

Mirror the same shape as Ollama where possible.

### 9.3 Hosted providers

Start with one fast model per vendor. Keep prompt contract narrow:

- summarize candidate categories
- ask for zero or more likely categories
- optionally ask for `When`
- require compact JSON output

## Phase 10: Test Plan

### Engine and model tests

- saving an item with fuzzy date text creates a `When` suggestion or auto-applies it
- accepting a `When` suggestion updates `when_date` and re-runs cascades
- accepting a category suggestion triggers profile/action/subsumption behavior
- rejecting a suggestion suppresses immediate reappearance for the same revision
- manual assignment still triggers cascades
- current `enable_implicit_string` behavior remains intact

### Store tests

- classification config round-trips through `app_settings`
- suggestions persist and filter by pending/accepted/rejected
- jobs persist and transition through lifecycle states

### TUI tests

- pending count appears in status/footer
- `?` opens review queue
- accept/reject update store and return to normal state cleanly
- category manager still shows the rule checkbox
- no new provider toggles appear in category details

### Suggested test skeleton

```rust
#[test]
fn accepting_category_suggestion_triggers_existing_cascade_engine() {
    let (store, agenda) = test_harness();
    let item_id = seed_item(&store, "Book travel to Southampton for Tom");
    let suggestion = seed_pending_category_suggestion(&store, item_id, "Travel");

    apply_suggestion(&agenda, item_id, &suggestion).expect("accept suggestion");

    let item = store.get_item(item_id).expect("reload item");
    assert!(has_category_named(&store, &item, "Travel"));
    assert!(has_category_named(&store, &item, "Expense"));
}
```

## Rollout Order

Recommended implementation order:

1. settings + suggestion storage
2. candidate model + provider traits
3. implicit string provider
4. `when_parser` provider
5. apply/queue routing
6. review queue TUI
7. provenance/source updates
8. background jobs
9. Ollama / LM Studio
10. hosted providers

## Definition Of Done

This work is complete when:

- eager default classification still feels immediate for ordinary item edits
- `When` inference is handled in the same review/apply pipeline as category inference
- users can switch between `auto_apply` and `suggest_review`
- pending suggestions are reviewable in the TUI
- accepted suggestions preserve current cascade semantics
- expensive provider work no longer blocks interactive TUI use
- the existing `Auto-match` checkbox still works and is not overloaded with new meaning
