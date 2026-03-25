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

    fn cycle_global_settings_literal_mode(
        &mut self,
        agenda: &Agenda<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current_index =
            modes::classification::literal_mode_index(self.classification_ui.config.literal_mode);
        let next_index = next_index(current_index, 3, if forward { 1 } else { -1 });
        let mode = modes::classification::literal_mode_from_index(next_index);

        let mut config = self.classification_ui.config.clone();
        config.literal_mode = mode;
        config.sync_enabled_flag();
        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.status = format!(
            "Literal classification: {}",
            modes::classification::literal_mode_label(mode)
        );
        Ok(())
    }

    fn cycle_global_settings_semantic_mode(
        &mut self,
        agenda: &Agenda<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current_index =
            modes::classification::semantic_mode_index(self.classification_ui.config.semantic_mode);
        let next_index = next_index(current_index, 2, if forward { 1 } else { -1 });
        let mode = modes::classification::semantic_mode_from_index(next_index);

        let mut config = self.classification_ui.config.clone();
        config.semantic_mode = mode;
        config.sync_enabled_flag();
        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.status = format!(
            "Semantic classification: {}",
            modes::classification::semantic_mode_label(mode)
        );
        Ok(())
    }

    fn toggle_global_settings_ollama_enabled(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let mut config = self.classification_ui.config.clone();
        config.ollama.enabled = !config.ollama.enabled;
        config.set_provider_enabled(
            agenda_core::classification::PROVIDER_ID_OLLAMA_OPENAI_COMPAT,
            config.ollama.enabled,
        );
        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.status = format!(
            "Ollama {}",
            if self.classification_ui.config.ollama.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        Ok(())
    }

    fn open_global_settings_ollama_text_input(&mut self, context: NameInputContext) {
        let (current_value, label) = match context {
            NameInputContext::OllamaBaseUrl => (
                self.classification_ui.config.ollama.base_url.clone(),
                "Ollama base URL",
            ),
            NameInputContext::OllamaModel => (
                self.classification_ui.config.ollama.model.clone(),
                "Ollama model",
            ),
            NameInputContext::OllamaTimeout => (
                self.classification_ui.config.ollama.timeout_secs.to_string(),
                "Ollama timeout (seconds)",
            ),
            _ => return,
        };
        self.input_panel = Some(input_panel::InputPanel::new_name_input(
            &current_value,
            label,
        ));
        self.name_input_context = Some(context);
        self.mode = Mode::InputPanel;
        self.status = format!("{label}: edit text and press Enter to save");
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
                    GlobalSettingsRow::LiteralClassificationMode => {
                        self.cycle_global_settings_literal_mode(agenda, true)?;
                    }
                    GlobalSettingsRow::SemanticClassificationMode => {
                        self.cycle_global_settings_semantic_mode(agenda, true)?;
                    }
                    GlobalSettingsRow::OllamaEnabled => {
                        self.toggle_global_settings_ollama_enabled(agenda)?;
                    }
                    GlobalSettingsRow::OllamaBaseUrl
                    | GlobalSettingsRow::OllamaModel
                    | GlobalSettingsRow::OllamaTimeout
                    | GlobalSettingsRow::WorkflowReady
                    | GlobalSettingsRow::WorkflowClaim => {}
                }
            }
            KeyCode::Left | KeyCode::Char('h') => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(agenda, false)?;
                }
                GlobalSettingsRow::LiteralClassificationMode => {
                    self.cycle_global_settings_literal_mode(agenda, false)?;
                }
                GlobalSettingsRow::SemanticClassificationMode => {
                    self.cycle_global_settings_semantic_mode(agenda, false)?;
                }
                GlobalSettingsRow::OllamaEnabled => {
                    self.toggle_global_settings_ollama_enabled(agenda)?;
                }
                GlobalSettingsRow::OllamaBaseUrl
                | GlobalSettingsRow::OllamaModel
                | GlobalSettingsRow::OllamaTimeout
                | GlobalSettingsRow::WorkflowReady
                | GlobalSettingsRow::WorkflowClaim => {}
            },
            KeyCode::Enter => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(agenda, true)?;
                }
                GlobalSettingsRow::LiteralClassificationMode => {
                    self.cycle_global_settings_literal_mode(agenda, true)?;
                }
                GlobalSettingsRow::SemanticClassificationMode => {
                    self.cycle_global_settings_semantic_mode(agenda, true)?;
                }
                GlobalSettingsRow::OllamaEnabled => {
                    self.toggle_global_settings_ollama_enabled(agenda)?;
                }
                GlobalSettingsRow::OllamaBaseUrl => {
                    self.open_global_settings_ollama_text_input(NameInputContext::OllamaBaseUrl);
                }
                GlobalSettingsRow::OllamaModel => {
                    self.open_global_settings_ollama_text_input(NameInputContext::OllamaModel);
                }
                GlobalSettingsRow::OllamaTimeout => {
                    self.open_global_settings_ollama_text_input(NameInputContext::OllamaTimeout);
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
