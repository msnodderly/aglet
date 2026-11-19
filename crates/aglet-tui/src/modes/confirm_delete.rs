//! Confirmation dialogs over the board: deleting items (`x`) and marking done
//! an item that other items depend on. Each dialog's state lives in its mode:
//! `Mode::ConfirmDelete(DeleteTarget)` and `Mode::DoneBlocksConfirm(..)`.

use aglet_core::aglet::Aglet;
use aglet_core::model::ItemId;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::render::MUTED_TEXT_COLOR;
use crate::state::board::{DoneBlocksConfirmScope, DoneBlocksConfirmState};
use crate::ui_support::{centered_fixed_rect, truncate_board_cell};
use crate::undo::UndoEntry;
use crate::{App, Mode, TuiResult};

/// What the delete confirmation deletes, fixed when the dialog opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeleteTarget {
    /// The highlighted item.
    Item(ItemId),
    /// The selected items, in view order.
    Items(Vec<ItemId>),
}

impl App {
    pub(crate) fn handle_confirm_delete_key(
        &mut self,
        target: DeleteTarget,
        key: KeyEvent,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        match (key.code, target) {
            (KeyCode::Char('y'), DeleteTarget::Item(item_id)) => {
                // Capture item state for undo before deletion.
                if let Ok(item) = aglet.store().get_item(item_id) {
                    self.push_undo(UndoEntry::ItemDeleted {
                        item: Box::new(item),
                    });
                }
                self.mode = Mode::Normal;
                aglet.delete_item(item_id, "user:tui")?;
                self.refresh(aglet.store())?;
                self.status = "Item deleted".to_string();
            }
            (KeyCode::Char('y'), DeleteTarget::Items(item_ids)) => {
                let captured_items: Vec<_> = item_ids
                    .iter()
                    .filter_map(|item_id| aglet.store().get_item(*item_id).ok())
                    .collect();
                let mut deleted = 0usize;
                let mut failed = 0usize;
                let mut first_error = None;
                for item_id in &item_ids {
                    match aglet.delete_item(*item_id, "user:tui") {
                        Ok(()) => deleted += 1,
                        Err(err) => {
                            failed += 1;
                            first_error.get_or_insert_with(|| err.to_string());
                        }
                    }
                }
                // Reverse order so undoing pops them back in original order.
                for item in captured_items.into_iter().rev() {
                    self.push_undo(UndoEntry::ItemDeleted {
                        item: Box::new(item),
                    });
                }
                self.clear_selected_items();
                self.refresh(aglet.store())?;
                self.mode = Mode::Normal;
                self.status = if failed == 0 {
                    format!("Deleted {deleted} selected items")
                } else {
                    let mut summary = format!("Deleted {deleted} selected items (failed={failed})");
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    summary
                };
            }
            (KeyCode::Esc, target) => {
                self.mode = Mode::Normal;
                self.status = match target {
                    DeleteTarget::Item(_) => "Delete canceled",
                    DeleteTarget::Items(_) => "Batch delete canceled",
                }
                .to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    /// `y` removes the blocking links and marks done, `n` marks done and
    /// keeps the links, Esc returns to where the done toggle came from.
    pub(crate) fn handle_done_blocks_confirm_key(
        &mut self,
        confirm: DoneBlocksConfirmState,
        key: KeyEvent,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        let remove_links = match key.code {
            KeyCode::Char('y') => true,
            KeyCode::Char('n') => false,
            KeyCode::Esc => {
                self.mode = Self::done_toggle_return_mode(confirm.origin);
                self.status = "Done update canceled".to_string();
                return Ok(false);
            }
            _ => return Ok(false),
        };
        match confirm.scope {
            DoneBlocksConfirmScope::Single {
                item_id,
                blocked_item_ids,
            } => {
                let links: &[ItemId] = if remove_links { &blocked_item_ids } else { &[] };
                self.apply_done_toggle_action(aglet, item_id, false, confirm.origin, links)?;
            }
            DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                self.apply_batch_done_action(aglet, &item_ids, remove_links, confirm.origin)?;
            }
        }
        Ok(false)
    }

    /// The popup for either dialog; draws nothing in other modes.
    pub(crate) fn render_confirm_popup(&self, frame: &mut Frame<'_>, area: Rect) {
        let muted = Style::default().fg(MUTED_TEXT_COLOR);
        let mut lines: Vec<Line<'_>> = Vec::new();
        let (popup_title, border_color, keys_hint) = match &self.mode {
            Mode::DoneBlocksConfirm(confirm) => {
                lines.push(Line::from(done_blocks_message(&confirm.scope)));
                (
                    "Mark Done",
                    Color::Yellow,
                    "y:remove links + done   n:done only   Esc:cancel",
                )
            }
            Mode::ConfirmDelete(target) => {
                const TITLE_WIDTH: usize = 52;
                const SHOWN_TITLES: usize = 3;
                match target {
                    DeleteTarget::Items(item_ids) => {
                        let count = item_ids.len();
                        let suffix = if count == 1 { "" } else { "s" };
                        lines.push(Line::from(format!("Delete {count} item{suffix}?")));
                        for id in item_ids.iter().take(SHOWN_TITLES) {
                            let title = self
                                .item_title_for_id(*id)
                                .unwrap_or_else(|| "(unknown item)".to_string());
                            lines.push(Line::from(format!(
                                "  \u{2022} {}",
                                truncate_board_cell(&title, TITLE_WIDTH)
                            )));
                        }
                        if count > SHOWN_TITLES {
                            lines.push(Line::from(Span::styled(
                                format!("  \u{2026} and {} more", count - SHOWN_TITLES),
                                muted,
                            )));
                        }
                    }
                    DeleteTarget::Item(item_id) => {
                        let title = self
                            .item_title_for_id(*item_id)
                            .unwrap_or_else(|| "item".to_string());
                        lines.push(Line::from(format!(
                            "Delete \"{}\"?",
                            truncate_board_cell(&title, TITLE_WIDTH)
                        )));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Logged \u{2014} restorable via 'aglet deleted' / 'aglet restore'",
                    muted,
                )));
                ("Confirm Delete", Color::Red, "y:confirm   Esc:cancel")
            }
            _ => return,
        };

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(keys_hint, muted)));

        let width = area.width.saturating_sub(4).clamp(20, 66);
        let height = (lines.len() as u16).saturating_add(2);
        let popup = centered_fixed_rect(area, width, height);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(popup_title)
            .title_style(Style::default().fg(Color::White))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(Paragraph::new(lines).block(block), popup);
    }

    /// Looks up an item's title in the currently loaded slots.
    fn item_title_for_id(&self, item_id: ItemId) -> Option<String> {
        self.slots
            .iter()
            .flat_map(|slot| slot.items.iter())
            .find(|item| item.id == item_id)
            .map(|item| item.text.clone())
    }
}

/// The question the done-blocker dialog asks; shared by popup and footer.
pub(crate) fn done_blocks_message(scope: &DoneBlocksConfirmScope) -> String {
    match scope {
        DoneBlocksConfirmScope::Single {
            blocked_item_ids, ..
        } => {
            let blocked_count = blocked_item_ids.len();
            let suffix = if blocked_count == 1 { "" } else { "s" };
            format!(
                "This item blocks {blocked_count} other item{suffix}. Remove that link and mark done?"
            )
        }
        DoneBlocksConfirmScope::Batch {
            blocking_item_count,
            blocked_link_count,
            ..
        } => {
            let item_suffix = if *blocking_item_count == 1 { "" } else { "s" };
            let blocked_suffix = if *blocked_link_count == 1 { "" } else { "s" };
            format!(
                "{blocking_item_count} selected item{item_suffix} blocks {blocked_link_count} other item{blocked_suffix}. Remove those links and mark done?"
            )
        }
    }
}
