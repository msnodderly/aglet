use crate::*;

impl App {
    pub(crate) fn global_settings_selected_row(&self) -> usize {
        self.global_settings
            .as_ref()
            .map(|state| state.selected_row)
            .unwrap_or(0)
    }

    fn set_global_settings_selected_row(&mut self, selected_row: usize) {
        let selected_row = selected_row.min(GlobalSettingsRow::count().saturating_sub(1));
        if let Some(state) = self.global_settings.as_mut() {
            state.selected_row = selected_row;
        } else {
            self.global_settings = Some(GlobalSettingsState { selected_row });
        }
    }

    pub(crate) fn global_settings_selected_kind(&self) -> GlobalSettingsRow {
        GlobalSettingsRow::from_index(self.global_settings_selected_row())
    }

    fn cycle_global_settings_auto_refresh(
        &mut self,
        agenda: &Agenda<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let next = if forward {
            self.auto_refresh_interval.next()
        } else {
            self.auto_refresh_interval.prev()
        };
        self.set_auto_refresh_interval(next);
        self.persist_auto_refresh_interval(agenda.store())?;
        self.status = format!("Auto-refresh interval: {}", self.auto_refresh_mode_label());
        Ok(())
    }

    fn cycle_global_settings_classification_mode(
        &mut self,
        agenda: &Agenda<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current_index = modes::classification::continuous_mode_index(
            self.classification_ui.config.continuous_mode,
        );
        let next_index = next_index(current_index, 3, if forward { 1 } else { -1 });
        let mode = modes::classification::continuous_mode_from_index(next_index);

        let mut config = self.classification_ui.config.clone();
        config.continuous_mode = mode;
        config.enabled = config.continuous_mode != ContinuousMode::Off;
        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.status = format!(
            "Classification mode: {}",
            modes::classification::continuous_mode_label(mode)
        );
        Ok(())
    }

    fn open_global_settings_workflow_picker(&mut self) {
        let role_index = match self.global_settings_selected_kind() {
            GlobalSettingsRow::WorkflowReady => 0,
            GlobalSettingsRow::WorkflowClaim => 1,
            _ => return,
        };
        self.open_workflow_role_picker_with_origin(
            role_index,
            WorkflowRolePickerOrigin::GlobalSettings,
        );
    }

    pub(crate) fn handle_global_settings_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self
            .workflow_role_picker
            .as_ref()
            .is_some_and(|picker| picker.origin == WorkflowRolePickerOrigin::GlobalSettings)
        {
            self.handle_workflow_role_picker_key(code, agenda)?;
            return Ok(false);
        }

        match code {
            KeyCode::Esc => {
                self.workflow_role_picker = None;
                self.global_settings = None;
                self.mode = Mode::Normal;
                self.status = "Closed Global Settings".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = next_index_clamped(
                    self.global_settings_selected_row(),
                    GlobalSettingsRow::count(),
                    1,
                );
                self.set_global_settings_selected_row(next);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let next = next_index_clamped(
                    self.global_settings_selected_row(),
                    GlobalSettingsRow::count(),
                    -1,
                );
                self.set_global_settings_selected_row(next);
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                match self.global_settings_selected_kind() {
                    GlobalSettingsRow::AutoRefresh => {
                        self.cycle_global_settings_auto_refresh(agenda, true)?;
                    }
                    GlobalSettingsRow::ClassificationMode => {
                        self.cycle_global_settings_classification_mode(agenda, true)?;
                    }
                    GlobalSettingsRow::WorkflowReady | GlobalSettingsRow::WorkflowClaim => {}
                }
            }
            KeyCode::Left | KeyCode::Char('h') => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(agenda, false)?;
                }
                GlobalSettingsRow::ClassificationMode => {
                    self.cycle_global_settings_classification_mode(agenda, false)?;
                }
                GlobalSettingsRow::WorkflowReady | GlobalSettingsRow::WorkflowClaim => {}
            },
            KeyCode::Enter => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(agenda, true)?;
                }
                GlobalSettingsRow::ClassificationMode => {
                    self.cycle_global_settings_classification_mode(agenda, true)?;
                }
                GlobalSettingsRow::WorkflowReady | GlobalSettingsRow::WorkflowClaim => {
                    self.open_global_settings_workflow_picker();
                }
            },
            _ => {}
        }

        Ok(false)
    }
}
