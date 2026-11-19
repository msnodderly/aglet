//! The view palette (`v`). Its only state is the highlighted row, carried in
//! `Mode::ViewPicker { index }` and `Mode::ViewDeleteConfirm { index }`.
//! Screens opened from here (rename, clone, the view editor) carry the row
//! back so the palette reopens where it was.

use aglet_core::aglet::Aglet;
use aglet_core::model::View;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use ratatui::Frame;

use crate::input_panel::InputPanel;
use crate::state::board::NameInputContext;
use crate::ui_support::{next_index_clamped, selected_row_style};
use crate::{App, Mode, TuiResult};

fn is_immutable_view(view: &View) -> bool {
    aglet_core::store::is_system_view_name(&view.name)
}

/// `index` clamped to the view list, or `None` when there are no views.
fn clamped(index: usize, views: &[View]) -> Option<usize> {
    (!views.is_empty()).then(|| index.min(views.len() - 1))
}

impl App {
    /// The view at the highlighted row, unless it is a system view, in which
    /// case the status says so.
    fn editable_view_at(&mut self, index: usize, missing: &str) -> Option<View> {
        let Some(view) = clamped(index, &self.views).and_then(|i| self.views.get(i).cloned())
        else {
            self.status = missing.to_string();
            return None;
        };
        if is_immutable_view(&view) {
            self.status = format!("{} view is immutable", view.name);
            return None;
        }
        Some(view)
    }

    pub(crate) fn handle_view_picker_key(
        &mut self,
        index: usize,
        key: KeyEvent,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "View switch canceled".to_string();
            }
            KeyCode::Enter => {
                if let Some(index) = clamped(index, &self.views) {
                    self.set_active_view_index(index);
                    self.slot_index = 0;
                    self.item_index = 0;
                    self.slot_sort_keys.clear();
                    self.refresh(aglet.store())?;
                    self.reset_section_filters();
                    let view_name = self
                        .current_view()
                        .map(|view| view.name.clone())
                        .unwrap_or_else(|| "(none)".to_string());
                    self.status =
                        format!("Switched to view: {view_name} (press v then e to edit view)");
                } else {
                    self.status = "No views available".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let mut view = View::new("Untitled View".to_string());
                view.show_unmatched = false;
                if view.sections.is_empty() {
                    view.sections.push(Self::view_edit_default_section(
                        Self::DEFAULT_VIEW_EDIT_SECTION_TITLE,
                    ));
                }
                self.open_view_edit_new_view_focus_name(view, index);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.status =
                    "Use n:new, then set View type to Datebook in View Settings".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.editable_view_at(index, "No selected view to rename") {
                    self.input_panel = Some(InputPanel::new_name_input(&view.name, "Rename view"));
                    self.name_input_context = Some(NameInputContext::ViewRename {
                        view_id: view.id,
                        picker_index: index,
                    });
                    self.mode = Mode::InputPanel;
                    self.status = format!(
                        "Rename view {}: edit name, Save to confirm, Esc to cancel",
                        view.name
                    );
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(view) = self.editable_view_at(index, "No selected view to edit") {
                    self.open_view_edit(view, index);
                }
            }
            KeyCode::Char('V') => {
                if let Some(view) = self.editable_view_at(index, "No views available") {
                    self.open_view_edit(view, index);
                }
            }
            KeyCode::Char('c') => {
                if let Some(view) = self.editable_view_at(index, "No selected view to clone") {
                    self.input_panel = Some(InputPanel::new_name_input(
                        "",
                        &format!("Clone view '{}'", view.name),
                    ));
                    self.name_input_context = Some(NameInputContext::ViewClone {
                        view_id: view.id,
                        picker_index: index,
                    });
                    self.mode = Mode::InputPanel;
                    self.status = format!(
                        "Clone view '{}': type new name, Enter to confirm, Esc to cancel",
                        view.name
                    );
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.editable_view_at(index, "No selected view to delete") {
                    self.mode = Mode::ViewDeleteConfirm { index };
                    self.status = format!("Delete view '{}' ? y/n", view.name);
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !self.views.is_empty() => {
                self.mode = Mode::ViewPicker {
                    index: next_index_clamped(index, self.views.len(), 1),
                };
            }
            KeyCode::Up | KeyCode::Char('k') if !self.views.is_empty() => {
                self.mode = Mode::ViewPicker {
                    index: next_index_clamped(index, self.views.len(), -1),
                };
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_delete_key(
        &mut self,
        index: usize,
        key: KeyEvent,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        match key.code {
            KeyCode::Char('y') => {
                self.mode = Mode::ViewPicker { index };
                let Some(deleted_index) = clamped(index, &self.views) else {
                    self.status = "Delete failed: no selected view".to_string();
                    return Ok(false);
                };
                let view = self.views[deleted_index].clone();
                if is_immutable_view(&view) {
                    self.status = format!("Delete failed: {} view is immutable", view.name);
                    return Ok(false);
                }
                match aglet.store().delete_view(view.id) {
                    Ok(()) => {
                        match self.view_index.cmp(&deleted_index) {
                            std::cmp::Ordering::Greater => self.view_index -= 1,
                            std::cmp::Ordering::Equal => {
                                self.view_index = deleted_index.saturating_sub(1);
                            }
                            std::cmp::Ordering::Less => {}
                        }
                        self.refresh(aglet.store())?;
                        self.mode = Mode::ViewPicker {
                            index: index.min(self.views.len().saturating_sub(1)),
                        };
                        self.status = format!("Deleted view: {}", view.name);
                    }
                    Err(err) => self.status = format!("Delete failed: {err}"),
                }
            }
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker { index };
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}

/// Draw the palette with row `index` highlighted.
pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, views: &[View], index: usize) {
    frame.render_widget(Clear, area);
    let items: Vec<ListItem<'_>> = if views.is_empty() {
        vec![ListItem::new(Line::from("(no views configured)"))]
    } else {
        views
            .iter()
            .map(|view| ListItem::new(Line::from(view.name.clone())))
            .collect()
    };
    let mut state = App::list_state_for(area, clamped(index, views));
    let item_count = items.len();
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("> ")
            .highlight_style(selected_row_style())
            .block(
                Block::default()
                    .title("View Palette")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
        area,
        &mut state,
    );
    App::render_vertical_scrollbar(frame, area, item_count, state.offset());
}
