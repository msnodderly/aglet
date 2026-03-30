use std::time::{Duration, Instant};

use agenda_core::agenda::Agenda;
use agenda_core::model::{
    Assignment, AssignmentSource, CategoryId, Item, ItemId, ItemLinkKind,
};
use jiff::Timestamp;
use rust_decimal::Decimal;

use crate::{truncate_str, App, TransientStatus, TuiResult};

const UNDO_STACK_MAX: usize = 50;

#[derive(Default)]
pub(crate) struct UndoState {
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

impl UndoState {
    pub(crate) fn push(&mut self, entry: UndoEntry) {
        self.redo_stack.clear();
        self.undo_stack.push(entry);
        if self.undo_stack.len() > UNDO_STACK_MAX {
            self.undo_stack.remove(0);
        }
    }

    fn pop_undo(&mut self) -> Option<UndoEntry> {
        self.undo_stack.pop()
    }

    fn pop_redo(&mut self) -> Option<UndoEntry> {
        self.redo_stack.pop()
    }

    fn push_redo(&mut self, entry: UndoEntry) {
        self.redo_stack.push(entry);
        if self.redo_stack.len() > UNDO_STACK_MAX {
            self.redo_stack.remove(0);
        }
    }

    fn push_undo_direct(&mut self, entry: UndoEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > UNDO_STACK_MAX {
            self.undo_stack.remove(0);
        }
    }

    pub(crate) fn has_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub(crate) fn has_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

/// Captures enough state to reverse a single TUI mutation.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum UndoEntry {
    ItemCreated {
        item_id: ItemId,
    },
    ItemEdited {
        item_id: ItemId,
        old_text: String,
        old_note: Option<String>,
    },
    ItemDeleted {
        item: Box<Item>,
    },
    ItemDoneToggled {
        item_id: ItemId,
        was_done: bool,
    },
    CategoryAssigned {
        item_id: ItemId,
        category_id: CategoryId,
    },
    CategoryUnassigned {
        item_id: ItemId,
        category_id: CategoryId,
        old_assignment: Assignment,
    },
    NumericValueSet {
        item_id: ItemId,
        category_id: CategoryId,
        old_value: Option<Decimal>,
    },
    LinkCreated {
        item_id: ItemId,
        other_id: ItemId,
        kind: ItemLinkKind,
    },
    LinkRemoved {
        item_id: ItemId,
        other_id: ItemId,
        kind: ItemLinkKind,
    },
    BatchDone {
        item_ids: Vec<ItemId>,
    },
}

impl UndoEntry {
    fn description(&self) -> String {
        match self {
            Self::ItemCreated { .. } => "item creation".to_string(),
            Self::ItemEdited { .. } => "item edit".to_string(),
            Self::ItemDeleted { item } => {
                format!("deletion of \"{}\"", truncate_str(&item.text, 30))
            }
            Self::ItemDoneToggled { was_done, .. } => {
                if *was_done {
                    "mark undone".to_string()
                } else {
                    "mark done".to_string()
                }
            }
            Self::CategoryAssigned { .. } => "category assignment".to_string(),
            Self::CategoryUnassigned { .. } => "category unassignment".to_string(),
            Self::NumericValueSet { .. } => "numeric value change".to_string(),
            Self::LinkCreated { .. } => "link creation".to_string(),
            Self::LinkRemoved { .. } => "link removal".to_string(),
            Self::BatchDone { item_ids } => format!("batch done ({} items)", item_ids.len()),
        }
    }
}

impl App {
    pub(crate) fn push_undo(&mut self, entry: UndoEntry) {
        self.undo.push(entry);
    }

    /// Apply an undo/redo entry, returning the inverse entry that can reverse it.
    fn apply_entry(&mut self, entry: UndoEntry, agenda: &Agenda<'_>) -> TuiResult<UndoEntry> {
        let inverse = match entry {
            UndoEntry::ItemCreated { item_id } => {
                let item = agenda.store().get_item(item_id)?;
                agenda.delete_item(item_id, "undo")?;
                UndoEntry::ItemDeleted {
                    item: Box::new(item),
                }
            }
            UndoEntry::ItemEdited {
                item_id,
                old_text,
                old_note,
            } => {
                let mut item = agenda.store().get_item(item_id)?;
                let inverse = UndoEntry::ItemEdited {
                    item_id,
                    old_text: item.text.clone(),
                    old_note: item.note.clone(),
                };
                item.text = old_text;
                item.note = old_note;
                item.modified_at = Timestamp::now();
                let reference_date = jiff::Zoned::now().date();
                agenda.update_item_with_reference_date(&item, reference_date)?;
                inverse
            }
            UndoEntry::ItemDeleted { item } => {
                let item_id = item.id;
                let reference_date = jiff::Zoned::now().date();
                agenda.create_item_with_reference_date(&item, reference_date)?;
                let mut restored = agenda.store().get_item(item.id)?;
                restored.note = item.note.clone();
                restored.is_done = item.is_done;
                restored.done_date = item.done_date;
                restored.modified_at = Timestamp::now();
                agenda.update_item_with_reference_date(&restored, reference_date)?;
                for (cat_id, assignment) in &item.assignments {
                    if assignment.source == AssignmentSource::Manual {
                        if let Some(numeric_value) = assignment.numeric_value {
                            let _ = agenda.assign_item_numeric_manual(
                                item.id,
                                *cat_id,
                                numeric_value,
                                Some("undo:restore".to_string()),
                            );
                        } else {
                            let _ = agenda.assign_item_manual(
                                item.id,
                                *cat_id,
                                Some("undo:restore".to_string()),
                            );
                        }
                    }
                }
                self.set_item_selection_by_id(item.id);
                UndoEntry::ItemCreated { item_id }
            }
            UndoEntry::ItemDoneToggled { item_id, was_done } => {
                agenda.toggle_item_done(item_id)?;
                self.set_item_selection_by_id(item_id);
                UndoEntry::ItemDoneToggled {
                    item_id,
                    was_done: !was_done,
                }
            }
            UndoEntry::CategoryAssigned {
                item_id,
                category_id,
            } => {
                let old_assignment = agenda
                    .store()
                    .get_item(item_id)
                    .ok()
                    .and_then(|item| item.assignments.get(&category_id).cloned())
                    .unwrap_or(Assignment {
                        source: AssignmentSource::Manual,
                        assigned_at: Timestamp::now(),
                        sticky: false,
                        origin: None,
                        explanation: None,
                        numeric_value: None,
                    });
                let _ = agenda.unassign_item_manual(item_id, category_id);
                self.set_item_selection_by_id(item_id);
                UndoEntry::CategoryUnassigned {
                    item_id,
                    category_id,
                    old_assignment,
                }
            }
            UndoEntry::CategoryUnassigned {
                item_id,
                category_id,
                old_assignment,
            } => {
                if let Some(numeric_value) = old_assignment.numeric_value {
                    let _ = agenda.assign_item_numeric_manual(
                        item_id,
                        category_id,
                        numeric_value,
                        Some("undo:restore".to_string()),
                    );
                } else {
                    let _ = agenda.assign_item_manual(
                        item_id,
                        category_id,
                        Some("undo:restore".to_string()),
                    );
                }
                self.set_item_selection_by_id(item_id);
                UndoEntry::CategoryAssigned {
                    item_id,
                    category_id,
                }
            }
            UndoEntry::NumericValueSet {
                item_id,
                category_id,
                old_value,
            } => {
                let current_value = agenda.store().get_item(item_id).ok().and_then(|item| {
                    item.assignments
                        .get(&category_id)
                        .and_then(|assignment| assignment.numeric_value)
                });
                if let Some(val) = old_value {
                    let _ = agenda.assign_item_numeric_manual(
                        item_id,
                        category_id,
                        val,
                        Some("undo:restore".to_string()),
                    );
                } else {
                    let _ = agenda.unassign_item_manual(item_id, category_id);
                }
                self.set_item_selection_by_id(item_id);
                UndoEntry::NumericValueSet {
                    item_id,
                    category_id,
                    old_value: current_value,
                }
            }
            UndoEntry::LinkCreated {
                item_id,
                other_id,
                kind,
            } => {
                match kind {
                    ItemLinkKind::DependsOn => {
                        let _ = agenda.unlink_items_depends_on(item_id, other_id);
                    }
                    ItemLinkKind::Related => {
                        let _ = agenda.unlink_items_related(item_id, other_id);
                    }
                }
                self.set_item_selection_by_id(item_id);
                UndoEntry::LinkRemoved {
                    item_id,
                    other_id,
                    kind,
                }
            }
            UndoEntry::LinkRemoved {
                item_id,
                other_id,
                kind,
            } => {
                match kind {
                    ItemLinkKind::DependsOn => {
                        let _ = agenda.link_items_depends_on(item_id, other_id);
                    }
                    ItemLinkKind::Related => {
                        let _ = agenda.link_items_related(item_id, other_id);
                    }
                }
                self.set_item_selection_by_id(item_id);
                UndoEntry::LinkCreated {
                    item_id,
                    other_id,
                    kind,
                }
            }
            UndoEntry::BatchDone { item_ids } => {
                for item_id in &item_ids {
                    let _ = agenda.toggle_item_done(*item_id);
                }
                UndoEntry::BatchDone {
                    item_ids: item_ids.clone(),
                }
            }
        };
        self.refresh(agenda.store())?;
        Ok(inverse)
    }

    pub(crate) fn apply_undo(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(entry) = self.undo.pop_undo() else {
            self.status = "Nothing to undo".to_string();
            return Ok(());
        };
        let desc = entry.description();
        let inverse = self.apply_entry(entry, agenda)?;
        self.undo.push_redo(inverse);
        self.transient.status = Some(TransientStatus {
            message: format!("Undid {}", desc),
            expires_at: Instant::now() + Duration::from_secs(3),
        });
        Ok(())
    }

    pub(crate) fn apply_redo(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(entry) = self.undo.pop_redo() else {
            self.status = "Nothing to redo".to_string();
            return Ok(());
        };
        let desc = entry.description();
        let inverse = self.apply_entry(entry, agenda)?;
        self.undo.push_undo_direct(inverse);
        self.transient.status = Some(TransientStatus {
            message: format!("Redid {}", desc),
            expires_at: Instant::now() + Duration::from_secs(3),
        });
        Ok(())
    }
}
