use aglet_core::classification::SemanticProviderKind;

use crate::*;

impl App {
    pub(crate) fn global_settings_visible_rows(&self) -> Vec<GlobalSettingsRow> {
        GlobalSettingsRow::visible_rows(self.classification.ui.config.semantic_provider)
    }

    pub(crate) fn global_settings_selected_row(&self) -> usize {
        self.settings
            .global_settings
            .as_ref()
            .map(|state| state.selected_row)
            .unwrap_or(0)
    }

    fn set_global_settings_selected_row(&mut self, selected_row: usize) {
        let rows = self.global_settings_visible_rows();
        let selected_row = selected_row.min(rows.len().saturating_sub(1));
        if let Some(state) = self.settings.global_settings.as_mut() {
            state.selected_row = selected_row;
        } else {
            self.settings.global_settings = Some(GlobalSettingsState { selected_row });
        }
    }

    pub(crate) fn global_settings_selected_kind(&self) -> GlobalSettingsRow {
        let rows = self.global_settings_visible_rows();
        let idx = self.global_settings_selected_row();
        rows.get(idx)
            .copied()
            .unwrap_or(GlobalSettingsRow::AutoRefresh)
    }

    fn cycle_global_settings_auto_refresh(
        &mut self,
        aglet: &Aglet<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let next = if forward {
            self.auto_refresh_interval.next()
        } else {
            self.auto_refresh_interval.prev()
        };
        self.set_auto_refresh_interval(next);
        self.persist_auto_refresh_interval(aglet.store())?;
        self.status = format!("Auto-refresh interval: {}", self.auto_refresh_mode_label());
        Ok(())
    }

    fn cycle_global_settings_section_borders(
        &mut self,
        aglet: &Aglet<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let next = if forward {
            self.section_border_mode.next()
        } else {
            self.section_border_mode.prev()
        };
        self.set_section_border_mode(next);
        self.persist_section_border_mode(aglet.store())?;
        self.status = format!("Section borders: {}", self.section_border_mode_label());
        Ok(())
    }

    fn toggle_global_settings_note_glyphs(&mut self, aglet: &Aglet<'_>) -> TuiResult<()> {
        self.show_note_glyphs = !self.show_note_glyphs;
        self.persist_show_note_glyphs(aglet.store())?;
        self.status = format!("Note glyphs: {}", self.show_note_glyphs_label());
        Ok(())
    }

    fn cycle_global_settings_literal_mode(
        &mut self,
        aglet: &Aglet<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current_index =
            modes::classification::literal_mode_index(self.classification.ui.config.literal_mode);
        let next_index = next_index(current_index, 3, if forward { 1 } else { -1 });
        let mode = modes::classification::literal_mode_from_index(next_index);

        let mut config = self.classification.ui.config.clone();
        config.literal_mode = mode;
        config.sync_enabled_flag();
        aglet.store().set_classification_config(&config)?;
        self.refresh(aglet.store())?;
        self.status = format!(
            "Literal classification: {}",
            modes::classification::literal_mode_label(mode)
        );
        Ok(())
    }

    fn cycle_global_settings_semantic_mode(
        &mut self,
        aglet: &Aglet<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current_index =
            modes::classification::semantic_mode_index(self.classification.ui.config.semantic_mode);
        let next_index = next_index(current_index, 2, if forward { 1 } else { -1 });
        let mode = modes::classification::semantic_mode_from_index(next_index);

        let mut config = self.classification.ui.config.clone();
        config.semantic_mode = mode;
        config.sync_enabled_flag();
        aglet.store().set_classification_config(&config)?;
        self.refresh(aglet.store())?;
        self.status = format!(
            "Semantic classification: {}",
            modes::classification::semantic_mode_label(mode)
        );
        Ok(())
    }

    fn cycle_global_settings_semantic_provider(
        &mut self,
        aglet: &Aglet<'_>,
        forward: bool,
    ) -> TuiResult<()> {
        let current = self.classification.ui.config.semantic_provider;
        let next = if forward {
            match current {
                SemanticProviderKind::Ollama => SemanticProviderKind::OpenRouter,
                SemanticProviderKind::OpenRouter => SemanticProviderKind::OpenAi,
                SemanticProviderKind::OpenAi => SemanticProviderKind::Ollama,
            }
        } else {
            match current {
                SemanticProviderKind::Ollama => SemanticProviderKind::OpenAi,
                SemanticProviderKind::OpenRouter => SemanticProviderKind::Ollama,
                SemanticProviderKind::OpenAi => SemanticProviderKind::OpenRouter,
            }
        };
        let mut config = self.classification.ui.config.clone();
        config.semantic_provider = next;
        aglet.store().set_classification_config(&config)?;
        self.refresh(aglet.store())?;
        // Re-clamp cursor: the row count changes when the provider switches.
        let current_row = self.global_settings_selected_row();
        self.set_global_settings_selected_row(current_row);
        self.status = format!("Semantic provider: {}", semantic_provider_label(next));
        Ok(())
    }

    pub(crate) fn open_global_settings_text_input(&mut self, context: NameInputContext) {
        let (current_value, label) = match context {
            NameInputContext::OllamaBaseUrl => (
                self.classification.ui.config.ollama.base_url.clone(),
                "Ollama base URL",
            ),
            NameInputContext::OllamaModel => (
                self.classification.ui.config.ollama.model.clone(),
                "Ollama model",
            ),
            NameInputContext::OllamaTimeout => (
                self.classification
                    .ui
                    .config
                    .ollama
                    .timeout_secs
                    .to_string(),
                "Ollama timeout (seconds)",
            ),
            NameInputContext::OpenRouterModel => (
                self.classification.ui.config.openrouter.model.clone(),
                "OpenRouter model",
            ),
            NameInputContext::OpenRouterTimeout => (
                self.classification
                    .ui
                    .config
                    .openrouter
                    .timeout_secs
                    .to_string(),
                "OpenRouter timeout (seconds)",
            ),
            NameInputContext::OpenAiModel => (
                self.classification.ui.config.openai.model.clone(),
                "OpenAI model",
            ),
            NameInputContext::OpenAiTimeout => (
                self.classification
                    .ui
                    .config
                    .openai
                    .timeout_secs
                    .to_string(),
                "OpenAI timeout (seconds)",
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
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        if self.settings.ollama_model_picker.is_some() {
            self.handle_ollama_model_picker_key(code, aglet)?;
            return Ok(false);
        }
        if self
            .settings
            .workflow_role_picker
            .as_ref()
            .is_some_and(|picker| picker.origin == WorkflowRolePickerOrigin::GlobalSettings)
        {
            self.handle_workflow_role_picker_key(code, aglet)?;
            return Ok(false);
        }

        match code {
            KeyCode::Esc => {
                self.settings.workflow_role_picker = None;
                self.settings.global_settings = None;
                self.mode = Mode::Normal;
                self.status = "Closed Global Settings".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let row_count = self.global_settings_visible_rows().len();
                let next = next_index_clamped(self.global_settings_selected_row(), row_count, 1);
                self.set_global_settings_selected_row(next);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let row_count = self.global_settings_visible_rows().len();
                let next = next_index_clamped(self.global_settings_selected_row(), row_count, -1);
                self.set_global_settings_selected_row(next);
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                match self.global_settings_selected_kind() {
                    GlobalSettingsRow::AutoRefresh => {
                        self.cycle_global_settings_auto_refresh(aglet, true)?;
                    }
                    GlobalSettingsRow::SectionBorders => {
                        self.cycle_global_settings_section_borders(aglet, true)?;
                    }
                    GlobalSettingsRow::NoteGlyphs => {
                        self.toggle_global_settings_note_glyphs(aglet)?;
                    }
                    GlobalSettingsRow::LiteralClassificationMode => {
                        self.cycle_global_settings_literal_mode(aglet, true)?;
                    }
                    GlobalSettingsRow::SemanticClassificationMode => {
                        self.cycle_global_settings_semantic_mode(aglet, true)?;
                    }
                    GlobalSettingsRow::SemanticProvider => {
                        self.cycle_global_settings_semantic_provider(aglet, true)?;
                    }
                    GlobalSettingsRow::OllamaBaseUrl
                    | GlobalSettingsRow::OllamaModel
                    | GlobalSettingsRow::OllamaTimeout
                    | GlobalSettingsRow::OpenRouterModel
                    | GlobalSettingsRow::OpenRouterTimeout
                    | GlobalSettingsRow::OpenAiModel
                    | GlobalSettingsRow::OpenAiTimeout
                    | GlobalSettingsRow::WorkflowReady
                    | GlobalSettingsRow::WorkflowClaim => {}
                }
            }
            KeyCode::Left | KeyCode::Char('h') => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(aglet, false)?;
                }
                GlobalSettingsRow::SectionBorders => {
                    self.cycle_global_settings_section_borders(aglet, false)?;
                }
                GlobalSettingsRow::NoteGlyphs => {
                    self.toggle_global_settings_note_glyphs(aglet)?;
                }
                GlobalSettingsRow::LiteralClassificationMode => {
                    self.cycle_global_settings_literal_mode(aglet, false)?;
                }
                GlobalSettingsRow::SemanticClassificationMode => {
                    self.cycle_global_settings_semantic_mode(aglet, false)?;
                }
                GlobalSettingsRow::SemanticProvider => {
                    self.cycle_global_settings_semantic_provider(aglet, false)?;
                }
                GlobalSettingsRow::OllamaBaseUrl
                | GlobalSettingsRow::OllamaModel
                | GlobalSettingsRow::OllamaTimeout
                | GlobalSettingsRow::OpenRouterModel
                | GlobalSettingsRow::OpenRouterTimeout
                | GlobalSettingsRow::OpenAiModel
                | GlobalSettingsRow::OpenAiTimeout
                | GlobalSettingsRow::WorkflowReady
                | GlobalSettingsRow::WorkflowClaim => {}
            },
            KeyCode::Enter => match self.global_settings_selected_kind() {
                GlobalSettingsRow::AutoRefresh => {
                    self.cycle_global_settings_auto_refresh(aglet, true)?;
                }
                GlobalSettingsRow::SectionBorders => {
                    self.cycle_global_settings_section_borders(aglet, true)?;
                }
                GlobalSettingsRow::NoteGlyphs => {
                    self.toggle_global_settings_note_glyphs(aglet)?;
                }
                GlobalSettingsRow::LiteralClassificationMode => {
                    self.cycle_global_settings_literal_mode(aglet, true)?;
                }
                GlobalSettingsRow::SemanticClassificationMode => {
                    self.cycle_global_settings_semantic_mode(aglet, true)?;
                }
                GlobalSettingsRow::SemanticProvider => {
                    self.cycle_global_settings_semantic_provider(aglet, true)?;
                }
                GlobalSettingsRow::OllamaBaseUrl => {
                    self.open_global_settings_text_input(NameInputContext::OllamaBaseUrl);
                }
                GlobalSettingsRow::OllamaModel => {
                    self.open_ollama_model_picker(aglet);
                }
                GlobalSettingsRow::OllamaTimeout => {
                    self.open_global_settings_text_input(NameInputContext::OllamaTimeout);
                }
                GlobalSettingsRow::OpenRouterModel => {
                    self.open_global_settings_text_input(NameInputContext::OpenRouterModel);
                }
                GlobalSettingsRow::OpenRouterTimeout => {
                    self.open_global_settings_text_input(NameInputContext::OpenRouterTimeout);
                }
                GlobalSettingsRow::OpenAiModel => {
                    self.open_global_settings_text_input(NameInputContext::OpenAiModel);
                }
                GlobalSettingsRow::OpenAiTimeout => {
                    self.open_global_settings_text_input(NameInputContext::OpenAiTimeout);
                }
                GlobalSettingsRow::WorkflowReady | GlobalSettingsRow::WorkflowClaim => {
                    self.open_global_settings_workflow_picker();
                }
            },
            _ => {}
        }

        Ok(false)
    }

    fn open_ollama_model_picker(&mut self, _aglet: &Aglet<'_>) {
        match aglet_core::classification::list_ollama_models(&self.classification.ui.config.ollama)
        {
            Ok(models) if !models.is_empty() => {
                let current = &self.classification.ui.config.ollama.model;
                let selected_index = models.iter().position(|m| m == current).unwrap_or(0);
                self.settings.ollama_model_picker = Some(OllamaModelPickerState {
                    models,
                    selected_index,
                });
                self.status = "Pick a model: j/k navigate, Enter select, Esc cancel".to_string();
            }
            Ok(_) => {
                self.status = "No models found. Falling back to text input.".to_string();
                self.open_global_settings_text_input(NameInputContext::OllamaModel);
            }
            Err(err) => {
                self.status = format!("Could not reach Ollama: {err}");
                self.open_global_settings_text_input(NameInputContext::OllamaModel);
            }
        }
    }

    fn handle_ollama_model_picker_key(
        &mut self,
        code: KeyCode,
        aglet: &Aglet<'_>,
    ) -> TuiResult<()> {
        let Some(picker) = self.settings.ollama_model_picker.as_mut() else {
            return Ok(());
        };
        match code {
            KeyCode::Esc => {
                self.settings.ollama_model_picker = None;
                self.status = "Model selection cancelled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j')
                if picker.selected_index + 1 < picker.models.len() =>
            {
                picker.selected_index += 1;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected_index = picker.selected_index.saturating_sub(1);
            }
            KeyCode::Enter => {
                let selected = picker.models[picker.selected_index].clone();
                self.settings.ollama_model_picker = None;
                let mut config = self.classification.ui.config.clone();
                config.ollama.model = selected.clone();
                aglet.store().set_classification_config(&config)?;
                self.refresh(aglet.store())?;
                self.status = format!("Ollama model set to '{selected}'");
            }
            _ => {}
        }
        Ok(())
    }
}

pub(crate) fn semantic_provider_label(kind: SemanticProviderKind) -> &'static str {
    match kind {
        SemanticProviderKind::Ollama => "Ollama",
        SemanticProviderKind::OpenRouter => "OpenRouter",
        SemanticProviderKind::OpenAi => "OpenAI",
    }
}
