# Async Classification for Expensive Classifiers

## Context

Classification can invoke Ollama (local LLM) via HTTP, which takes seconds. Currently this freezes the TUI — either via the blocking overlay on item save, or via the new `=` on-demand classify key. The goal is to run expensive classification providers (Ollama) on a background thread so the UI stays responsive.

Key design question: what happens when an item is modified while background classification is in-flight? The `item_revision_hash` mechanism already exists — we use it to detect staleness and discard results for items whose content has changed.

## Design: Staleness Handling

- When classification is submitted, we snapshot the `item_revision_hash` (hash of text + note + when + manual categories).
- When results arrive, we re-read the item from Store, compute the current hash, and compare.
- **If hashes match**: apply results normally (supersede old suggestions, upsert new ones).
- **If hashes differ**: discard results silently. The item was modified — stale suggestions would be wrong.
- **No cancellation**: Ollama HTTP calls have a timeout. Letting them complete and discarding is simpler and more reliable than attempting thread cancellation with `reqwest::blocking`.
- **Deduplication**: Track in-flight item IDs. If `=` is pressed again on an item already being classified, skip (or show "already classifying" status).

## Architecture: `std::thread` + `mpsc` channels

No tokio needed — the codebase is fully synchronous with `reqwest::blocking`. A single worker thread receives jobs via `mpsc::channel` and sends results back.

**Split `collect_candidates` into two phases:**
1. **Prepare** (main thread, needs Store): read item, build `ClassificationRequest`, snapshot `item_revision_hash`
2. **Execute** (worker thread, no Store): call providers against the request, return candidates

This works because `ClassificationRequest`, `ClassificationCandidate`, `ClassificationConfig`, `OllamaProviderSettings`, and `Arc<dyn OllamaTransport>` are all `Send + Sync`.

## Changes

### 1. Split `collect_candidates` into prepare + execute
**File:** `crates/agenda-core/src/classification.rs`

Add to `ClassificationService`:
```rust
pub fn prepare_request(&self, item_id: ItemId) -> Result<(ClassificationRequest, String)>
```
Reads item from Store, calls `build_request`, computes `item_revision_hash`. Returns the request + hash.

Add free function:
```rust
pub fn execute_providers(
    providers: &[Box<dyn ClassificationProvider>],
    request: &ClassificationRequest,
) -> Result<(Vec<ClassificationCandidate>, Vec<String>)>
```
Runs provider iteration + dedup. No Store access needed.

Refactor `collect_candidates` to call both internally (pure refactor, no behavior change).

### 2. Extract `apply_classification_results` from `classify_item_on_demand`
**File:** `crates/agenda-core/src/agenda.rs`

Extract the candidate-processing loop (supersede old suggestions, upsert new ones based on config disposition) into:
```rust
pub fn apply_classification_results(
    &self, item_id: ItemId, item_revision_hash: &str, candidates: &[ClassificationCandidate],
) -> Result<usize>
```

Refactor `classify_item_on_demand` to call `collect_candidates` + `apply_classification_results`. No behavior change.

### 3. Add `prepare_background_classification` to `Agenda`
**File:** `crates/agenda-core/src/agenda.rs`

```rust
pub fn prepare_background_classification(
    &self, item_id: ItemId, reference_date: jiff::civil::Date,
) -> Result<Option<BackgroundClassificationJob>>
```

Returns `None` if no expensive providers are enabled. Otherwise returns a `BackgroundClassificationJob` containing: `item_id`, `item_revision_hash`, `ClassificationRequest`, cloned `ClassificationConfig`, cloned `OllamaProviderSettings`, cloned `Arc<dyn OllamaTransport>`, and `reference_date`. All fields are `Send`.

### 4. New module: `crates/agenda-tui/src/async_classify.rs`

Types:
- `BackgroundClassificationJob` (from agenda-core, or defined here)
- `ClassifyResult { item_id, item_revision_hash, candidates, error }`
- `ClassificationWorker` — owns `mpsc::Sender<Job>` + `mpsc::Receiver<Result>` + `JoinHandle`

`ClassificationWorker::spawn()` creates channels + `std::thread::spawn`. Worker loop:
1. `job_rx.recv()` — blocks until a job arrives
2. Build providers from owned config/settings/transport (no Store needed)
3. Call `execute_providers()`
4. Send `ClassifyResult` back via `result_tx`
5. Catch panics with `std::panic::catch_unwind` to avoid killing the thread

### 5. Wire worker into the TUI event loop
**File:** `crates/agenda-tui/src/lib.rs`

New fields on `App`:
```rust
classification_worker: ClassificationWorker,
in_flight_classifications: HashSet<ItemId>,
```

**File:** `crates/agenda-tui/src/app.rs`

New method `process_classification_results(&mut self, agenda)`:
- Called at top of each event loop iteration (before `terminal.draw`)
- Calls `worker.try_recv()` in a loop to drain completed results
- For each result:
  - Remove from `in_flight_classifications`
  - If error: set status message
  - Re-read item from Store, compute current `item_revision_hash`
  - If hash differs from result's hash: discard (item was modified), set status
  - If hash matches: call `agenda.apply_classification_results()`, refresh, set status

New method `submit_background_classification(&mut self, agenda, item_id)`:
- Skip if `in_flight_classifications.contains(&item_id)`
- Call `agenda.prepare_background_classification(item_id, reference_date)`
- If `Some(job)`: send to worker, insert into `in_flight_classifications`
- If `None`: no expensive providers enabled, run sync path

### 6. Update `=` key handler to use async path
**File:** `crates/agenda-tui/src/modes/board.rs`

Replace `queue_blocking_ui_action(ClassifyItems(...))` with:
- Run cheap providers synchronously (implicit string, when parser) via existing `classify_item_on_demand` but only for cheap providers
- Submit expensive providers (Ollama) via `submit_background_classification`
- Status: "Classifying N items in background..."
- Remove `PendingBlockingUiAction::ClassifyItems` variant

### 7. Update item save flow to use async path
**File:** `crates/agenda-tui/src/modes/board.rs`

In the `SaveInputPanelAdd`/`SaveInputPanelEdit` branches:
- Always save synchronously (no blocking overlay needed — save + cheap providers are fast)
- After save, submit background classification if semantic providers are enabled
- Remove the `should_show_blocking_classification_overlay()` check and `PendingBlockingUiAction::SaveInputPanelAdd/Edit` overlay path

### 8. In-flight indicator + completion notification
**File:** `crates/agenda-tui/src/render/mod.rs`

**In-progress indicator**: When `in_flight_classifications` is non-empty, prepend `[classifying N...]` to the footer status line so the user always sees that background work is happening.

**Completion notification**: When `process_classification_results` receives a result:
- Success with suggestions: `"Classification complete for '<item text>': N new suggestions (Shift+C to review)"`
- Success with no suggestions: `"Classification complete for '<item text>': no new suggestions"`
- Discarded (stale): `"Classification skipped for '<item text>' (item was modified)"`
- Error: `"Classification error: <message>"`
- When all in-flight jobs complete: status line returns to normal (no `[classifying...]` prefix)

These status messages use the existing transient status mechanism so they're visible until the next action.

## Critical Files
- `crates/agenda-core/src/classification.rs` — split collect_candidates
- `crates/agenda-core/src/agenda.rs` — extract apply_classification_results, add prepare method
- `crates/agenda-tui/src/async_classify.rs` — NEW: worker thread + channels
- `crates/agenda-tui/src/lib.rs` — App fields, mod declaration
- `crates/agenda-tui/src/app.rs` — event loop integration, result processing
- `crates/agenda-tui/src/modes/board.rs` — `=` handler + save flow changes
- `crates/agenda-tui/src/render/mod.rs` — in-flight indicator

## Verification
1. `cargo build -p agenda-core -p agenda-tui`
2. `cargo test -p agenda-core -p agenda-tui` — all existing tests pass
3. Manual test: configure Ollama, press `=` on an item → UI stays responsive, status shows "classifying in background", result arrives after a few seconds
4. Manual test: press `=`, then immediately edit the same item → stale result is discarded
5. Manual test: multi-select 3 items, press `=` → all 3 submitted, results trickle in
6. Manual test: add new item with Ollama enabled → save is instant, classification runs in background
