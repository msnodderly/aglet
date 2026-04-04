use crate::*;
use agenda_core::date_rules::{parse_date_value_expr, render_date_value_expr, EvaluationContext};
use agenda_core::model::{AssignmentSource, ConditionMatchMode, DateValueExpr};
use jiff::civil::{Date, DateTime, Time};
use jiff::Span;

enum CategoryInlineConfirmKeyAction {
    Confirm,
    Cancel,
    None,
}

struct WorkflowRolePrepResult {
    auto_match_disabled: bool,
    warn_other_derived_sources: bool,
}

fn category_inline_confirm_key_action(code: KeyCode) -> CategoryInlineConfirmKeyAction {
    match code {
        KeyCode::Char('y') => CategoryInlineConfirmKeyAction::Confirm,
        KeyCode::Esc => CategoryInlineConfirmKeyAction::Cancel,
        _ => CategoryInlineConfirmKeyAction::None,
    }
}

fn parse_also_match_entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn editable_condition_indices(category: &Category) -> Vec<usize> {
    category
        .conditions
        .iter()
        .enumerate()
        .filter(|(_, condition)| !matches!(condition, Condition::ImplicitString))
        .map(|(index, _)| index)
        .collect()
}

pub(crate) fn condition_match_mode_label(mode: ConditionMatchMode) -> &'static str {
    match mode {
        ConditionMatchMode::Any => "ANY",
        ConditionMatchMode::All => "ALL",
    }
}

fn date_draft_from_condition(condition: &Condition) -> Option<DateConditionDraft> {
    let Condition::Date { source, matcher } = condition else {
        return None;
    };
    let mut draft = DateConditionDraft {
        source: *source,
        ..DateConditionDraft::default()
    };
    match matcher {
        agenda_core::model::DateMatcher::Compare { op, value } => {
            draft.kind = DateConditionDraftKind::Compare(*op);
            draft.value_input = text_buffer::TextBuffer::new(render_date_value_expr(value));
        }
        agenda_core::model::DateMatcher::Range { from, through } => {
            match (from, through) {
                (DateValueExpr::TimeToday(time), DateValueExpr::Today) => {
                    if *time == default_afternoon_start() {
                        draft.kind = DateConditionDraftKind::ThisAfternoon;
                    } else {
                        draft.kind = DateConditionDraftKind::TodayAfter;
                        draft.value_input =
                            text_buffer::TextBuffer::new(render_date_value_expr(from));
                    }
                }
                (DateValueExpr::Today, DateValueExpr::TimeToday(_time)) => {
                    draft.kind = DateConditionDraftKind::TodayBefore;
                    draft.value_input = text_buffer::TextBuffer::new(render_date_value_expr(through));
                }
                _ => {
                    draft.kind = DateConditionDraftKind::Range;
                    draft.from_input = text_buffer::TextBuffer::new(render_date_value_expr(from));
                    draft.through_input =
                        text_buffer::TextBuffer::new(render_date_value_expr(through));
                }
            }
        }
    }
    Some(draft)
}

fn cycle_date_match_mode(draft: &mut DateConditionDraft, forward: bool) {
    const MODES: [DateConditionDraftKind; 9] = [
        DateConditionDraftKind::Compare(DateCompareOp::On),
        DateConditionDraftKind::Compare(DateCompareOp::Before),
        DateConditionDraftKind::Compare(DateCompareOp::After),
        DateConditionDraftKind::Compare(DateCompareOp::AtOrBefore),
        DateConditionDraftKind::Compare(DateCompareOp::AtOrAfter),
        DateConditionDraftKind::Range,
        DateConditionDraftKind::TodayAfter,
        DateConditionDraftKind::TodayBefore,
        DateConditionDraftKind::ThisAfternoon,
    ];

    let current = MODES
        .iter()
        .position(|mode| *mode == draft.kind)
        .unwrap_or(0);
    let next = if forward {
        (current + 1) % MODES.len()
    } else {
        (current + MODES.len() - 1) % MODES.len()
    };
    draft.kind = MODES[next];

    match draft.kind {
        DateConditionDraftKind::TodayAfter => {
            if draft.value_input.trimmed().is_empty() || draft.value_input.trimmed() == "today" {
                draft.value_input = text_buffer::TextBuffer::new("1:00pm today".to_string());
            }
        }
        DateConditionDraftKind::TodayBefore => {
            if draft.value_input.trimmed().is_empty() || draft.value_input.trimmed() == "today" {
                draft.value_input = text_buffer::TextBuffer::new("1:00pm today".to_string());
            }
        }
        DateConditionDraftKind::ThisAfternoon => {}
        DateConditionDraftKind::Range => {}
        DateConditionDraftKind::Compare(_) => {}
    }
}

fn default_afternoon_start() -> Time {
    Time::new(13, 0, 0, 0).expect("1pm should be representable")
}

pub(crate) fn draft_uses_range_fields(kind: DateConditionDraftKind) -> bool {
    matches!(kind, DateConditionDraftKind::Range)
}

pub(crate) fn draft_uses_value_field(kind: DateConditionDraftKind) -> bool {
    matches!(
        kind,
        DateConditionDraftKind::Compare(_)
            | DateConditionDraftKind::TodayAfter
            | DateConditionDraftKind::TodayBefore
    )
}

pub(crate) fn draft_match_label(kind: DateConditionDraftKind) -> String {
    match kind {
        DateConditionDraftKind::Compare(op) => agenda_core::date_rules::render_compare_op(op).to_string(),
        DateConditionDraftKind::Range => "Range".to_string(),
        DateConditionDraftKind::TodayAfter => "Today After".to_string(),
        DateConditionDraftKind::TodayBefore => "Today Before".to_string(),
        DateConditionDraftKind::ThisAfternoon => "This Afternoon".to_string(),
    }
}

pub(crate) fn draft_value_label(kind: DateConditionDraftKind) -> &'static str {
    match kind {
        DateConditionDraftKind::TodayAfter => "After",
        DateConditionDraftKind::TodayBefore => "Before",
        _ => "Value",
    }
}

fn parse_time_today_value_expr(input: &str) -> Result<DateValueExpr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("time value cannot be empty".to_string());
    }
    if let Ok(expr) = parse_date_value_expr(trimmed) {
        return match expr {
            DateValueExpr::TimeToday(_) => Ok(expr),
            _ => Err(format!("expected a time like '1:00pm' or '1:00pm today', got '{trimmed}'")),
        };
    }
    parse_date_value_expr(&format!("{trimmed} today")).and_then(|expr| match expr {
        DateValueExpr::TimeToday(_) => Ok(expr),
        _ => Err(format!("expected a time like '1:00pm', got '{trimmed}'")),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DateDraftMessageSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone)]
pub(crate) struct DateDraftMessage {
    pub(crate) severity: DateDraftMessageSeverity,
    pub(crate) text: String,
}

#[derive(Clone, Default)]
pub(crate) struct DateDraftFieldFeedback {
    pub(crate) normalized: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct DateConditionDraftFeedback {
    pub(crate) preview: String,
    pub(crate) value: DateDraftFieldFeedback,
    pub(crate) from: DateDraftFieldFeedback,
    pub(crate) through: DateDraftFieldFeedback,
    pub(crate) messages: Vec<DateDraftMessage>,
}

#[derive(Clone, Copy)]
enum DraftResolvedValue {
    Date(Date),
    DateTime(DateTime),
}

fn resolve_draft_value(expr: &DateValueExpr, ctx: &EvaluationContext) -> DraftResolvedValue {
    match expr {
        DateValueExpr::Today => DraftResolvedValue::Date(ctx.today()),
        DateValueExpr::Tomorrow => DraftResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(1))
                .expect("tomorrow should be representable"),
        ),
        DateValueExpr::DaysFromToday(days) => DraftResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(i64::from(*days)))
                .expect("future relative date should be representable"),
        ),
        DateValueExpr::DaysAgo(days) => DraftResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(-i64::from(*days)))
                .expect("past relative date should be representable"),
        ),
        DateValueExpr::AbsoluteDate(date) => DraftResolvedValue::Date(*date),
        DateValueExpr::AbsoluteDateTime(datetime) => DraftResolvedValue::DateTime(*datetime),
        DateValueExpr::TimeToday(time) => {
            DraftResolvedValue::DateTime(ctx.today().to_datetime(*time))
        }
    }
}

fn resolved_range_lower(value: DraftResolvedValue) -> DateTime {
    match value {
        DraftResolvedValue::Date(date) => date.to_datetime(jiff::civil::Time::midnight()),
        DraftResolvedValue::DateTime(datetime) => datetime,
    }
}

fn resolved_range_upper(value: DraftResolvedValue) -> DateTime {
    match value {
        DraftResolvedValue::Date(date) => date
            .checked_add(Span::new().days(1))
            .expect("next day should be representable")
            .to_datetime(jiff::civil::Time::midnight()),
        DraftResolvedValue::DateTime(datetime) => datetime,
    }
}

fn is_date_only_expr(expr: &DateValueExpr) -> bool {
    matches!(
        expr,
        DateValueExpr::Today
            | DateValueExpr::Tomorrow
            | DateValueExpr::DaysFromToday(_)
            | DateValueExpr::DaysAgo(_)
            | DateValueExpr::AbsoluteDate(_)
    )
}

pub(crate) fn date_condition_draft_feedback(
    draft: &DateConditionDraft,
    category_name: &str,
) -> DateConditionDraftFeedback {
    let ctx = EvaluationContext::now();
    let mut feedback = DateConditionDraftFeedback::default();

    let parse_date_field =
        |text: &str| -> (DateDraftFieldFeedback, Option<DateValueExpr>, String) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return (
                DateDraftFieldFeedback {
                    normalized: None,
                    error: Some("date value cannot be empty".to_string()),
                },
                None,
                trimmed.to_string(),
            );
        }
        match parse_date_value_expr(trimmed) {
            Ok(expr) => {
                let normalized = render_date_value_expr(&expr);
                (
                    DateDraftFieldFeedback {
                        normalized: Some(normalized.clone()),
                        error: None,
                    },
                    Some(expr),
                    normalized,
                )
            }
            Err(err) => (
                DateDraftFieldFeedback {
                    normalized: None,
                    error: Some(err),
                },
                None,
                trimmed.to_string(),
            ),
        }
    };

    let parse_time_field =
        |text: &str| -> (DateDraftFieldFeedback, Option<DateValueExpr>, String) {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return (
                    DateDraftFieldFeedback {
                        normalized: None,
                        error: Some("time value cannot be empty".to_string()),
                    },
                    None,
                    trimmed.to_string(),
                );
            }
            match parse_time_today_value_expr(trimmed) {
                Ok(expr) => {
                    let normalized = render_date_value_expr(&expr);
                    (
                        DateDraftFieldFeedback {
                            normalized: Some(normalized.clone()),
                            error: None,
                        },
                        Some(expr),
                        normalized,
                    )
                }
                Err(err) => (
                    DateDraftFieldFeedback {
                        normalized: None,
                        error: Some(err),
                    },
                    None,
                    trimmed.to_string(),
                ),
            }
        };

    if draft_uses_range_fields(draft.kind) {
        let (from_feedback, from_expr, from_preview) = parse_date_field(draft.from_input.text());
        let (through_feedback, through_expr, through_preview) =
            parse_date_field(draft.through_input.text());
        feedback.from = from_feedback;
        feedback.through = through_feedback;
        feedback.preview = format!(
            "{} from {} through {} -> {}",
            agenda_core::date_rules::render_date_source(draft.source),
            from_preview,
            through_preview,
            category_name
        );

        if let Some(error) = feedback.from.error.as_ref() {
            feedback.messages.push(DateDraftMessage {
                severity: DateDraftMessageSeverity::Error,
                text: format!("From: {error}"),
            });
        }
        if let Some(error) = feedback.through.error.as_ref() {
            feedback.messages.push(DateDraftMessage {
                severity: DateDraftMessageSeverity::Error,
                text: format!("Through: {error}"),
            });
        }

        if let (Some(from_expr), Some(through_expr)) = (from_expr.as_ref(), through_expr.as_ref()) {
            let lower = resolved_range_lower(resolve_draft_value(from_expr, &ctx));
            let upper = resolved_range_upper(resolve_draft_value(through_expr, &ctx));
            match lower.cmp(&upper) {
                std::cmp::Ordering::Greater => {
                    feedback.messages.push(DateDraftMessage {
                        severity: DateDraftMessageSeverity::Error,
                        text: "Impossible range: 'Through' is earlier than 'From'.".to_string(),
                    });
                }
                std::cmp::Ordering::Equal => {
                    feedback.messages.push(DateDraftMessage {
                        severity: DateDraftMessageSeverity::Warning,
                        text: "Suspicious range: this matches only a single instant.".to_string(),
                    });
                }
                std::cmp::Ordering::Less => {}
            }
        }
    } else if draft_uses_value_field(draft.kind) {
        let (value_feedback, value_expr, value_preview) = match draft.kind {
            DateConditionDraftKind::TodayAfter | DateConditionDraftKind::TodayBefore => {
                parse_time_field(draft.value_input.text())
            }
            _ => parse_date_field(draft.value_input.text()),
        };
        feedback.value = value_feedback;
        feedback.preview = match draft.kind {
            DateConditionDraftKind::Compare(op) => format!(
                "{} {} {} -> {}",
                agenda_core::date_rules::render_date_source(draft.source),
                agenda_core::date_rules::render_compare_op(op),
                value_preview,
                category_name
            ),
            DateConditionDraftKind::TodayAfter => format!(
                "{} today, after {} -> {}",
                agenda_core::date_rules::render_date_source(draft.source),
                value_preview,
                category_name
            ),
            DateConditionDraftKind::TodayBefore => format!(
                "{} today, before {} -> {}",
                agenda_core::date_rules::render_date_source(draft.source),
                value_preview,
                category_name
            ),
            _ => unreachable!("value field only applies to compare/today before-after"),
        };

        if let Some(error) = feedback.value.error.as_ref() {
            feedback.messages.push(DateDraftMessage {
                severity: DateDraftMessageSeverity::Error,
                text: format!("{}: {error}", draft_value_label(draft.kind)),
            });
        } else if let Some(expr) = value_expr.as_ref() {
            if matches!(
                draft.kind,
                DateConditionDraftKind::Compare(DateCompareOp::AtOrBefore)
                    | DateConditionDraftKind::Compare(DateCompareOp::AtOrAfter)
            )
                && is_date_only_expr(expr)
            {
                feedback.messages.push(DateDraftMessage {
                    severity: DateDraftMessageSeverity::Warning,
                    text: "Date-only cutoff spans a whole day. Add a time if you meant a clock boundary.".to_string(),
                });
            }
        }
    } else {
        feedback.preview = format!(
            "{} this afternoon -> {}",
            agenda_core::date_rules::render_date_source(draft.source),
            category_name
        );
        feedback.messages.push(DateDraftMessage {
            severity: DateDraftMessageSeverity::Info,
            text: "Parsed: from 1:00pm today through today".to_string(),
        });
    }

    if feedback.messages.is_empty() {
        let normalized = if draft_uses_range_fields(draft.kind) {
            feedback
                .through
                .normalized
                .as_ref()
                .zip(feedback.from.normalized.as_ref())
                .map(|(through, from)| format!("Parsed: {from} through {through}"))
        } else {
            feedback
                .value
                .normalized
                .as_ref()
                .map(|value| match draft.kind {
                    DateConditionDraftKind::TodayAfter => {
                        format!("Parsed: today, after {value}")
                    }
                    DateConditionDraftKind::TodayBefore => {
                        format!("Parsed: today, before {value}")
                    }
                    _ => format!("Parsed: {value}"),
                })
        };
        if let Some(text) = normalized {
            feedback.messages.push(DateDraftMessage {
                severity: DateDraftMessageSeverity::Info,
                text,
            });
        }
    }

    feedback
}

impl App {
    fn normalize_focused_date_condition_field(&mut self) {
        let Some(edit) = self.category_manager_condition_edit_mut() else {
            return;
        };
        let active_input = match edit.draft_date.field_focus {
            DateConditionField::Value if draft_uses_value_field(edit.draft_date.kind) => {
                Some(&mut edit.draft_date.value_input)
            }
            DateConditionField::From if draft_uses_range_fields(edit.draft_date.kind) => {
                Some(&mut edit.draft_date.from_input)
            }
            DateConditionField::Through if draft_uses_range_fields(edit.draft_date.kind) => {
                Some(&mut edit.draft_date.through_input)
            }
            _ => None,
        };

        let Some(input) = active_input else {
            return;
        };
        let trimmed = input.trimmed().to_string();
        if trimmed.is_empty() {
            return;
        }
        let parsed = match edit.draft_date.kind {
            DateConditionDraftKind::TodayAfter | DateConditionDraftKind::TodayBefore => {
                parse_time_today_value_expr(&trimmed)
            }
            _ => parse_date_value_expr(&trimmed),
        };
        if let Ok(expr) = parsed {
            input.set(render_date_value_expr(&expr));
        }
    }

    fn prepare_category_for_workflow_role(
        &mut self,
        category_id: CategoryId,
        agenda: &Agenda<'_>,
    ) -> TuiResult<WorkflowRolePrepResult> {
        let mut category = agenda.store().get_category(category_id)?;
        let warn_other_derived_sources =
            !category.conditions.is_empty() || !category.actions.is_empty();
        let auto_match_disabled = category.enable_implicit_string;
        if auto_match_disabled {
            category.enable_implicit_string = false;
            agenda.update_category(&category)?;
            let implicit_origin = format!("cat:{}", category.name);
            let implicit_assigned_item_ids: Vec<_> = agenda
                .store()
                .list_items()?
                .into_iter()
                .filter_map(|item| {
                    item.assignments.get(&category_id).and_then(|assignment| {
                        ((assignment.source == AssignmentSource::AutoMatch
                            || assignment.source == AssignmentSource::AutoClassified)
                            && assignment.origin.as_deref() == Some(implicit_origin.as_str()))
                        .then_some(item.id)
                    })
                })
                .collect();
            if !implicit_assigned_item_ids.is_empty() {
                for item_id in implicit_assigned_item_ids {
                    agenda.store().unassign_item(item_id, category_id)?;
                }
                let refreshed_category = agenda.store().get_category(category_id)?;
                agenda.update_category(&refreshed_category)?;
            }
        }
        Ok(WorkflowRolePrepResult {
            auto_match_disabled,
            warn_other_derived_sources,
        })
    }

    fn workflow_role_status_message(
        role_label: &str,
        category_name: &str,
        previous_name: Option<&str>,
        prep: Option<&WorkflowRolePrepResult>,
    ) -> String {
        let mut message = if let Some(previous_name) = previous_name {
            format!("{category_name} is now the {role_label} category (replaced {previous_name})")
        } else {
            format!("{category_name} is now the {role_label} category")
        };
        if let Some(prep) = prep {
            if prep.auto_match_disabled {
                message.push_str("; Auto-match disabled for workflow role");
            }
            if prep.warn_other_derived_sources {
                message.push_str("; warning: profile rules/actions can still assign it");
            }
        }
        message
    }

    fn workflow_setup_cross_role_conflict_status(
        &self,
        agenda: &Agenda<'_>,
        role_index: usize,
        selected_category_id: CategoryId,
    ) -> TuiResult<Option<String>> {
        let (role_label, current_role_id, other_role_label, other_role_id) = if role_index == 0 {
            (
                "Ready Queue",
                self.workflow_config.ready_category_id,
                "Claim Result",
                self.workflow_config.claim_category_id,
            )
        } else {
            (
                "Claim Result",
                self.workflow_config.claim_category_id,
                "Ready Queue",
                self.workflow_config.ready_category_id,
            )
        };

        if other_role_id != Some(selected_category_id)
            || current_role_id == Some(selected_category_id)
        {
            return Ok(None);
        }

        let selected_name = agenda.store().get_category(selected_category_id)?.name;
        let current_name = current_role_id
            .and_then(|category_id| agenda.store().get_category(category_id).ok())
            .map(|category| category.name)
            .unwrap_or_else(|| "(unset)".to_string());

        Ok(Some(format!(
            "{selected_name} is already the {other_role_label} category. Select {current_name} to unset {role_label}, or another category to replace it"
        )))
    }

    fn category_manager_save_key_pressed(&self, code: KeyCode) -> bool {
        matches!(code, KeyCode::Char('S'))
            || (matches!(code, KeyCode::Char('s'))
                && self.transient.key_modifiers.contains(KeyModifiers::SHIFT))
    }

    fn close_category_manager_with_status(&mut self, status: &str) {
        self.mode = Mode::Normal;
        self.close_category_manager_session();
        self.workflow_setup_open = false;
        self.settings.workflow_role_picker = None;
        self.clear_input();
        self.status = status.to_string();
    }

    fn handle_category_manager_discard_confirm_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Char('y') => {
                self.save_category_manager_dirty_details(agenda)?;
                self.close_category_manager_with_status("Category manager closed (saved)");
            }
            KeyCode::Char('n') => {
                self.close_category_manager_with_status(
                    "Category manager closed; unsaved detail changes discarded",
                );
            }
            KeyCode::Esc => {
                self.set_category_manager_discard_confirm(false);
                self.status =
                    "Kept category manager open; unsaved detail drafts retained".to_string();
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn category_manager_parent_label(&self, parent_id: Option<CategoryId>) -> String {
        parent_id
            .and_then(|id| {
                self.category_rows
                    .iter()
                    .find(|row| row.id == id)
                    .map(|row| row.name.clone())
            })
            .unwrap_or_else(|| "top level".to_string())
    }

    pub(crate) fn category_name_exists_elsewhere(
        &self,
        candidate: &str,
        excluding_id: Option<CategoryId>,
    ) -> bool {
        self.categories.iter().any(|category| {
            Some(category.id) != excluding_id && category.name.eq_ignore_ascii_case(candidate)
        })
    }

    fn selected_category_parent_id(&self) -> Option<CategoryId> {
        let selected_id = self.selected_category_id()?;
        self.categories
            .iter()
            .find(|category| category.id == selected_id)
            .and_then(|category| category.parent)
    }

    fn open_category_create_panel(&mut self, parent_id: Option<CategoryId>, status: String) {
        let parent_label = self.category_manager_parent_label(parent_id);
        self.input_panel = Some(input_panel::InputPanel::new_category_create(
            parent_id,
            &parent_label,
        ));
        // CategoryCreate uses InputPanel; clear any stale inline action first.
        self.set_category_manager_inline_action(None);
        self.name_input_context = Some(NameInputContext::CategoryCreate);
        self.mode = Mode::InputPanel;
        self.status = status;
    }

    fn start_category_inline_rename(&mut self) {
        let Some((row_id, row_name, is_reserved)) = self
            .selected_category_row()
            .map(|row| (row.id, row.name.clone(), row.is_reserved))
        else {
            self.status = "No selected category".to_string();
            return;
        };
        if is_reserved {
            self.status = format!("Category {} is reserved and cannot be renamed", row_name);
            return;
        }
        self.set_category_manager_inline_action(Some(CategoryInlineAction::Rename {
            category_id: row_id,
            original_name: row_name.clone(),
            buf: text_buffer::TextBuffer::new(row_name.clone()),
        }));
        self.status = format!("Rename {}: edit name, Enter apply, Esc cancel", row_name);
    }

    fn start_category_inline_delete_confirm(&mut self) {
        let Some((row_id, row_name)) = self
            .selected_category_row()
            .map(|row| (row.id, row.name.clone()))
        else {
            self.status = "No selected category".to_string();
            return;
        };
        self.set_category_manager_inline_action(Some(CategoryInlineAction::DeleteConfirm {
            category_id: row_id,
            category_name: row_name.clone(),
        }));
        self.status = format!("Delete category \"{}\"? y/n", row_name);
    }

    fn apply_category_inline_rename(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        original_name: String,
        name: String,
    ) -> TuiResult<()> {
        if name == original_name {
            self.set_category_manager_inline_action(None);
            self.status = "Category rename canceled (unchanged)".to_string();
            return Ok(());
        }
        let mut category = agenda.store().get_category(category_id)?;
        if is_reserved_category_name(&category.name) {
            self.set_category_manager_inline_action(None);
            self.status = format!(
                "Category {} is reserved and cannot be renamed",
                category.name
            );
            return Ok(());
        }
        category.name = name.clone();
        match agenda.update_category(&category) {
            Ok(result) => {
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.set_category_manager_inline_action(None);
                self.status = format!(
                    "Renamed category to {name} (processed_items={}, affected_items={})",
                    result.processed_items, result.affected_items
                );
            }
            Err(err) => {
                self.status = format!("Rename failed: {err}");
            }
        }
        Ok(())
    }

    fn apply_category_inline_delete_confirm(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        category_name: String,
    ) -> TuiResult<()> {
        let old_visible_index = self.category_manager_visible_tree_index().unwrap_or(0);
        match agenda.store().delete_category(category_id) {
            Ok(()) => {
                self.refresh(agenda.store())?;
                if let Some(visible) = self.category_manager_visible_row_indices() {
                    if !visible.is_empty() {
                        let next = old_visible_index.min(visible.len().saturating_sub(1));
                        self.set_category_manager_visible_selection(next);
                    }
                }
                self.status = format!("Deleted category {}", category_name);
            }
            Err(err) => {
                self.status = format!("Delete failed: {err}");
            }
        }
        self.set_category_manager_inline_action(None);
        Ok(())
    }

    fn category_manager_has_active_filter(&self) -> bool {
        self.category_manager_filter_text()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }

    fn block_direct_structure_move_while_filtered(&mut self) -> bool {
        if self.category_manager_has_active_filter() {
            self.status =
                "Clear category filter before direct H/L/J/K moves or << / >> shifts".to_string();
            true
        } else {
            false
        }
    }

    fn recompute_category_manager_details_note_dirty(&mut self) {
        let selected_id = self.selected_category_id();
        let saved_note = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .and_then(|c| c.note.clone())
            .unwrap_or_default();
        let current_note = self
            .category_manager_details_note_text()
            .unwrap_or_default()
            .to_string();
        self.mark_category_manager_details_note_dirty(current_note != saved_note);
    }

    fn recompute_category_manager_details_also_match_dirty(&mut self) {
        let selected_id = self.selected_category_id();
        let saved_also_match = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.also_match.clone())
            .unwrap_or_default();
        let current_also_match = parse_also_match_entries(
            self.category_manager_details_also_match_text()
                .unwrap_or_default(),
        );
        self.mark_category_manager_details_also_match_dirty(current_also_match != saved_also_match);
    }

    fn start_category_manager_details_note_edit(&mut self) {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return;
        }
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        self.set_category_manager_details_note_editing(true);
        self.status = "Edit category note: type text, Esc:discard, Tab:leave".to_string();
    }

    fn start_category_manager_details_also_match_edit(&mut self) {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return;
        }
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::AlsoMatch);
        self.set_category_manager_details_also_match_editing(true);
        self.status =
            "Edit also-match terms: one entry per line, Esc:discard, Tab:leave".to_string();
    }

    fn save_category_manager_details_note(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            self.set_category_manager_details_note_editing(false);
            self.reload_category_manager_details_note_from_selected();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        let next_note = self
            .category_manager_details_note_text()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let next_note = if next_note.trim().is_empty() {
            None
        } else {
            Some(next_note)
        };
        if category.note == next_note {
            self.mark_category_manager_details_note_dirty(false);
            self.set_category_manager_details_note_editing(false);
            self.status = "Category note unchanged".to_string();
            return Ok(());
        }

        category.note = next_note;
        let saved_name = category.name.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.reload_category_manager_details_note_from_selected();
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        self.status = format!(
            "Saved note for {} (processed_items={}, affected_items={})",
            saved_name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn save_category_manager_details_also_match(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            self.set_category_manager_details_also_match_editing(false);
            self.reload_category_manager_details_also_match_from_selected();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        let next_also_match = parse_also_match_entries(
            self.category_manager_details_also_match_text()
                .unwrap_or_default(),
        );
        if category.also_match == next_also_match {
            self.mark_category_manager_details_also_match_dirty(false);
            self.set_category_manager_details_also_match_editing(false);
            self.status = "Also-match terms unchanged".to_string();
            return Ok(());
        }

        category.also_match = next_also_match;
        let saved_name = category.name.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.reload_category_manager_details_also_match_from_selected();
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::AlsoMatch);
        self.status = format!(
            "Saved also-match terms for {} (processed_items={}, affected_items={})",
            saved_name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn save_category_manager_dirty_details(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.category_manager_details_note_dirty()
            && !self.category_manager_details_note_editing()
        {
            self.save_category_manager_details_note(agenda)?;
        }
        if self.category_manager_details_also_match_dirty()
            && !self.category_manager_details_also_match_editing()
        {
            self.save_category_manager_details_also_match(agenda)?;
        }
        Ok(())
    }

    fn category_manager_details_context(&self) -> (bool, bool) {
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        let integer_mode = self
            .selected_category_id()
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.numeric_format.clone().unwrap_or_default().decimal_places == 0)
            .unwrap_or(false);
        (is_numeric, integer_mode)
    }

    fn cycle_category_manager_details_section(&mut self, delta: i32) {
        let Some(details_focus) = self.category_manager_details_focus() else {
            return;
        };
        let (is_numeric, _integer_mode) = self.category_manager_details_context();
        let target = match delta.signum() {
            d if d > 0 => {
                if is_numeric {
                    match details_focus {
                        CategoryManagerDetailsFocus::Integer
                        | CategoryManagerDetailsFocus::DecimalPlaces
                        | CategoryManagerDetailsFocus::CurrencySymbol
                        | CategoryManagerDetailsFocus::ThousandsSeparator => {
                            CategoryManagerDetailsFocus::Note
                        }
                        CategoryManagerDetailsFocus::Note => {
                            self.set_category_manager_focus(CategoryManagerFocus::Filter);
                            return;
                        }
                        _ => CategoryManagerDetailsFocus::Integer,
                    }
                } else {
                    match details_focus {
                        CategoryManagerDetailsFocus::Exclusive
                        | CategoryManagerDetailsFocus::AutoMatch
                        | CategoryManagerDetailsFocus::SemanticMatch
                        | CategoryManagerDetailsFocus::MatchCategoryName
                        | CategoryManagerDetailsFocus::Actionable => {
                            CategoryManagerDetailsFocus::AlsoMatch
                        }
                        CategoryManagerDetailsFocus::AlsoMatch => {
                            CategoryManagerDetailsFocus::Conditions
                        }
                        CategoryManagerDetailsFocus::Conditions => {
                            CategoryManagerDetailsFocus::Actions
                        }
                        CategoryManagerDetailsFocus::Actions => CategoryManagerDetailsFocus::Note,
                        CategoryManagerDetailsFocus::Note => {
                            self.set_category_manager_focus(CategoryManagerFocus::Filter);
                            return;
                        }
                        _ => CategoryManagerDetailsFocus::Exclusive,
                    }
                }
            }
            d if d < 0 => {
                if is_numeric {
                    match details_focus {
                        CategoryManagerDetailsFocus::Note => {
                            CategoryManagerDetailsFocus::ThousandsSeparator
                        }
                        CategoryManagerDetailsFocus::Integer
                        | CategoryManagerDetailsFocus::DecimalPlaces
                        | CategoryManagerDetailsFocus::CurrencySymbol
                        | CategoryManagerDetailsFocus::ThousandsSeparator => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                        _ => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                    }
                } else {
                    match details_focus {
                        CategoryManagerDetailsFocus::Note => CategoryManagerDetailsFocus::Actions,
                        CategoryManagerDetailsFocus::Actions => {
                            CategoryManagerDetailsFocus::Conditions
                        }
                        CategoryManagerDetailsFocus::Conditions => {
                            CategoryManagerDetailsFocus::AlsoMatch
                        }
                        CategoryManagerDetailsFocus::AlsoMatch => {
                            CategoryManagerDetailsFocus::Actionable
                        }
                        CategoryManagerDetailsFocus::Exclusive
                        | CategoryManagerDetailsFocus::AutoMatch
                        | CategoryManagerDetailsFocus::SemanticMatch
                        | CategoryManagerDetailsFocus::MatchCategoryName
                        | CategoryManagerDetailsFocus::Actionable => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                        _ => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                    }
                }
            }
            _ => details_focus,
        };
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(target);
    }

    fn handle_category_manager_tab_navigation(&mut self, reverse: bool) {
        match self.category_manager_focus() {
            Some(CategoryManagerFocus::Filter) => {
                self.set_category_manager_filter_editing(false);
                if reverse {
                    self.set_category_manager_focus(CategoryManagerFocus::Details);
                } else {
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                }
            }
            Some(CategoryManagerFocus::Tree) => {
                self.set_category_manager_focus(if reverse {
                    CategoryManagerFocus::Filter
                } else {
                    CategoryManagerFocus::Details
                });
            }
            Some(CategoryManagerFocus::Details) => {
                self.cycle_category_manager_details_section(if reverse { -1 } else { 1 });
            }
            None => {}
        }
    }

    fn handle_category_manager_details_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self.category_manager_focus() != Some(CategoryManagerFocus::Details) {
            return Ok(false);
        }
        let Some(mut details_focus) = self.category_manager_details_focus() else {
            return Ok(false);
        };

        // Snap focus to Note when viewing a numeric category (flags don't apply)
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if is_numeric
            && matches!(
                details_focus,
                CategoryManagerDetailsFocus::Exclusive
                    | CategoryManagerDetailsFocus::AutoMatch
                    | CategoryManagerDetailsFocus::SemanticMatch
                    | CategoryManagerDetailsFocus::MatchCategoryName
                    | CategoryManagerDetailsFocus::Actionable
                    | CategoryManagerDetailsFocus::AlsoMatch
                    | CategoryManagerDetailsFocus::Conditions
                    | CategoryManagerDetailsFocus::Actions
            )
        {
            details_focus = CategoryManagerDetailsFocus::Note;
            self.set_category_manager_details_focus(details_focus);
        }

        if self.category_manager_details_inline_input().is_some() {
            match code {
                KeyCode::Esc => {
                    self.set_category_manager_details_inline_input(None);
                    self.status = "Numeric format edit canceled".to_string();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    self.save_category_manager_numeric_inline_edit(agenda)?;
                    return Ok(true);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(input) = self.category_manager_details_inline_input_mut() {
                        if input.buffer.handle_key_event(text_key, false) {
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if self.category_manager_save_key_pressed(code)
            && (self.category_manager_details_note_dirty()
                || self.category_manager_details_also_match_dirty())
            && !self.category_manager_details_note_editing()
            && !self.category_manager_details_also_match_editing()
        {
            // Let category-manager level save handling persist any dirty detail drafts
            // before text-entry auto-start consumes the key.
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::Note
            && self.category_manager_details_note_editing()
        {
            match code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                    self.set_category_manager_details_note_editing(false);
                    if self.category_manager_details_note_dirty() {
                        self.save_category_manager_details_note(agenda)?;
                    }
                    // Esc is consumed (stays on Note); Tab/BackTab fall through to navigation.
                    return Ok(code == KeyCode::Esc);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(buf) = self.category_manager_details_note_edit_mut() {
                        if buf.handle_key_event(text_key, true) {
                            self.recompute_category_manager_details_note_dirty();
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::AlsoMatch
            && self.category_manager_details_also_match_editing()
        {
            match code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                    self.set_category_manager_details_also_match_editing(false);
                    if self.category_manager_details_also_match_dirty() {
                        self.save_category_manager_details_also_match(agenda)?;
                    }
                    return Ok(code == KeyCode::Esc);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(buf) = self.category_manager_details_also_match_edit_mut() {
                        if buf.handle_key_event(text_key, true) {
                            self.recompute_category_manager_details_also_match_dirty();
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::Note
            && (matches!(code, KeyCode::Char(c) if c != ' ')
                || matches!(code, KeyCode::Backspace | KeyCode::Delete))
        {
            self.start_category_manager_details_note_edit();
            if self.category_manager_details_note_editing() {
                let text_key = self.text_key_event(code);
                if let Some(buf) = self.category_manager_details_note_edit_mut() {
                    if buf.handle_key_event(text_key, true) {
                        self.recompute_category_manager_details_note_dirty();
                    }
                }
                return Ok(true);
            }
        }

        if details_focus == CategoryManagerDetailsFocus::AlsoMatch
            && (matches!(code, KeyCode::Char(_))
                || matches!(code, KeyCode::Backspace | KeyCode::Delete))
        {
            self.start_category_manager_details_also_match_edit();
            if self.category_manager_details_also_match_editing() {
                let text_key = self.text_key_event(code);
                if let Some(buf) = self.category_manager_details_also_match_edit_mut() {
                    if buf.handle_key_event(text_key, true) {
                        self.recompute_category_manager_details_also_match_dirty();
                    }
                }
                return Ok(true);
            }
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cycle_category_manager_details_focus(-1);
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cycle_category_manager_details_focus(1);
                return Ok(true);
            }
            KeyCode::Enter => match details_focus {
                CategoryManagerDetailsFocus::Exclusive => {
                    self.toggle_selected_category_exclusive(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AutoMatch => {
                    self.toggle_selected_category_implicit(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::SemanticMatch => {
                    self.toggle_selected_category_semantic(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::MatchCategoryName => {
                    self.toggle_selected_category_match_category_name(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actionable => {
                    self.toggle_selected_category_actionable(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AlsoMatch => {
                    self.start_category_manager_details_also_match_edit();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Integer => {
                    self.toggle_selected_category_integer_mode(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::DecimalPlaces => {
                    self.start_category_manager_numeric_inline_edit(
                        CategoryManagerDetailsInlineField::DecimalPlaces,
                    )?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::CurrencySymbol => {
                    self.start_category_manager_numeric_inline_edit(
                        CategoryManagerDetailsInlineField::CurrencySymbol,
                    )?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::ThousandsSeparator => {
                    self.toggle_selected_category_thousands_separator(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Conditions => {
                    self.open_condition_edit_list();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actions => {
                    self.open_action_edit_list();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Note => {
                    self.start_category_manager_details_note_edit();
                    return Ok(true);
                }
            },
            KeyCode::Char(' ') => match details_focus {
                CategoryManagerDetailsFocus::Exclusive => {
                    self.toggle_selected_category_exclusive(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AutoMatch => {
                    self.toggle_selected_category_implicit(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::SemanticMatch => {
                    self.toggle_selected_category_semantic(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::MatchCategoryName => {
                    self.toggle_selected_category_match_category_name(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actionable => {
                    self.toggle_selected_category_actionable(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AlsoMatch => {
                    self.start_category_manager_details_also_match_edit();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Integer => {
                    self.toggle_selected_category_integer_mode(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::DecimalPlaces
                | CategoryManagerDetailsFocus::CurrencySymbol => {
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::ThousandsSeparator => {
                    self.toggle_selected_category_thousands_separator(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Conditions => {
                    self.open_condition_edit_list();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actions => {
                    self.open_action_edit_list();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Note => {
                    self.start_category_manager_details_note_edit();
                    return Ok(true);
                }
            },
            _ => {}
        }

        Ok(false)
    }

    fn selected_category_mut(&self) -> Option<Category> {
        let row = self.selected_category_row()?;
        self.categories.iter().find(|c| c.id == row.id).cloned()
    }

    fn persist_selected_category_numeric_format(
        &mut self,
        agenda: &Agenda<'_>,
        next: NumericFormat,
        status: String,
    ) -> TuiResult<()> {
        let mut cat = self.selected_category_mut().ok_or("No category")?;
        cat.numeric_format = Some(next);
        agenda.store().update_category(&cat)?;
        let category_id = cat.id;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.normalize_category_manager_details_focus();
        self.status = status;
        Ok(())
    }

    fn toggle_selected_category_integer_mode(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        let mut next = current.clone();
        next.decimal_places = if current.decimal_places == 0 { 2 } else { 0 };
        self.persist_selected_category_numeric_format(
            agenda,
            next,
            format!(
                "Format: {}",
                if current.decimal_places == 0 {
                    "decimal mode"
                } else {
                    "integer mode"
                }
            ),
        )
    }

    fn toggle_selected_category_thousands_separator(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        let mut next = current.clone();
        next.use_thousands_separator = !next.use_thousands_separator;
        self.persist_selected_category_numeric_format(
            agenda,
            next,
            if current.use_thousands_separator {
                "Thousands separator disabled".to_string()
            } else {
                "Thousands separator enabled".to_string()
            },
        )
    }

    fn start_category_manager_numeric_inline_edit(
        &mut self,
        field: CategoryManagerDetailsInlineField,
    ) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        if field == CategoryManagerDetailsInlineField::DecimalPlaces && current.decimal_places == 0
        {
            self.status = "Decimal places is disabled while Integer is enabled".to_string();
            self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Integer);
            return Ok(());
        }
        let buffer = match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                text_buffer::TextBuffer::new(current.decimal_places.to_string())
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                text_buffer::TextBuffer::new(current.currency_symbol.unwrap_or_default())
            }
        };
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                CategoryManagerDetailsFocus::DecimalPlaces
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                CategoryManagerDetailsFocus::CurrencySymbol
            }
        });
        self.set_category_manager_details_inline_input(Some(CategoryManagerDetailsInlineInput {
            field,
            buffer,
        }));
        self.status = match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                "Editing decimal places: Enter save, Esc cancel".to_string()
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                "Editing currency symbol: Enter save, Esc cancel".to_string()
            }
        };
        Ok(())
    }

    fn save_category_manager_numeric_inline_edit(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(input) = self.category_manager_details_inline_input().cloned() else {
            return Ok(());
        };
        let mut next = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        match input.field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                let raw = input.buffer.trimmed();
                let Ok(parsed) = raw.parse::<u8>() else {
                    self.status = "Decimal places must be a non-negative integer".to_string();
                    return Ok(());
                };
                next.decimal_places = parsed;
                self.set_category_manager_details_inline_input(None);
                self.persist_selected_category_numeric_format(
                    agenda,
                    next,
                    format!("Decimal places set to {parsed}"),
                )?;
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                let trimmed = input.buffer.trimmed();
                next.currency_symbol = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.set_category_manager_details_inline_input(None);
                self.persist_selected_category_numeric_format(
                    agenda,
                    next,
                    "Updated currency symbol".to_string(),
                )?;
            }
        }
        Ok(())
    }

    fn outdent_selected_category(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category structure is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let Some(parent_id) = category.parent else {
            self.status = format!("{category_name} is already at the top level");
            return Ok(());
        };
        let Some(parent) = self.categories.iter().find(|c| c.id == parent_id) else {
            self.status = "Outdent failed: parent category missing".to_string();
            return Ok(());
        };
        let new_parent_id = parent.parent;
        let target_siblings: Vec<CategoryId> = if let Some(grandparent_id) = new_parent_id {
            self.categories
                .iter()
                .find(|c| c.id == grandparent_id)
                .map(|grandparent| grandparent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let insert_index = Some(
            target_siblings
                .iter()
                .position(|id| *id == parent_id)
                .map(|idx| idx + 1)
                .unwrap_or(target_siblings.len()),
        );
        let new_parent_label = self.category_manager_parent_label(new_parent_id);
        let result = agenda.move_category_to_parent(category_id, new_parent_id, insert_index)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = format!(
            "Outdented {} to {} (processed_items={}, affected_items={})",
            category_name, new_parent_label, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn indent_selected_category_under_previous_sibling(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category structure is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let sibling_ids: Vec<CategoryId> = if let Some(parent_id) = category.parent {
            self.categories
                .iter()
                .find(|c| c.id == parent_id)
                .map(|parent| parent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let Some(idx) = sibling_ids.iter().position(|id| *id == category_id) else {
            self.status = "Indent failed: category not found among siblings".to_string();
            return Ok(());
        };
        if idx == 0 {
            self.status = format!("{category_name} has no previous sibling to indent under");
            return Ok(());
        }
        let new_parent_id = Some(sibling_ids[idx - 1]);
        let new_parent_label = self.category_manager_parent_label(new_parent_id);
        let result = agenda.move_category_to_parent(category_id, new_parent_id, None)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = format!(
            "Indented {} under {} (processed_items={}, affected_items={})",
            category_name, new_parent_label, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn reorder_selected_category_sibling(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category order is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let parent_id = category.parent;
        let sibling_ids: Vec<CategoryId> = if let Some(parent_id) = parent_id {
            self.categories
                .iter()
                .find(|c| c.id == parent_id)
                .map(|parent| parent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let Some(idx) = sibling_ids.iter().position(|id| *id == category_id) else {
            self.status = "Reorder failed: category not found among siblings".to_string();
            return Ok(());
        };

        if (delta < 0 && idx == 0) || (delta > 0 && idx + 1 >= sibling_ids.len()) {
            self.status = if delta < 0 {
                format!("{category_name} is already first among siblings")
            } else {
                format!("{category_name} is already last among siblings")
            };
            return Ok(());
        }

        agenda.move_category_within_parent(category_id, delta.signum())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = if delta < 0 {
            format!("Moved {category_name} up among siblings")
        } else {
            format!("Moved {category_name} down among siblings")
        };
        Ok(())
    }

    pub(crate) fn handle_category_manager_inline_action_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let Some(action) = self.category_manager_inline_action().cloned() else {
            return Ok(false);
        };

        match action {
            CategoryInlineAction::Rename {
                category_id,
                original_name,
                mut buf,
            } => {
                match code {
                    KeyCode::Esc => {
                        self.set_category_manager_inline_action(None);
                        self.status = "Rename canceled".to_string();
                    }
                    KeyCode::Enter => {
                        let name = buf.trimmed().to_string();
                        if name.is_empty() {
                            self.status = "Name cannot be empty".to_string();
                        } else if is_reserved_category_name(&name)
                            && !original_name.eq_ignore_ascii_case(&name)
                        {
                            self.status = format!(
                                "Cannot rename to reserved category '{}'. Use a different name.",
                                name
                            );
                        } else if self.category_name_exists_elsewhere(&name, Some(category_id)) {
                            self.status = format!(
                                "Category '{}' already exists. Cannot rename duplicate.",
                                name
                            );
                        } else {
                            self.apply_category_inline_rename(
                                agenda,
                                category_id,
                                original_name,
                                name,
                            )?;
                        }
                    }
                    _ => {
                        if buf.handle_key_event(self.text_key_event(code), false) {
                            self.set_category_manager_inline_action(Some(
                                CategoryInlineAction::Rename {
                                    category_id,
                                    original_name,
                                    buf,
                                },
                            ));
                        }
                    }
                }
                Ok(true)
            }
            CategoryInlineAction::DeleteConfirm {
                category_id,
                category_name,
            } => {
                match category_inline_confirm_key_action(code) {
                    CategoryInlineConfirmKeyAction::Confirm => {
                        self.apply_category_inline_delete_confirm(
                            agenda,
                            category_id,
                            category_name,
                        )?;
                    }
                    CategoryInlineConfirmKeyAction::Cancel => {
                        self.set_category_manager_inline_action(None);
                        self.status = "Delete canceled".to_string();
                    }
                    CategoryInlineConfirmKeyAction::None => {}
                }
                Ok(true)
            }
        }
    }

    fn handle_workflow_setup_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('w') => {
                self.workflow_setup_open = false;
                self.settings.workflow_role_picker = None;
                self.status = "Workflow setup closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.workflow_setup_focus = 1;
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.workflow_setup_focus = 0;
                return Ok(true);
            }
            KeyCode::Char('x') => {
                self.clear_workflow_role(agenda, self.workflow_setup_focus)?;
                return Ok(true);
            }
            KeyCode::Enter => {
                self.open_workflow_role_picker_with_origin(
                    self.workflow_setup_focus,
                    WorkflowRolePickerOrigin::CategoryManager,
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn workflow_role_picker_row_indices(&self) -> Vec<usize> {
        self.category_rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                if row.is_reserved || row.value_kind == CategoryValueKind::Numeric {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect()
    }

    pub(crate) fn open_workflow_role_picker_with_origin(
        &mut self,
        role_index: usize,
        origin: WorkflowRolePickerOrigin,
    ) {
        let row_indices = self.workflow_role_picker_row_indices();
        if row_indices.is_empty() {
            self.status = "No eligible categories available for workflow roles".to_string();
            return;
        }
        let current_role_id = if role_index == 0 {
            self.workflow_config.ready_category_id
        } else {
            self.workflow_config.claim_category_id
        };
        let row_index = current_role_id
            .and_then(|category_id| {
                row_indices.iter().position(|idx| {
                    self.category_rows
                        .get(*idx)
                        .map(|row| row.id == category_id)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(0);
        self.settings.workflow_role_picker = Some(WorkflowRolePickerState {
            role_index,
            row_index,
            origin,
            scroll_offset: ScrollCell::new(0),
        });
        let role_label = if role_index == 0 {
            "Ready Queue"
        } else {
            "Claim Result"
        };
        self.status = format!("{role_label} picker: j/k select category, Enter assign, Esc back");
    }

    pub(crate) fn handle_workflow_role_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let visible_row_indices = self.workflow_role_picker_row_indices();
        let Some(picker) = self.settings.workflow_role_picker.clone() else {
            return Ok(true);
        };
        match code {
            KeyCode::Esc | KeyCode::Char('w') => {
                self.settings.workflow_role_picker = None;
                self.status = "Workflow category picker closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(active_picker) = self.settings.workflow_role_picker.as_mut() {
                    active_picker.row_index =
                        next_index_clamped(picker.row_index, visible_row_indices.len(), 1);
                }
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(active_picker) = self.settings.workflow_role_picker.as_mut() {
                    active_picker.row_index =
                        next_index_clamped(picker.row_index, visible_row_indices.len(), -1);
                }
                return Ok(true);
            }
            KeyCode::Char('x') => {
                self.clear_workflow_role(agenda, picker.role_index)?;
                self.settings.workflow_role_picker = None;
                return Ok(true);
            }
            KeyCode::Enter => {
                let Some(row_idx) = visible_row_indices.get(picker.row_index).copied() else {
                    self.status = "No category selected".to_string();
                    return Ok(true);
                };
                let Some(row) = self.category_rows.get(row_idx).cloned() else {
                    self.status = "No category selected".to_string();
                    return Ok(true);
                };
                let preserved_selection = self.selected_category_id();
                if let Some(status) = self.workflow_setup_cross_role_conflict_status(
                    agenda,
                    picker.role_index,
                    row.id,
                )? {
                    self.status = status;
                    return Ok(true);
                }
                if picker.role_index == 0 {
                    self.assign_ready_queue_role(agenda, row.id, preserved_selection)?;
                } else {
                    self.assign_claim_result_role(agenda, row.id, preserved_selection)?;
                }
                self.settings.workflow_role_picker = None;
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    fn clear_workflow_role(&mut self, agenda: &Agenda<'_>, role_index: usize) -> TuiResult<()> {
        let selected_category_id = self.selected_category_id();
        let mut workflow = self.workflow_config.clone();
        let (role_label, cleared_id) = if role_index == 0 {
            ("Ready Queue", workflow.ready_category_id.take())
        } else {
            ("Claim Result", workflow.claim_category_id.take())
        };
        let Some(cleared_id) = cleared_id else {
            self.status = format!("{role_label} is already unset");
            return Ok(());
        };
        let cleared_name = agenda.store().get_category(cleared_id)?.name;
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        if let Some(category_id) = selected_category_id {
            self.set_category_selection_by_id(category_id);
        }
        self.status = format!("Cleared {role_label} category ({cleared_name})");
        Ok(())
    }

    fn handle_classification_mode_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('m') => {
                self.settings.classification_mode_picker_open = false;
                self.status = "Classification mode picker closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings.classification_mode_picker_focus =
                    next_index_clamped(self.settings.classification_mode_picker_focus, 3, 1);
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings.classification_mode_picker_focus =
                    next_index_clamped(self.settings.classification_mode_picker_focus, 3, -1);
                return Ok(true);
            }
            KeyCode::Enter => {
                let mode = modes::classification::literal_mode_from_index(
                    self.settings.classification_mode_picker_focus,
                );
                self.apply_category_manager_classification_mode(agenda, mode)?;
                self.settings.classification_mode_picker_open = false;
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn handle_category_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        self.ensure_category_manager_session();
        if self.handle_category_manager_inline_action_key(code, agenda)? {
            return Ok(false);
        }
        if self.category_manager_discard_confirm() {
            self.handle_category_manager_discard_confirm_key(code, agenda)?;
            return Ok(false);
        }
        if self.settings.classification_mode_picker_open {
            self.handle_classification_mode_picker_key(code, agenda)?;
            return Ok(false);
        }
        if self.settings.workflow_role_picker.is_some() {
            self.handle_workflow_role_picker_key(code, agenda)?;
            return Ok(false);
        }
        if self.workflow_setup_open {
            self.handle_workflow_setup_key(code, agenda)?;
            return Ok(false);
        }
        if self.category_manager_condition_edit().is_some() {
            self.handle_condition_edit_key(code, agenda)?;
            return Ok(false);
        }
        if self.category_manager_action_edit().is_some() {
            self.handle_action_edit_key(code, agenda)?;
            return Ok(false);
        }
        if self.handle_category_manager_details_key(code, agenda)? {
            return Ok(false);
        }
        if self.category_manager_filter_editing() {
            match code {
                KeyCode::Char('/') => {
                    self.set_category_manager_focus(CategoryManagerFocus::Filter);
                    return Ok(false);
                }
                KeyCode::Esc
                | KeyCode::F(9)
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Down
                | KeyCode::Up => {
                    self.set_category_manager_filter_editing(false);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(filter) = self.category_manager_filter_mut() {
                        if filter.handle_key_event(text_key, false) {
                            self.rebuild_category_manager_visible_rows();
                            let count = self
                                .category_manager_visible_row_indices()
                                .map(|rows| rows.len())
                                .unwrap_or(0);
                            self.status = if count == 0 {
                                "No categories match filter".to_string()
                            } else {
                                format!("Category filter active: {} matches", count)
                            };
                            return Ok(false);
                        }
                    }
                }
            }
        }
        if !matches!(code, KeyCode::Char('<') | KeyCode::Char('>')) {
            self.set_category_manager_structure_move_prefix(None);
        }
        if self.category_manager_save_key_pressed(code)
            && (self.category_manager_details_note_dirty()
                || self.category_manager_details_also_match_dirty())
            && !self.category_manager_details_note_editing()
            && !self.category_manager_details_also_match_editing()
        {
            self.save_category_manager_dirty_details(agenda)?;
            return Ok(false);
        }
        match code {
            KeyCode::Tab => {
                self.handle_category_manager_tab_navigation(false);
            }
            KeyCode::BackTab => {
                self.handle_category_manager_tab_navigation(true);
            }
            KeyCode::Char('/') => {
                self.set_category_manager_focus(CategoryManagerFocus::Filter);
                self.set_category_manager_filter_editing(true);
                self.status = "Category filter: type to narrow list, Esc clears filter".to_string();
            }
            KeyCode::Esc | KeyCode::F(9) => {
                self.set_category_manager_filter_editing(false);
                if self
                    .category_manager_filter_text()
                    .is_some_and(|t| !t.trim().is_empty())
                {
                    if let Some(filter) = self.category_manager_filter_mut() {
                        filter.clear();
                    }
                    self.rebuild_category_manager_visible_rows();
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    self.status = "Category filter cleared".to_string();
                } else if self.category_manager_details_note_dirty()
                    || self.category_manager_details_also_match_dirty()
                {
                    self.set_category_manager_discard_confirm(true);
                    self.status =
                        "Save changes? y:save and close  n:discard  Esc:keep editing".to_string();
                } else {
                    self.close_category_manager_with_status("Category manager closed");
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.set_category_manager_filter_editing(false);
                self.save_category_manager_dirty_details(agenda)?;
                self.move_category_cursor(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_category_manager_filter_editing(false);
                self.save_category_manager_dirty_details(agenda)?;
                self.move_category_cursor(-1)
            }
            KeyCode::Char('K') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.reorder_selected_category_sibling(-1, agenda)?;
            }
            KeyCode::Char('J') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.reorder_selected_category_sibling(1, agenda)?;
            }
            KeyCode::Char('<') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    self.set_category_manager_structure_move_prefix(None);
                    return Ok(false);
                }
                if self.category_manager_structure_move_prefix() == Some('<') {
                    self.set_category_manager_structure_move_prefix(None);
                    self.outdent_selected_category(agenda)?;
                } else {
                    self.set_category_manager_structure_move_prefix(Some('<'));
                    self.status = "Press < again to outdent selected category (<<)".to_string();
                }
            }
            KeyCode::Char('>') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    self.set_category_manager_structure_move_prefix(None);
                    return Ok(false);
                }
                if self.category_manager_structure_move_prefix() == Some('>') {
                    self.set_category_manager_structure_move_prefix(None);
                    self.indent_selected_category_under_previous_sibling(agenda)?;
                } else {
                    self.set_category_manager_structure_move_prefix(Some('>'));
                    self.status = "Press > again to indent selected category (>>)".to_string();
                }
            }
            KeyCode::Char('H') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.outdent_selected_category(agenda)?;
            }
            KeyCode::Char('L') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.indent_selected_category_under_previous_sibling(agenda)?;
            }
            KeyCode::Char('n') => {
                let selected_name = self.selected_category_row().map(|row| row.name.clone());
                let parent_id = self.selected_category_parent_id();
                let status = match selected_name {
                    Some(name) if parent_id.is_some() => {
                        let parent_label = self.category_manager_parent_label(parent_id);
                        format!("Create category at same level as {name} under {parent_label}")
                    }
                    Some(name) => format!("Create top-level category at same level as {name}"),
                    None => "Create top-level category".to_string(),
                };
                self.open_category_create_panel(parent_id, status);
            }
            KeyCode::Char('N') => {
                let selected_name = self.selected_category_row().map(|row| row.name.clone());
                let parent_id = if self.selected_category_is_numeric()
                    || self.selected_category_is_reserved()
                {
                    None
                } else {
                    self.selected_category_id()
                };
                let status = match selected_name {
                    Some(name) if parent_id.is_some() => {
                        format!("Create child category under {name}")
                    }
                    Some(name) => {
                        format!(
                            "{name} cannot have child categories here; creating top-level category"
                        )
                    }
                    None => "Create top-level category".to_string(),
                };
                self.open_category_create_panel(parent_id, status);
            }
            KeyCode::Char('r') => {
                self.start_category_inline_rename();
            }
            KeyCode::Char('e') => {
                if self.selected_category_is_numeric() {
                    self.status = "Exclusive not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_exclusive(agenda)?;
                }
            }
            KeyCode::Char('i') => {
                if self.selected_category_is_numeric() {
                    self.status = "Auto-match not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_implicit(agenda)?;
                }
            }
            KeyCode::Char('g') => {
                if self.selected_category_is_numeric() {
                    self.status =
                        "Match-category-name not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_match_category_name(agenda)?;
                }
            }
            KeyCode::Char('a') => {
                if self.selected_category_is_numeric() {
                    self.status = "Actionable not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_actionable(agenda)?;
                }
            }
            KeyCode::Char('w') => {
                self.status =
                    "Workflow roles moved to Global Settings (return to Normal and use g s or F10)"
                        .to_string();
            }
            KeyCode::Char('m') => {
                self.status =
                    "Classification mode moved to Global Settings (return to Normal and use g s or F10)"
                        .to_string();
            }
            KeyCode::Enter => {
                self.set_category_manager_focus(CategoryManagerFocus::Details);
                self.status =
                    "Details pane focused: use j/k (or arrows) to select field, Enter/Space to edit/toggle"
                        .to_string();
            }
            KeyCode::Char('x') => {
                self.start_category_inline_delete_confirm();
            }
            _ => {}
        }
        Ok(false)
    }

    fn apply_category_manager_classification_mode(
        &mut self,
        agenda: &Agenda<'_>,
        mode: LiteralClassificationMode,
    ) -> TuiResult<()> {
        let mut config = self.classification.ui.config.clone();
        config.literal_mode = mode;
        config.sync_enabled_flag();
        let selected_category_id = self.selected_category_id();
        let manager_focus = self.category_manager_focus();
        let details_focus = self.category_manager_details_focus();
        let mode_label = modes::classification::literal_mode_label(config.literal_mode);

        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.mode = Mode::CategoryManager;
        if let Some(category_id) = selected_category_id {
            self.set_category_selection_by_id(category_id);
        }
        if let Some(focus) = manager_focus {
            self.set_category_manager_focus(focus);
        }
        if let Some(focus) = details_focus {
            self.set_category_manager_details_focus(focus);
        }
        self.status = format!("Literal classification: {mode_label}");
        Ok(())
    }

    pub(crate) fn toggle_selected_category_exclusive(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.is_exclusive = !category.is_exclusive;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} exclusive={} (processed_items={}, affected_items={})",
            updated.name, updated.is_exclusive, result.processed_items, result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_implicit(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.enable_implicit_string = !category.enable_implicit_string;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} auto-match={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_implicit_string,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_semantic(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.enable_semantic_classification = !category.enable_semantic_classification;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} semantic-match={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_semantic_classification,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_match_category_name(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.match_category_name = !category.match_category_name;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} match-category-name={} (processed_items={}, affected_items={})",
            updated.name,
            updated.match_category_name,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_actionable(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.is_actionable = !category.is_actionable;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} actionable={} (processed_items={}, affected_items={})",
            updated.name, updated.is_actionable, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn assign_ready_queue_role(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        selection_after: Option<CategoryId>,
    ) -> TuiResult<()> {
        let Some(row) = self.category_rows.iter().find(|row| row.id == category_id) else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = "Reserved categories cannot be workflow roles".to_string();
            return Ok(());
        }
        if row.value_kind == CategoryValueKind::Numeric {
            self.status = "Workflow roles are not applicable to numeric categories".to_string();
            return Ok(());
        }

        let category = agenda.store().get_category(category_id)?;
        let mut workflow = self.workflow_config.clone();
        if workflow.ready_category_id == Some(category_id) {
            self.status = format!("{} is already the Ready Queue category", category.name);
            return Ok(());
        }
        let previous_ready_category_name = workflow
            .ready_category_id
            .and_then(|existing_id| agenda.store().get_category(existing_id).ok())
            .map(|existing| existing.name);
        if workflow.claim_category_id == Some(category_id) {
            self.status = format!(
                "{} is already the Claim Result category and cannot also be Ready Queue",
                category.name
            );
            return Ok(());
        }

        let prep = Some(self.prepare_category_for_workflow_role(category_id, agenda)?);
        workflow.ready_category_id = Some(category_id);
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(selection_after.unwrap_or(category_id));
        self.status = Self::workflow_role_status_message(
            "Ready Queue",
            &category.name,
            previous_ready_category_name.as_deref(),
            prep.as_ref(),
        );
        Ok(())
    }

    fn assign_claim_result_role(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        selection_after: Option<CategoryId>,
    ) -> TuiResult<()> {
        let Some(row) = self.category_rows.iter().find(|row| row.id == category_id) else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = "Reserved categories cannot be workflow roles".to_string();
            return Ok(());
        }
        if row.value_kind == CategoryValueKind::Numeric {
            self.status = "Workflow roles are not applicable to numeric categories".to_string();
            return Ok(());
        }

        let category = agenda.store().get_category(category_id)?;
        let mut workflow = self.workflow_config.clone();
        if workflow.claim_category_id == Some(category_id) {
            self.status = format!("{} is already the Claim Result category", category.name);
            return Ok(());
        }
        let previous_claim_category_name = workflow
            .claim_category_id
            .and_then(|existing_id| agenda.store().get_category(existing_id).ok())
            .map(|existing| existing.name);
        if workflow.ready_category_id == Some(category_id) {
            self.status = format!(
                "{} is already the Ready Queue category and cannot also be Claim Result",
                category.name
            );
            return Ok(());
        }

        let prep = Some(self.prepare_category_for_workflow_role(category_id, agenda)?);
        workflow.claim_category_id = Some(category_id);
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(selection_after.unwrap_or(category_id));
        self.status = Self::workflow_role_status_message(
            "Claim Result",
            &category.name,
            previous_claim_category_name.as_deref(),
            prep.as_ref(),
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Condition editing (profile conditions)
    // -------------------------------------------------------------------------

    fn open_condition_edit_list(&mut self) {
        if let Some(state) = &mut self.category_manager {
            state.condition_edit = Some(ConditionEditState {
                condition_index: None,
                draft_query: Query::default(),
                list_index: 0,
                picker_open: false,
                picker_index: 0,
                editor_kind: ConditionEditorKind::ProfilePicker,
                draft_date: DateConditionDraft::default(),
            });
        }
        self.condition_list_status();
    }

    fn close_condition_edit(&mut self) {
        if let Some(state) = &mut self.category_manager {
            state.condition_edit = None;
        }
        self.status = "j/k: focus field  Enter/Space: toggle/edit".to_string();
    }

    fn condition_list_status(&mut self) {
        let mode = self
            .selected_category_row()
            .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
            .map(|category| condition_match_mode_label(category.condition_match_mode))
            .unwrap_or("ANY");
        self.status =
            format!("Conditions ({mode}): m:toggle  a:add profile  d:add date  Enter:edit  x:delete  Esc:close");
    }

    fn condition_picker_status(&mut self) {
        self.status =
            "Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Enter:save  Esc:cancel"
                .to_string();
    }

    fn condition_date_status(&mut self) {
        self.status =
            "Tab/Shift-Tab or Up/Down: field  Source/Match h/l: cycle  Enter:save  Esc:cancel".to_string();
    }

    fn open_condition_edit_picker(&mut self, condition_index: Option<usize>) {
        let draft = if let Some(idx) = condition_index {
            // Load existing condition's query
            self.selected_category_row()
                .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
                .and_then(|cat| cat.conditions.get(idx))
                .and_then(|cond| match cond {
                    Condition::Profile { criteria } => Some(criteria.as_ref().clone()),
                    _ => None,
                })
                .unwrap_or_default()
        } else {
            Query::default()
        };
        let initial_picker_index = draft
            .criteria
            .first()
            .and_then(|criterion| {
                self.category_rows
                    .iter()
                    .enumerate()
                    .find(|(_, row)| row.id == criterion.category_id && !row.is_reserved)
                    .map(|(idx, _)| idx)
            })
            .unwrap_or_else(|| first_non_reserved_category_index(&self.category_rows));
        if let Some(state) = &mut self.category_manager {
            if let Some(edit) = &mut state.condition_edit {
                edit.condition_index = condition_index;
                edit.draft_query = draft;
                edit.picker_open = true;
                edit.picker_index = initial_picker_index;
                edit.editor_kind = ConditionEditorKind::ProfilePicker;
            }
        }
        self.condition_picker_status();
    }

    fn open_condition_date_editor(&mut self, condition_index: Option<usize>) {
        let draft = condition_index
            .and_then(|idx| {
                self.selected_category_row()
                    .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
                    .and_then(|cat| cat.conditions.get(idx))
                    .and_then(date_draft_from_condition)
            })
            .unwrap_or_default();
        if let Some(state) = &mut self.category_manager {
            if let Some(edit) = &mut state.condition_edit {
                edit.condition_index = condition_index;
                edit.picker_open = true;
                edit.editor_kind = ConditionEditorKind::DateEditor;
                edit.draft_date = draft;
            }
        }
        self.condition_date_status();
    }

    fn save_condition_from_picker(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let (condition_index, draft_query) = {
            let edit = match self.category_manager_condition_edit() {
                Some(e) => e,
                None => return Ok(false),
            };
            (edit.condition_index, edit.draft_query.clone())
        };

        if draft_query.criteria.is_empty() {
            // Nothing to save — just close the picker
            if let Some(edit) = self.category_manager_condition_edit_mut() {
                edit.picker_open = false;
            }
            self.condition_list_status();
            return Ok(true);
        }

        let category_id = match self.selected_category_row() {
            Some(r) => r.id,
            None => return Ok(false),
        };
        let mut category = match self.categories.iter().find(|c| c.id == category_id) {
            Some(c) => c.clone(),
            None => return Ok(false),
        };

        let new_condition = Condition::Profile {
            criteria: Box::new(draft_query),
        };

        if let Some(idx) = condition_index {
            if idx < category.conditions.len() {
                category.conditions[idx] = new_condition;
            }
        } else {
            category.conditions.push(new_condition);
        }

        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);

        // Close picker, stay in list
        if let Some(edit) = self.category_manager_condition_edit_mut() {
            edit.picker_open = false;
        }
        let action = if condition_index.is_some() {
            "updated"
        } else {
            "added"
        };
        self.status = format!(
            "Condition {} (processed={}, affected={})  a:add  Enter:edit  x:delete  Esc:close",
            action, result.processed_items, result.affected_items
        );
        Ok(true)
    }

    fn cancel_condition_picker(&mut self) -> bool {
        let Some(edit) = self.category_manager_condition_edit_mut() else {
            return false;
        };
        if !edit.picker_open {
            return false;
        }
        edit.picker_open = false;
        self.condition_list_status();
        true
    }

    fn toggle_condition_match_mode(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let category_id = match self.selected_category_row() {
            Some(row) => row.id,
            None => return Ok(false),
        };
        let mut category = match self.categories.iter().find(|c| c.id == category_id) {
            Some(category) => category.clone(),
            None => return Ok(false),
        };
        category.condition_match_mode = match category.condition_match_mode {
            ConditionMatchMode::Any => ConditionMatchMode::All,
            ConditionMatchMode::All => ConditionMatchMode::Any,
        };

        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.condition_list_status();
        self.status = format!(
            "{} rules now use {} matching (processed={}, affected={})",
            category.name,
            condition_match_mode_label(category.condition_match_mode),
            result.processed_items,
            result.affected_items
        );
        Ok(true)
    }

    fn save_condition_from_date_editor(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let (condition_index, draft) = {
            let edit = match self.category_manager_condition_edit() {
                Some(e) => e,
                None => return Ok(false),
            };
            (edit.condition_index, edit.draft_date.clone())
        };

        let feedback = date_condition_draft_feedback(
            &draft,
            &self.selected_category_row().map(|r| r.name.clone()).unwrap_or_default(),
        );
        if let Some(error) = feedback
            .messages
            .iter()
            .find(|message| message.severity == DateDraftMessageSeverity::Error)
        {
            self.status = error.text.clone();
            return Ok(true);
        }

        let matcher = match draft.kind {
            DateConditionDraftKind::Range => agenda_core::model::DateMatcher::Range {
                from: parse_date_value_expr(draft.from_input.trimmed())
                    .map_err(TuiError::App)?,
                through: parse_date_value_expr(draft.through_input.trimmed())
                    .map_err(TuiError::App)?,
            },
            DateConditionDraftKind::Compare(op) => agenda_core::model::DateMatcher::Compare {
                op,
                value: parse_date_value_expr(draft.value_input.trimmed())
                    .map_err(TuiError::App)?,
            },
            DateConditionDraftKind::TodayAfter => agenda_core::model::DateMatcher::Range {
                from: parse_time_today_value_expr(draft.value_input.trimmed())
                    .map_err(TuiError::App)?,
                through: DateValueExpr::Today,
            },
            DateConditionDraftKind::TodayBefore => agenda_core::model::DateMatcher::Range {
                from: DateValueExpr::Today,
                through: parse_time_today_value_expr(draft.value_input.trimmed())
                    .map_err(TuiError::App)?,
            },
            DateConditionDraftKind::ThisAfternoon => agenda_core::model::DateMatcher::Range {
                from: DateValueExpr::TimeToday(default_afternoon_start()),
                through: DateValueExpr::Today,
            },
        };

        let category_id = match self.selected_category_row() {
            Some(r) => r.id,
            None => return Ok(false),
        };
        let mut category = match self.categories.iter().find(|c| c.id == category_id) {
            Some(c) => c.clone(),
            None => return Ok(false),
        };

        let new_condition = Condition::Date {
            source: draft.source,
            matcher,
        };

        if let Some(idx) = condition_index {
            if idx < category.conditions.len() {
                category.conditions[idx] = new_condition;
            }
        } else {
            category.conditions.push(new_condition);
        }

        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);

        if let Some(edit) = self.category_manager_condition_edit_mut() {
            edit.picker_open = false;
        }
        let action = if condition_index.is_some() {
            "updated"
        } else {
            "added"
        };
        self.status = format!(
            "Condition {} (processed={}, affected={})  m:toggle  a:add profile  d:add date  Enter:edit  x:delete  Esc:close",
            action, result.processed_items, result.affected_items
        );
        Ok(true)
    }

    fn delete_condition_at_list_index(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let list_index = match self.category_manager_condition_edit() {
            Some(e) => e.list_index,
            None => return Ok(false),
        };
        let category_id = match self.selected_category_row() {
            Some(r) => r.id,
            None => return Ok(false),
        };
        let mut category = match self.categories.iter().find(|c| c.id == category_id) {
            Some(c) => c.clone(),
            None => return Ok(false),
        };

        let condition_indices = editable_condition_indices(&category);

        if list_index >= condition_indices.len() {
            return Ok(false);
        }
        let actual_index = condition_indices[list_index];
        category.conditions.remove(actual_index);

        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);

        // Adjust list_index if it's now out of bounds
        if let Some(edit) = self.category_manager_condition_edit_mut() {
            let new_count = condition_indices.len() - 1;
            if edit.list_index >= new_count && new_count > 0 {
                edit.list_index = new_count - 1;
            }
        }
        self.status = format!(
            "Condition removed (processed={}, affected={})  m:toggle  a:add profile  d:add date  Enter:edit  x:delete  Esc:close",
            result.processed_items, result.affected_items
        );
        Ok(true)
    }

    fn condition_edit_filtered_category_indices(&self) -> Vec<usize> {
        self.category_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !row.is_reserved)
            .map(|(i, _)| i)
            .collect()
    }

    fn set_condition_picker_mode(
        &mut self,
        row: &CategoryListRow,
        requested_mode: Option<CriterionMode>,
    ) {
        let Some(category_id) = self.selected_category_row().map(|selected| selected.id) else {
            return;
        };
        let status_override = {
            let Some(edit) = self.category_manager_condition_edit_mut() else {
                return;
            };
            let current_mode = edit.draft_query.mode_for(row.id);

            if row.id == category_id {
                match (current_mode, requested_mode) {
                    (_, None) => {
                        edit.draft_query.remove_criterion(row.id);
                        None
                    }
                    (None, Some(_)) => Some(format!(
                        "{} can't depend on itself  0:clear  Enter:save  Esc:cancel",
                        row.name
                    )),
                    (Some(current), Some(mode)) if current == mode => {
                        edit.draft_query.remove_criterion(row.id);
                        None
                    }
                    (Some(_), Some(_)) => Some(format!(
                        "{} can't depend on itself  0:clear  Enter:save  Esc:cancel",
                        row.name
                    )),
                }
            } else {
                match requested_mode {
                    Some(mode) if current_mode == Some(mode) => {
                        edit.draft_query.remove_criterion(row.id);
                    }
                    Some(mode) => {
                        edit.draft_query.set_criterion(mode, row.id);
                    }
                    None => {
                        edit.draft_query.remove_criterion(row.id);
                    }
                }
                None
            }
        };

        if let Some(status) = status_override {
            self.status = status;
        } else {
            self.condition_picker_status();
        }
    }

    fn handle_condition_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let edit = match self.category_manager_condition_edit() {
            Some(e) => e.clone(),
            None => return Ok(false),
        };

        if edit.picker_open {
            match edit.editor_kind {
                ConditionEditorKind::ProfilePicker => {
                    self.handle_condition_picker_key(code, agenda, &edit)
                }
                ConditionEditorKind::DateEditor => {
                    self.handle_condition_date_key(code, agenda, &edit)
                }
            }
        } else {
            self.handle_condition_list_key(code, agenda, &edit)
        }
    }

    fn handle_condition_list_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
        edit: &ConditionEditState,
    ) -> TuiResult<bool> {
        let condition_count = self
            .selected_category_row()
            .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
            .map(|cat| editable_condition_indices(cat).len())
            .unwrap_or(0);

        match code {
            KeyCode::Esc => {
                self.close_condition_edit();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if condition_count > 0 {
                    if let Some(e) = self.category_manager_condition_edit_mut() {
                        e.list_index = (e.list_index + 1).min(condition_count.saturating_sub(1));
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(e) = self.category_manager_condition_edit_mut() {
                    e.list_index = e.list_index.saturating_sub(1);
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.open_condition_edit_picker(None);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.open_condition_date_editor(None);
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                return self.toggle_condition_match_mode(agenda);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if condition_count > 0 && edit.list_index < condition_count {
                    let actual_index = self
                        .selected_category_row()
                        .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
                        .and_then(|cat| editable_condition_indices(cat).get(edit.list_index).copied());
                    if let Some(idx) = actual_index {
                        let selected_condition = self
                            .selected_category_row()
                            .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
                            .and_then(|cat| cat.conditions.get(idx));
                        match selected_condition {
                            Some(Condition::Date { .. }) => self.open_condition_date_editor(Some(idx)),
                            _ => self.open_condition_edit_picker(Some(idx)),
                        }
                    }
                } else {
                    self.open_condition_edit_picker(None);
                }
            }
            KeyCode::Char('x') => {
                if condition_count > 0 && edit.list_index < condition_count {
                    self.delete_condition_at_list_index(agenda)?;
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn cycle_date_condition_source(source: DateSource, forward: bool) -> DateSource {
        match (source, forward) {
            (DateSource::When, true) => DateSource::Entry,
            (DateSource::Entry, true) => DateSource::Done,
            (DateSource::Done, true) => DateSource::When,
            (DateSource::When, false) => DateSource::Done,
            (DateSource::Entry, false) => DateSource::When,
            (DateSource::Done, false) => DateSource::Entry,
        }
    }

    fn handle_condition_date_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
        _edit: &ConditionEditState,
    ) -> TuiResult<bool> {
        let editable_focus = self
            .category_manager_condition_edit()
            .map(|edit| match edit.draft_date.field_focus {
                DateConditionField::Value => draft_uses_value_field(edit.draft_date.kind),
                DateConditionField::From | DateConditionField::Through => {
                    draft_uses_range_fields(edit.draft_date.kind)
                }
                DateConditionField::Source | DateConditionField::Match => false,
            })
            .unwrap_or(false);
        match code {
            KeyCode::Esc => {
                self.cancel_condition_picker();
                return Ok(true);
            }
            KeyCode::Enter => {
                self.normalize_focused_date_condition_field();
                self.save_condition_from_date_editor(agenda)?;
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Tab => {
                self.normalize_focused_date_condition_field();
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    edit.draft_date.field_focus = match edit.draft_date.field_focus {
                        DateConditionField::Source => DateConditionField::Match,
                        DateConditionField::Match => {
                            if draft_uses_range_fields(edit.draft_date.kind) {
                                DateConditionField::From
                            } else if draft_uses_value_field(edit.draft_date.kind) {
                                DateConditionField::Value
                            } else {
                                DateConditionField::Source
                            }
                        }
                        DateConditionField::Value => DateConditionField::Source,
                        DateConditionField::From => DateConditionField::Through,
                        DateConditionField::Through => DateConditionField::Source,
                    };
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.normalize_focused_date_condition_field();
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    edit.draft_date.field_focus = match edit.draft_date.field_focus {
                        DateConditionField::Source => {
                            if draft_uses_range_fields(edit.draft_date.kind) {
                                DateConditionField::Through
                            } else if draft_uses_value_field(edit.draft_date.kind) {
                                DateConditionField::Value
                            } else {
                                DateConditionField::Match
                            }
                        }
                        DateConditionField::Match => DateConditionField::Source,
                        DateConditionField::Value => DateConditionField::Match,
                        DateConditionField::From => DateConditionField::Match,
                        DateConditionField::Through => DateConditionField::From,
                    };
                }
            }
            KeyCode::Char('j') if !editable_focus => {
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    edit.draft_date.field_focus = match edit.draft_date.field_focus {
                        DateConditionField::Source => DateConditionField::Match,
                        DateConditionField::Match => {
                            if draft_uses_range_fields(edit.draft_date.kind) {
                                DateConditionField::From
                            } else if draft_uses_value_field(edit.draft_date.kind) {
                                DateConditionField::Value
                            } else {
                                DateConditionField::Source
                            }
                        }
                        DateConditionField::Value => DateConditionField::Source,
                        DateConditionField::From => DateConditionField::Through,
                        DateConditionField::Through => DateConditionField::Source,
                    };
                }
            }
            KeyCode::Char('k') if !editable_focus => {
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    edit.draft_date.field_focus = match edit.draft_date.field_focus {
                        DateConditionField::Source => {
                            if draft_uses_range_fields(edit.draft_date.kind) {
                                DateConditionField::Through
                            } else if draft_uses_value_field(edit.draft_date.kind) {
                                DateConditionField::Value
                            } else {
                                DateConditionField::Match
                            }
                        }
                        DateConditionField::Match => DateConditionField::Source,
                        DateConditionField::Value => DateConditionField::Match,
                        DateConditionField::From => DateConditionField::Match,
                        DateConditionField::Through => DateConditionField::From,
                    };
                }
            }
            KeyCode::Left | KeyCode::Char('h') if !editable_focus => {
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    match edit.draft_date.field_focus {
                        DateConditionField::Source => {
                            edit.draft_date.source =
                                Self::cycle_date_condition_source(edit.draft_date.source, false);
                        }
                        DateConditionField::Match => {
                            cycle_date_match_mode(&mut edit.draft_date, false);
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') if !editable_focus => {
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    match edit.draft_date.field_focus {
                        DateConditionField::Source => {
                            edit.draft_date.source =
                                Self::cycle_date_condition_source(edit.draft_date.source, true);
                        }
                        DateConditionField::Match => {
                            cycle_date_match_mode(&mut edit.draft_date, true);
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                let text_key = editable_focus.then(|| self.text_key_event(code));
                if let Some(edit) = self.category_manager_condition_edit_mut() {
                    if editable_focus {
                        match edit.draft_date.field_focus {
                            DateConditionField::Value if draft_uses_value_field(edit.draft_date.kind) => {
                                edit.draft_date
                                    .value_input
                                    .handle_key_event(text_key.expect("text key"), false);
                            }
                            DateConditionField::From if draft_uses_range_fields(edit.draft_date.kind) => {
                                edit.draft_date
                                    .from_input
                                    .handle_key_event(text_key.expect("text key"), false);
                            }
                            DateConditionField::Through if draft_uses_range_fields(edit.draft_date.kind) => {
                                edit.draft_date
                                    .through_input
                                    .handle_key_event(text_key.expect("text key"), false);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        self.condition_date_status();
        Ok(true)
    }

    fn handle_condition_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
        edit: &ConditionEditState,
    ) -> TuiResult<bool> {
        let filtered_indices = self.condition_edit_filtered_category_indices();
        let current_visible_pos = filtered_indices
            .iter()
            .position(|&idx| idx == edit.picker_index)
            .unwrap_or(0);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(&actual_idx) = filtered_indices
                    .get((current_visible_pos + 1).min(filtered_indices.len().saturating_sub(1)))
                {
                    if let Some(e) = self.category_manager_condition_edit_mut() {
                        e.picker_index = actual_idx;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(&actual_idx) =
                    filtered_indices.get(current_visible_pos.saturating_sub(1))
                {
                    if let Some(e) = self.category_manager_condition_edit_mut() {
                        e.picker_index = actual_idx;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        let current_mode = self
                            .category_manager_condition_edit()
                            .and_then(|e| e.draft_query.mode_for(row.id));
                        let next = match current_mode {
                            None => Some(CriterionMode::And),
                            Some(CriterionMode::And) => Some(CriterionMode::Not),
                            Some(CriterionMode::Not) => Some(CriterionMode::Or),
                            Some(CriterionMode::Or) => None,
                        };
                        self.set_condition_picker_mode(&row, next);
                    }
                }
            }
            KeyCode::Enter => {
                self.save_condition_from_picker(agenda)?;
            }
            KeyCode::Char('1') | KeyCode::Char('+') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        self.set_condition_picker_mode(&row, Some(CriterionMode::And));
                    }
                }
            }
            KeyCode::Char('2') | KeyCode::Char('-') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        self.set_condition_picker_mode(&row, Some(CriterionMode::Not));
                    }
                }
            }
            KeyCode::Char('3') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        self.set_condition_picker_mode(&row, Some(CriterionMode::Or));
                    }
                }
            }
            KeyCode::Char('0') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        self.set_condition_picker_mode(&row, None);
                    }
                }
            }
            KeyCode::Esc => {
                self.cancel_condition_picker();
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Action editing
    // -------------------------------------------------------------------------

    fn open_action_edit_list(&mut self) {
        if let Some(state) = &mut self.category_manager {
            state.action_edit = Some(ActionEditState {
                action_index: None,
                draft_kind: ActionEditKind::Assign,
                draft_targets: HashSet::new(),
                list_index: 0,
                picker_open: false,
                picker_index: 0,
            });
        }
        self.status = "Actions: a:add  Enter:edit  x:delete  Esc:close".to_string();
    }

    fn close_action_edit(&mut self) {
        if let Some(state) = &mut self.category_manager {
            state.action_edit = None;
        }
        self.status = "j/k: focus field  Enter/Space: toggle/edit".to_string();
    }

    fn action_list_status(&mut self) {
        self.status = "Actions: a:add  Enter:edit  x:delete  Esc:close".to_string();
    }

    fn action_picker_status(&mut self) {
        self.status = "Space:toggle target  1:Assign  2:Remove  Enter:save  Esc:cancel".to_string();
    }

    fn open_action_edit_picker(&mut self, action_index: Option<usize>) {
        let (draft_kind, draft_targets) = if let Some(idx) = action_index {
            self.selected_category_row()
                .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
                .and_then(|cat| cat.actions.get(idx))
                .map(|action| match action {
                    Action::Assign { targets } => (ActionEditKind::Assign, targets.clone()),
                    Action::Remove { targets } => (ActionEditKind::Remove, targets.clone()),
                })
                .unwrap_or((ActionEditKind::Assign, HashSet::new()))
        } else {
            (ActionEditKind::Assign, HashSet::new())
        };
        let initial_picker_index = draft_targets
            .iter()
            .next()
            .and_then(|target_id| {
                self.category_rows
                    .iter()
                    .enumerate()
                    .find(|(_, row)| row.id == *target_id && !row.is_reserved)
                    .map(|(idx, _)| idx)
            })
            .unwrap_or_else(|| first_non_reserved_category_index(&self.category_rows));
        if let Some(state) = &mut self.category_manager {
            if let Some(edit) = &mut state.action_edit {
                edit.action_index = action_index;
                edit.draft_kind = draft_kind;
                edit.draft_targets = draft_targets;
                edit.picker_open = true;
                edit.picker_index = initial_picker_index;
            }
        }
        self.action_picker_status();
    }

    fn save_action_from_picker(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let (action_index, draft_kind, draft_targets) = {
            let edit = match self.category_manager_action_edit() {
                Some(e) => e,
                None => return Ok(false),
            };
            (
                edit.action_index,
                edit.draft_kind,
                edit.draft_targets.clone(),
            )
        };

        if draft_targets.is_empty() {
            if let Some(edit) = self.category_manager_action_edit_mut() {
                edit.picker_open = false;
            }
            self.action_list_status();
            return Ok(true);
        }

        let category_id = match self.selected_category_row() {
            Some(r) => r.id,
            None => return Ok(false),
        };

        let new_action = match draft_kind {
            ActionEditKind::Assign => Action::Assign {
                targets: draft_targets,
            },
            ActionEditKind::Remove => Action::Remove {
                targets: draft_targets,
            },
        };

        let result = if let Some(idx) = action_index {
            agenda.update_category_action(category_id, idx, new_action)?
        } else {
            agenda.add_category_action(category_id, new_action)?.1
        };
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);

        if let Some(edit) = self.category_manager_action_edit_mut() {
            edit.picker_open = false;
        }
        let action = if action_index.is_some() {
            "updated"
        } else {
            "added"
        };
        self.status = format!(
            "Action {} (processed={}, affected={})  a:add  Enter:edit  x:delete  Esc:close",
            action, result.processed_items, result.affected_items
        );
        Ok(true)
    }

    fn cancel_action_picker(&mut self) -> bool {
        let Some(edit) = self.category_manager_action_edit_mut() else {
            return false;
        };
        if !edit.picker_open {
            return false;
        }
        edit.picker_open = false;
        self.action_list_status();
        true
    }

    fn delete_action_at_list_index(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let list_index = match self.category_manager_action_edit() {
            Some(e) => e.list_index,
            None => return Ok(false),
        };
        let category_id = match self.selected_category_row() {
            Some(r) => r.id,
            None => return Ok(false),
        };
        let category = match self.categories.iter().find(|c| c.id == category_id) {
            Some(c) => c.clone(),
            None => return Ok(false),
        };

        if list_index >= category.actions.len() {
            return Ok(false);
        }

        let (_, result) = agenda.remove_category_action(category_id, list_index)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);

        if let Some(edit) = self.category_manager_action_edit_mut() {
            let new_count = category.actions.len();
            if edit.list_index >= new_count && new_count > 0 {
                edit.list_index = new_count - 1;
            }
        }
        self.status = format!(
            "Action removed (processed={}, affected={})  a:add  Enter:edit  x:delete  Esc:close",
            result.processed_items, result.affected_items
        );
        Ok(true)
    }

    fn set_action_picker_kind(&mut self, kind: ActionEditKind) {
        if let Some(edit) = self.category_manager_action_edit_mut() {
            edit.draft_kind = kind;
        }
        self.action_picker_status();
    }

    fn toggle_action_picker_target(&mut self, row: &CategoryListRow) {
        let Some(selected_category_id) = self.selected_category_row().map(|selected| selected.id)
        else {
            return;
        };
        let Some(edit) = self.category_manager_action_edit_mut() else {
            return;
        };
        if row.id == selected_category_id {
            self.status = format!(
                "{} can't target itself  1:Assign  2:Remove  Enter:save  Esc:cancel",
                row.name
            );
            return;
        }
        if !edit.draft_targets.insert(row.id) {
            edit.draft_targets.remove(&row.id);
        }
        self.action_picker_status();
    }

    fn action_edit_filtered_category_indices(&self) -> Vec<usize> {
        self.category_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !row.is_reserved)
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_action_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> TuiResult<bool> {
        let edit = match self.category_manager_action_edit() {
            Some(e) => e.clone(),
            None => return Ok(false),
        };

        if edit.picker_open {
            self.handle_action_picker_key(code, agenda, &edit)
        } else {
            self.handle_action_list_key(code, agenda, &edit)
        }
    }

    fn handle_action_list_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
        edit: &ActionEditState,
    ) -> TuiResult<bool> {
        let action_count = self
            .selected_category_row()
            .and_then(|row| self.categories.iter().find(|c| c.id == row.id))
            .map(|cat| cat.actions.len())
            .unwrap_or(0);

        match code {
            KeyCode::Esc => self.close_action_edit(),
            KeyCode::Char('j') | KeyCode::Down => {
                if action_count > 0 {
                    if let Some(e) = self.category_manager_action_edit_mut() {
                        e.list_index = (e.list_index + 1).min(action_count.saturating_sub(1));
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(e) = self.category_manager_action_edit_mut() {
                    e.list_index = e.list_index.saturating_sub(1);
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_action_edit_picker(None),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if action_count > 0 && edit.list_index < action_count {
                    self.open_action_edit_picker(Some(edit.list_index));
                } else {
                    self.open_action_edit_picker(None);
                }
            }
            KeyCode::Char('x') | KeyCode::Char('d') => {
                if action_count > 0 && edit.list_index < action_count {
                    self.delete_action_at_list_index(agenda)?;
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_action_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
        edit: &ActionEditState,
    ) -> TuiResult<bool> {
        let filtered_indices = self.action_edit_filtered_category_indices();
        let current_visible_pos = filtered_indices
            .iter()
            .position(|&idx| idx == edit.picker_index)
            .unwrap_or(0);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(&actual_idx) = filtered_indices
                    .get((current_visible_pos + 1).min(filtered_indices.len().saturating_sub(1)))
                {
                    if let Some(e) = self.category_manager_action_edit_mut() {
                        e.picker_index = actual_idx;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(&actual_idx) =
                    filtered_indices.get(current_visible_pos.saturating_sub(1))
                {
                    if let Some(e) = self.category_manager_action_edit_mut() {
                        e.picker_index = actual_idx;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                    if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                        self.toggle_action_picker_target(&row);
                    }
                }
            }
            KeyCode::Enter => {
                self.save_action_from_picker(agenda)?;
            }
            KeyCode::Char('1') => self.set_action_picker_kind(ActionEditKind::Assign),
            KeyCode::Char('2') => self.set_action_picker_kind(ActionEditKind::Remove),
            KeyCode::Esc => {
                self.cancel_action_picker();
            }
            _ => {}
        }
        Ok(true)
    }
}
