//! Item delete confirmation (`x`, then `y`).

use crate::*;

impl App {
    pub(crate) fn handle_confirm_delete_key(
        &mut self,
        key: KeyEvent,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        let code = key.code;
        if let Some(done_confirm) = self.done_blocks_confirm.clone() {
            match code {
                KeyCode::Char('y') => {
                    self.done_blocks_confirm = None;
                    match done_confirm.scope {
                        DoneBlocksConfirmScope::Single {
                            item_id,
                            blocked_item_ids,
                        } => {
                            self.apply_done_toggle_action(
                                aglet,
                                item_id,
                                false,
                                done_confirm.origin,
                                &blocked_item_ids,
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                aglet,
                                &item_ids,
                                true,
                                done_confirm.origin,
                            )?;
                        }
                    }
                }
                KeyCode::Char('n') => {
                    self.done_blocks_confirm = None;
                    match done_confirm.scope {
                        DoneBlocksConfirmScope::Single { item_id, .. } => {
                            self.apply_done_toggle_action(
                                aglet,
                                item_id,
                                false,
                                done_confirm.origin,
                                &[],
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                aglet,
                                &item_ids,
                                false,
                                done_confirm.origin,
                            )?;
                        }
                    }
                }
                KeyCode::Esc => {
                    self.done_blocks_confirm = None;
                    self.mode = Self::done_toggle_return_mode(done_confirm.origin);
                    self.status = "Done update canceled".to_string();
                }
                _ => {}
            }
            return Ok(false);
        }

        if let Some(batch_delete_item_ids) = self.batch_delete_item_ids.clone() {
            match code {
                KeyCode::Char('y') => {
                    // Capture items for undo before deletion
                    let mut captured_items = Vec::new();
                    for item_id in &batch_delete_item_ids {
                        if let Ok(item) = aglet.store().get_item(*item_id) {
                            captured_items.push(item);
                        }
                    }
                    let mut deleted = 0usize;
                    let mut failed = 0usize;
                    let mut first_error = None;
                    for item_id in &batch_delete_item_ids {
                        match aglet.delete_item(*item_id, "user:tui") {
                            Ok(()) => deleted += 1,
                            Err(err) => {
                                failed += 1;
                                if first_error.is_none() {
                                    first_error = Some(err.to_string());
                                }
                            }
                        }
                    }
                    // Push undo entries for successfully deleted items (reverse
                    // order so undoing pops them back in original order)
                    for item in captured_items.into_iter().rev() {
                        self.push_undo(UndoEntry::ItemDeleted {
                            item: Box::new(item),
                        });
                    }
                    self.batch_delete_item_ids = None;
                    self.clear_selected_items();
                    self.refresh(aglet.store())?;
                    self.mode = Mode::Normal;
                    self.status = if failed == 0 {
                        format!("Deleted {deleted} selected items")
                    } else {
                        let mut summary =
                            format!("Deleted {deleted} selected items (failed={failed})");
                        if let Some(err) = first_error {
                            summary.push_str(&format!(" first_error={err}"));
                        }
                        summary
                    };
                }
                KeyCode::Esc => {
                    self.batch_delete_item_ids = None;
                    self.mode = Mode::Normal;
                    self.status = "Batch delete canceled".to_string();
                }
                _ => {}
            }
            return Ok(false);
        }

        match code {
            KeyCode::Char('y') => {
                if let Some(item_id) = self.selected_item_id() {
                    // Capture item state for undo before deletion
                    if let Ok(item) = aglet.store().get_item(item_id) {
                        self.push_undo(UndoEntry::ItemDeleted {
                            item: Box::new(item),
                        });
                    }
                    aglet.delete_item(item_id, "user:tui")?;
                    self.refresh(aglet.store())?;
                    self.status = "Item deleted".to_string();
                }
                self.done_blocks_confirm = None;
                self.batch_delete_item_ids = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.done_blocks_confirm = None;
                self.batch_delete_item_ids = None;
                self.mode = Mode::Normal;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}
