use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::time::Duration;

use jiff::civil::{Date, DateTime};
use jiff::Timestamp;
use reqwest::blocking::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::dates::{BasicDateParser, DateParser};
use crate::error::{AgendaError, Result};
use crate::matcher::{Classifier, ImplicitMatchSource};
use crate::model::{
    AssignmentSource, CategoryId, CategoryValueKind, Item, ItemId, RESERVED_CATEGORY_NAMES,
};
use crate::store::Store;

pub const CLASSIFICATION_CONFIG_KEY: &str = "classification.config.v1";
pub const PROVIDER_ID_IMPLICIT_STRING: &str = "implicit_string";
pub const PROVIDER_ID_WHEN_PARSER: &str = "when_parser";
pub const PROVIDER_ID_OLLAMA_OPENAI_COMPAT: &str = "ollama_openai_compat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ClassificationConfig {
    pub enabled: bool,
    pub literal_mode: LiteralClassificationMode,
    pub semantic_mode: SemanticClassificationMode,
    pub run_on_item_save: bool,
    pub run_on_category_change: bool,
    pub enabled_providers: Vec<ProviderConfig>,
    pub ollama: OllamaProviderSettings,
}

impl Default for ClassificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::Off,
            run_on_item_save: true,
            run_on_category_change: true,
            enabled_providers: vec![
                ProviderConfig {
                    provider_id: PROVIDER_ID_IMPLICIT_STRING.to_string(),
                    enabled: true,
                    mode: ProviderMode::InlineIfCheap,
                },
                ProviderConfig {
                    provider_id: PROVIDER_ID_WHEN_PARSER.to_string(),
                    enabled: true,
                    mode: ProviderMode::InlineIfCheap,
                },
                ProviderConfig {
                    provider_id: PROVIDER_ID_OLLAMA_OPENAI_COMPAT.to_string(),
                    enabled: false,
                    mode: ProviderMode::InlineIfCheap,
                },
            ],
            ollama: OllamaProviderSettings::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClassificationConfigWire {
    enabled: Option<bool>,
    literal_mode: Option<LiteralClassificationMode>,
    semantic_mode: Option<SemanticClassificationMode>,
    continuous_mode: Option<LegacyContinuousMode>,
    run_on_item_save: Option<bool>,
    run_on_category_change: Option<bool>,
    enabled_providers: Option<Vec<ProviderConfig>>,
    ollama: Option<OllamaProviderSettings>,
}

impl<'de> Deserialize<'de> for ClassificationConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ClassificationConfigWire::deserialize(deserializer)?;
        let mut config = ClassificationConfig::default();

        if let Some(enabled) = wire.enabled {
            config.enabled = enabled;
        }
        if let Some(mode) = wire.literal_mode {
            config.literal_mode = mode;
        }
        if let Some(mode) = wire.semantic_mode {
            config.semantic_mode = mode;
        }
        if let Some(legacy) = wire.continuous_mode {
            let (literal_mode, semantic_mode) = legacy.into_modes();
            config.literal_mode = literal_mode;
            config.semantic_mode = semantic_mode;
        }
        if let Some(run_on_item_save) = wire.run_on_item_save {
            config.run_on_item_save = run_on_item_save;
        }
        if let Some(run_on_category_change) = wire.run_on_category_change {
            config.run_on_category_change = run_on_category_change;
        }
        if let Some(enabled_providers) = wire.enabled_providers {
            config.enabled_providers = enabled_providers;
        }
        if let Some(ollama) = wire.ollama {
            config.ollama = ollama;
        }

        config.ensure_provider_defaults();
        config.sync_enabled_flag();
        Ok(config)
    }
}

impl ClassificationConfig {
    pub fn should_run_continuously(&self) -> bool {
        self.enabled
            && (self.literal_mode != LiteralClassificationMode::Off
                || self.semantic_mode != SemanticClassificationMode::Off)
    }

    pub fn provider_enabled(&self, provider_id: &str) -> bool {
        self.enabled_providers
            .iter()
            .find(|cfg| cfg.provider_id == provider_id)
            .map(|cfg| cfg.enabled)
            .unwrap_or(false)
    }

    pub fn set_provider_enabled(&mut self, provider_id: &str, enabled: bool) {
        if let Some(provider) = self
            .enabled_providers
            .iter_mut()
            .find(|cfg| cfg.provider_id == provider_id)
        {
            provider.enabled = enabled;
        } else {
            self.enabled_providers.push(ProviderConfig {
                provider_id: provider_id.to_string(),
                enabled,
                mode: ProviderMode::InlineIfCheap,
            });
        }
    }

    pub fn sync_enabled_flag(&mut self) {
        self.enabled = self.literal_mode != LiteralClassificationMode::Off
            || self.semantic_mode != SemanticClassificationMode::Off;
    }

    fn ensure_provider_defaults(&mut self) {
        for provider_id in [
            PROVIDER_ID_IMPLICIT_STRING,
            PROVIDER_ID_WHEN_PARSER,
            PROVIDER_ID_OLLAMA_OPENAI_COMPAT,
        ] {
            if self
                .enabled_providers
                .iter()
                .any(|cfg| cfg.provider_id == provider_id)
            {
                continue;
            }
            let enabled = matches!(
                provider_id,
                PROVIDER_ID_IMPLICIT_STRING | PROVIDER_ID_WHEN_PARSER
            );
            self.enabled_providers.push(ProviderConfig {
                provider_id: provider_id.to_string(),
                enabled,
                mode: ProviderMode::InlineIfCheap,
            });
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LiteralClassificationMode {
    Off,
    AutoApply,
    SuggestReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SemanticClassificationMode {
    Off,
    SuggestReview,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum LegacyContinuousMode {
    Off,
    AutoApply,
    SuggestReview,
}

impl LegacyContinuousMode {
    fn into_modes(self) -> (LiteralClassificationMode, SemanticClassificationMode) {
        match self {
            Self::Off => (
                LiteralClassificationMode::Off,
                SemanticClassificationMode::Off,
            ),
            Self::AutoApply => (
                LiteralClassificationMode::AutoApply,
                SemanticClassificationMode::Off,
            ),
            Self::SuggestReview => (
                LiteralClassificationMode::SuggestReview,
                SemanticClassificationMode::SuggestReview,
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_id: String,
    pub enabled: bool,
    pub mode: ProviderMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderMode {
    InlineIfCheap,
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OllamaProviderSettings {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for OllamaProviderSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "mistral".to_string(),
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryDescriptor {
    pub id: CategoryId,
    pub name: String,
    pub match_category_name: bool,
    pub also_match: Vec<String>,
    pub parent_id: Option<CategoryId>,
    pub value_kind: CategoryValueKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationRequest {
    pub item_id: ItemId,
    pub text: String,
    pub note: Option<String>,
    pub when_date: Option<DateTime>,
    pub manual_category_ids: Vec<CategoryId>,
    pub visible_view_name: Option<String>,
    pub visible_section_title: Option<String>,
    pub numeric_values: Vec<(CategoryId, Decimal)>,
    pub literal_candidate_categories: Vec<CategoryDescriptor>,
    pub semantic_candidate_categories: Vec<CategoryDescriptor>,
}

impl ClassificationRequest {
    pub fn match_text(&self) -> String {
        match self.note.as_deref() {
            Some(note) if !note.trim().is_empty() => format!("{} {}", self.text, note),
            _ => self.text.clone(),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CandidateAssignment {
    Category(CategoryId),
    When(DateTime),
}

impl CandidateAssignment {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Category(_) => "category",
            Self::When(_) => "when",
        }
    }

    pub fn stable_key(&self) -> String {
        match self {
            Self::Category(category_id) => format!("category:{category_id}"),
            Self::When(value) => format!("when:{value}"),
        }
    }

    pub fn category_id(&self) -> Option<CategoryId> {
        match self {
            Self::Category(category_id) => Some(*category_id),
            Self::When(_) => None,
        }
    }

    pub fn when_value(&self) -> Option<DateTime> {
        match self {
            Self::When(value) => Some(*value),
            Self::Category(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationSuggestion {
    pub id: Uuid,
    pub item_id: ItemId,
    pub assignment: CandidateAssignment,
    pub provider_id: String,
    pub model: Option<String>,
    pub confidence: Option<f32>,
    pub rationale: Option<String>,
    pub status: SuggestionStatus,
    pub context_hash: String,
    pub item_revision_hash: String,
    pub created_at: Timestamp,
    pub decided_at: Option<Timestamp>,
}

impl ClassificationSuggestion {
    pub fn from_candidate(
        candidate: &ClassificationCandidate,
        item_revision_hash: String,
        status: SuggestionStatus,
    ) -> Self {
        let identity = format!(
            "{}:{}:{}:{}",
            candidate.item_id,
            item_revision_hash,
            candidate.provider,
            candidate.assignment.stable_key()
        );
        let decided_at = match status {
            SuggestionStatus::Accepted
            | SuggestionStatus::Rejected
            | SuggestionStatus::Superseded => Some(Timestamp::now()),
            SuggestionStatus::Pending => None,
        };
        Self {
            id: Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            item_id: candidate.item_id,
            assignment: candidate.assignment.clone(),
            provider_id: candidate.provider.clone(),
            model: candidate.model.clone(),
            confidence: candidate.confidence,
            rationale: candidate.rationale.clone(),
            status,
            context_hash: candidate.context_hash.clone(),
            item_revision_hash,
            created_at: Timestamp::now(),
            decided_at,
        }
    }
}

pub trait ClassificationProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn classify(&self, request: &ClassificationRequest) -> Result<ProviderClassificationResult>;
    fn is_cheap(&self) -> bool {
        false
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProviderClassificationResult {
    pub candidates: Vec<ClassificationCandidate>,
    pub debug_summary: Option<String>,
}

pub trait OllamaTransport: Send + Sync {
    fn complete(
        &self,
        settings: &OllamaProviderSettings,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<Option<String>>;
}

#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OllamaModelListResponse {
    data: Vec<OllamaModelEntry>,
}

/// Queries the Ollama API for available models.
pub fn list_ollama_models(settings: &OllamaProviderSettings) -> Result<Vec<String>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| AgendaError::StorageError {
            source: Box::new(err),
        })?;
    let url = format!(
        "{}/models",
        settings.base_url.trim_end_matches('/')
    );
    let response = client
        .get(url)
        .send()
        .map_err(|err| AgendaError::StorageError {
            source: Box::new(err),
        })?;
    let response = response
        .error_for_status()
        .map_err(|err| AgendaError::StorageError {
            source: Box::new(err),
        })?;
    let parsed: OllamaModelListResponse =
        response.json().map_err(|err| AgendaError::StorageError {
            source: Box::new(err),
        })?;
    let mut models: Vec<String> = parsed.data.into_iter().map(|entry| entry.id).collect();
    models.sort();
    Ok(models)
}

#[derive(Debug, Default)]
pub struct ReqwestOllamaTransport;

impl OllamaTransport for ReqwestOllamaTransport {
    fn complete(
        &self,
        settings: &OllamaProviderSettings,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<Option<String>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(settings.timeout_secs))
            .build()
            .map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let url = format!(
            "{}/chat/completions",
            settings.base_url.trim_end_matches('/')
        );
        let body = json!({
            "model": settings.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.2,
            "response_format": {"type": "json_object"}
        });
        let response =
            client
                .post(url)
                .json(&body)
                .send()
                .map_err(|err| AgendaError::StorageError {
                    source: Box::new(err),
                })?;
        let response = response
            .error_for_status()
            .map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let parsed: OpenAiChatCompletionResponse =
            response.json().map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        Ok(parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content.trim().to_string())
            .filter(|content| !content.is_empty()))
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMessage {
    content: String,
}

pub struct ImplicitStringProvider<'a> {
    pub classifier: &'a dyn Classifier,
}

impl ClassificationProvider for ImplicitStringProvider<'_> {
    fn id(&self) -> &'static str {
        PROVIDER_ID_IMPLICIT_STRING
    }

    fn is_cheap(&self) -> bool {
        true
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<ProviderClassificationResult> {
        let match_text = request.match_text();
        let mut out = Vec::new();
        for category in &request.literal_candidate_categories {
            let Some(matched) = self.classifier.classify(
                &match_text,
                &category.name,
                category.match_category_name,
                &category.also_match,
            ) else {
                continue;
            };
            let rationale = match matched.source {
                ImplicitMatchSource::CategoryName => {
                    format!("matched category name '{}'", matched.matched_term)
                }
                ImplicitMatchSource::AlsoMatch => {
                    format!("matched also-match term '{}'", matched.matched_term)
                }
            };
            out.push(ClassificationCandidate {
                item_id: request.item_id,
                assignment: CandidateAssignment::Category(category.id),
                provider: self.id().to_string(),
                model: None,
                confidence: Some(1.0),
                rationale: Some(rationale),
                context_hash: "request:v1".to_string(),
            });
        }
        Ok(ProviderClassificationResult {
            candidates: out,
            debug_summary: None,
        })
    }
}

pub struct WhenParserProvider {
    pub parser: BasicDateParser,
    pub reference_date: Date,
}

impl ClassificationProvider for WhenParserProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID_WHEN_PARSER
    }

    fn is_cheap(&self) -> bool {
        true
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<ProviderClassificationResult> {
        let Some(parsed) = self.parser.parse(&request.text, self.reference_date) else {
            return Ok(ProviderClassificationResult::default());
        };
        let matched_text = request
            .text
            .get(parsed.span.0..parsed.span.1)
            .unwrap_or("")
            .to_string();

        Ok(ProviderClassificationResult {
            candidates: vec![ClassificationCandidate {
                item_id: request.item_id,
                assignment: CandidateAssignment::When(parsed.datetime),
                provider: self.id().to_string(),
                model: None,
                confidence: Some(1.0),
                rationale: Some(format!("parsed date expression '{}'", matched_text)),
                context_hash: "request:v1".to_string(),
            }],
            debug_summary: None,
        })
    }
}

pub struct OllamaProvider<'a> {
    pub settings: &'a OllamaProviderSettings,
    pub transport: &'a dyn OllamaTransport,
    pub debug: bool,
}

impl ClassificationProvider for OllamaProvider<'_> {
    fn id(&self) -> &'static str {
        PROVIDER_ID_OLLAMA_OPENAI_COMPAT
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<ProviderClassificationResult> {
        let debug = self.debug;
        let dbg = |msg: &str| {
            if !debug {
                return;
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/aglet-ollama-debug.log")
            {
                let _ = writeln!(f, "[{}] {msg}", jiff::Zoned::now());
            }
        };

        dbg(&format!(
            "classify called: enabled={} model={:?} base_url={:?} semantic_cats={}",
            self.settings.enabled,
            self.settings.model,
            self.settings.base_url,
            request.semantic_candidate_categories.len()
        ));

        if !self.settings.enabled
            || self.settings.model.trim().is_empty()
            || self.settings.base_url.trim().is_empty()
            || request.semantic_candidate_categories.is_empty()
        {
            dbg("early return: precondition failed");
            return Ok(ProviderClassificationResult::default());
        }

        let system_prompt = "You classify a single item into existing categories. Return strict JSON only with shape {\"suggestions\":[{\"category\":\"Exact Category Name\",\"confidence\":0.0,\"rationale\":\"short reason\"}]}. The \"category\" field must contain ONLY the category name — no parenthetical annotations, no hierarchy info. For example return \"High\" not \"High (child of Priority)\". Use only exact category names from the provided list. Any category name not exactly present in the allowed list is invalid and will be discarded. Do not invent, expand, paraphrase, or rewrite category names. If nothing applies, return {\"suggestions\":[]}. Suggest at most 3 categories. Categories are organized in a hierarchy. Categories marked [group] are parents that contain child categories. Prefer specific child categories over their parent group when a child clearly fits.";
        let user_prompt = build_ollama_user_prompt(request);
        dbg(&format!("user_prompt:\n{user_prompt}"));
        let content = match self
            .transport
            .complete(self.settings, system_prompt, &user_prompt)
        {
            Ok(content) => content,
            Err(err) => {
                dbg(&format!("transport error: {err}"));
                return Ok(ProviderClassificationResult {
                    candidates: Vec::new(),
                    debug_summary: Some(format!(
                        "semantic[{}]: transport error",
                        self.settings.model
                    )),
                });
            }
        };
        let Some(content) = content else {
            dbg("empty response from transport");
            return Ok(ProviderClassificationResult {
                candidates: Vec::new(),
                debug_summary: Some(format!("semantic[{}]: empty response", self.settings.model)),
            });
        };

        dbg(&format!("ollama response: {content}"));

        let result = parse_ollama_suggestions(
            request,
            &content,
            &self.settings.model,
            self.id(),
        );
        dbg(&format!(
            "parse result: {} candidates, debug={:?}",
            result.candidates.len(),
            result.debug_summary
        ));
        Ok(result)
    }
}

fn build_ollama_user_prompt(request: &ClassificationRequest) -> String {
    // Build a name lookup for resolving parent IDs to human-readable names.
    let name_map: HashMap<CategoryId, &str> = request
        .semantic_candidate_categories
        .iter()
        .map(|cat| (cat.id, cat.name.as_str()))
        .collect();

    // Identify which categories are parents (have children in the list).
    let parent_ids: HashSet<CategoryId> = request
        .semantic_candidate_categories
        .iter()
        .filter_map(|cat| cat.parent_id)
        .collect();

    let mut lines = Vec::new();
    lines.push(format!("Item text: {}", request.text));
    lines.push(format!(
        "Note: {}",
        request.note.as_deref().unwrap_or("(none)")
    ));
    lines.push(format!(
        "When: {}",
        request
            .when_date
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(none)".to_string())
    ));

    // Show manual categories by name when possible.
    if request.manual_category_ids.is_empty() {
        lines.push("Manual categories: (none)".to_string());
    } else {
        let mut manual_names: Vec<String> = request
            .manual_category_ids
            .iter()
            .map(|id| {
                name_map
                    .get(id)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| id.to_string())
            })
            .collect();
        manual_names.sort();
        lines.push(format!("Manual categories: {}", manual_names.join(", ")));
    }
    if request.numeric_values.is_empty() {
        lines.push("Numeric assignments: (none)".to_string());
    } else {
        let mut numeric_values: Vec<String> = request
            .numeric_values
            .iter()
            .map(|(id, value)| {
                let name = name_map
                    .get(id)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| id.to_string());
                format!("{name}={value}")
            })
            .collect();
        numeric_values.sort();
        lines.push(format!(
            "Numeric assignments: {}",
            numeric_values.join(", ")
        ));
    }
    lines.push(
        "Allowed category names (use exactly as written, or return an empty list):".to_string(),
    );
    for category in &request.semantic_candidate_categories {
        let aliases = if category.also_match.is_empty() {
            String::new()
        } else {
            format!(" | aliases: {}", category.also_match.join(", "))
        };
        let parent = match category.parent_id {
            Some(pid) => {
                let parent_name = name_map.get(&pid).copied().unwrap_or("?");
                format!(" (child of \"{}\")", parent_name)
            }
            None => String::new(),
        };
        let role = if parent_ids.contains(&category.id) {
            " [group]"
        } else {
            ""
        };
        lines.push(format!(
            "- {}{parent}{role}{aliases}",
            category.name,
        ));
    }
    lines.push("Do not invent, rewrite, expand, or paraphrase category names.".to_string());
    lines.join("\n")
}

#[derive(Debug, Deserialize)]
struct OllamaSuggestionEnvelope {
    suggestions: Vec<OllamaSuggestionRow>,
}

#[derive(Debug, Deserialize)]
struct OllamaSuggestionRow {
    category: String,
    confidence: Option<f32>,
    rationale: Option<String>,
}

fn parse_ollama_suggestions(
    request: &ClassificationRequest,
    content: &str,
    model: &str,
    provider_id: &str,
) -> ProviderClassificationResult {
    let Ok(parsed) = serde_json::from_str::<OllamaSuggestionEnvelope>(content) else {
        return ProviderClassificationResult {
            candidates: Vec::new(),
            debug_summary: Some(format!("semantic[{model}]: malformed JSON response")),
        };
    };
    let category_map: HashMap<String, &CategoryDescriptor> = request
        .semantic_candidate_categories
        .iter()
        .map(|category| (category.name.to_ascii_lowercase(), category))
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let raw_count = parsed.suggestions.len();
    let mut dropped_unknown = 0usize;
    let mut dropped_duplicate = 0usize;

    for suggestion in parsed.suggestions.into_iter().take(3) {
        // Strip any parenthetical annotations the model may echo back,
        // e.g. "High (child of Priority)" → "High".
        let raw = suggestion.category.trim();
        let normalized = raw
            .find(" (")
            .map(|pos| &raw[..pos])
            .unwrap_or(raw)
            .trim();
        let key = normalized.to_ascii_lowercase();
        let Some(category) = category_map.get(&key) else {
            dropped_unknown += 1;
            continue;
        };
        if !seen.insert(category.id) {
            dropped_duplicate += 1;
            continue;
        }
        let confidence = suggestion.confidence.map(|value| value.clamp(0.0, 1.0));
        let rationale = suggestion
            .rationale
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        out.push(ClassificationCandidate {
            item_id: request.item_id,
            assignment: CandidateAssignment::Category(category.id),
            provider: provider_id.to_string(),
            model: Some(model.to_string()),
            confidence,
            rationale,
            context_hash: "request:v1".to_string(),
        });
    }

    ProviderClassificationResult {
        candidates: out.clone(),
        debug_summary: Some(format!(
            "semantic[{model}]: raw={raw_count} kept={} dropped_unknown={} dropped_duplicate={}",
            out.len(),
            dropped_unknown,
            dropped_duplicate
        )),
    }
}

pub struct ClassificationService<'a> {
    store: &'a Store,
    providers: Vec<Box<dyn ClassificationProvider + 'a>>,
}

impl<'a> ClassificationService<'a> {
    pub fn new(store: &'a Store, providers: Vec<Box<dyn ClassificationProvider + 'a>>) -> Self {
        Self { store, providers }
    }

    pub fn has_providers(&self) -> bool {
        !self.providers.is_empty()
    }

    pub fn collect_candidates(
        &self,
        item_id: ItemId,
    ) -> Result<(Item, String, Vec<ClassificationCandidate>, Vec<String>)> {
        let item = self.store.get_item(item_id)?;
        let request = self.build_request(&item)?;
        let item_revision_hash = item_revision_hash(&item);
        let mut candidates = Vec::new();
        let mut debug_summaries = Vec::new();

        for provider in &self.providers {
            let result = provider.classify(&request)?;
            candidates.extend(result.candidates);
            if let Some(summary) = result.debug_summary {
                debug_summaries.push(summary);
            }
        }

        candidates.sort_by(|left, right| {
            left.provider.cmp(&right.provider).then_with(|| {
                left.assignment
                    .stable_key()
                    .cmp(&right.assignment.stable_key())
            })
        });
        candidates.dedup_by(|left, right| {
            left.provider == right.provider
                && left.assignment.stable_key() == right.assignment.stable_key()
        });

        Ok((item, item_revision_hash, candidates, debug_summaries))
    }

    fn build_request(&self, item: &Item) -> Result<ClassificationRequest> {
        let categories = self.store.get_hierarchy()?;
        let manual_category_ids = item
            .assignments
            .iter()
            .filter_map(|(category_id, assignment)| {
                (assignment.source == AssignmentSource::Manual).then_some(*category_id)
            })
            .collect();
        let numeric_values = item
            .assignments
            .iter()
            .filter_map(|(category_id, assignment)| {
                assignment.numeric_value.map(|value| (*category_id, value))
            })
            .collect();

        let mut literal_candidate_categories = Vec::new();
        let mut semantic_candidate_categories = Vec::new();
        for category in categories {
            if RESERVED_CATEGORY_NAMES
                .iter()
                .any(|reserved| reserved.eq_ignore_ascii_case(&category.name))
            {
                continue;
            }
            let descriptor = CategoryDescriptor {
                id: category.id,
                name: category.name,
                match_category_name: category.match_category_name,
                also_match: category.also_match,
                parent_id: category.parent,
                value_kind: category.value_kind,
            };
            if descriptor.value_kind != CategoryValueKind::Numeric
                && category.enable_implicit_string
            {
                literal_candidate_categories.push(descriptor.clone());
            }
            if descriptor.value_kind != CategoryValueKind::Numeric
                && category.enable_semantic_classification
            {
                semantic_candidate_categories.push(descriptor);
            }
        }

        Ok(ClassificationRequest {
            item_id: item.id,
            text: item.text.clone(),
            note: item.note.clone(),
            when_date: item.when_date,
            manual_category_ids,
            visible_view_name: None,
            visible_section_title: None,
            numeric_values,
            literal_candidate_categories,
            semantic_candidate_categories,
        })
    }
}

pub fn item_revision_hash(item: &Item) -> String {
    let mut manual_category_ids: Vec<String> = item
        .assignments
        .iter()
        .filter_map(|(category_id, assignment)| {
            (assignment.source == AssignmentSource::Manual).then_some(category_id.to_string())
        })
        .collect();
    manual_category_ids.sort();

    format!(
        "text={:?};note={:?};when={:?};manual={}",
        item.text,
        item.note,
        item.when_date.map(|value| value.to_string()),
        manual_category_ids.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeOllamaTransport {
        response: Option<String>,
        fail: bool,
    }

    impl OllamaTransport for FakeOllamaTransport {
        fn complete(
            &self,
            _settings: &OllamaProviderSettings,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> Result<Option<String>> {
            if self.fail {
                return Err(AgendaError::StorageError {
                    source: Box::new(std::io::Error::other("boom")),
                });
            }
            Ok(self.response.clone())
        }
    }

    #[test]
    fn classification_config_deserializes_legacy_continuous_mode() {
        let json = r#"{"continuous_mode":"SuggestReview"}"#;
        let config: ClassificationConfig = serde_json::from_str(json).expect("deserialize config");
        assert_eq!(
            config.literal_mode,
            LiteralClassificationMode::SuggestReview
        );
        assert_eq!(
            config.semantic_mode,
            SemanticClassificationMode::SuggestReview
        );
    }

    #[test]
    fn classification_config_roundtrips_dual_modes_and_ollama_settings() {
        let mut config = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        config.ollama.enabled = true;
        config.ollama.base_url = "http://localhost:11434/v1".to_string();
        config.ollama.model = "mistral".to_string();
        config.ollama.timeout_secs = 30;
        config.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);

        let json = serde_json::to_string(&config).expect("serialize config");
        let decoded: ClassificationConfig =
            serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(decoded.literal_mode, LiteralClassificationMode::Off);
        assert_eq!(
            decoded.semantic_mode,
            SemanticClassificationMode::SuggestReview
        );
        assert!(decoded.ollama.enabled);
        assert_eq!(decoded.ollama.base_url, "http://localhost:11434/v1");
        assert_eq!(decoded.ollama.model, "mistral");
        assert_eq!(decoded.ollama.timeout_secs, 30);
        assert!(decoded.provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT));
    }

    #[test]
    fn parse_ollama_suggestions_ignores_unknown_and_duplicate_categories() {
        let category_a = CategoryDescriptor {
            id: CategoryId::new_v4(),
            name: "Travel".to_string(),
            match_category_name: true,
            also_match: Vec::new(),
            parent_id: None,
            value_kind: CategoryValueKind::Tag,
        };
        let category_b = CategoryDescriptor {
            id: CategoryId::new_v4(),
            name: "Work".to_string(),
            match_category_name: true,
            also_match: Vec::new(),
            parent_id: None,
            value_kind: CategoryValueKind::Tag,
        };
        let request = ClassificationRequest {
            item_id: ItemId::new_v4(),
            text: "Book flights".to_string(),
            note: None,
            when_date: None,
            manual_category_ids: Vec::new(),
            visible_view_name: None,
            visible_section_title: None,
            numeric_values: Vec::new(),
            literal_candidate_categories: Vec::new(),
            semantic_candidate_categories: vec![category_a.clone(), category_b.clone()],
        };

        let parsed = parse_ollama_suggestions(
            &request,
            r#"{"suggestions":[{"category":"Travel","confidence":0.8,"rationale":"trip"},{"category":"Unknown","confidence":0.2},{"category":"travel","confidence":0.9}]}"#,
            "mistral",
            PROVIDER_ID_OLLAMA_OPENAI_COMPAT,
        );
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(
            parsed.candidates[0].assignment,
            CandidateAssignment::Category(category_a.id)
        );
        assert_eq!(
            parsed.debug_summary.as_deref(),
            Some("semantic[mistral]: raw=3 kept=1 dropped_unknown=1 dropped_duplicate=1")
        );
    }

    #[test]
    fn ollama_provider_returns_empty_for_malformed_json() {
        let request = ClassificationRequest {
            item_id: ItemId::new_v4(),
            text: "Book flights".to_string(),
            note: None,
            when_date: None,
            manual_category_ids: Vec::new(),
            visible_view_name: None,
            visible_section_title: None,
            numeric_values: Vec::new(),
            literal_candidate_categories: Vec::new(),
            semantic_candidate_categories: vec![CategoryDescriptor {
                id: CategoryId::new_v4(),
                name: "Travel".to_string(),
                match_category_name: true,
                also_match: Vec::new(),
                parent_id: None,
                value_kind: CategoryValueKind::Tag,
            }],
        };
        let settings = OllamaProviderSettings {
            enabled: true,
            ..OllamaProviderSettings::default()
        };
        let transport = FakeOllamaTransport {
            response: Some("not json".to_string()),
            fail: false,
        };
        let provider = OllamaProvider {
            settings: &settings,
            transport: &transport,
            debug: false,
        };

        let out = provider.classify(&request).expect("classify");
        assert!(out.candidates.is_empty());
        assert_eq!(
            out.debug_summary.as_deref(),
            Some("semantic[mistral]: malformed JSON response")
        );
    }

    #[test]
    fn ollama_provider_returns_valid_category_candidate() {
        let travel = CategoryDescriptor {
            id: CategoryId::new_v4(),
            name: "Travel".to_string(),
            match_category_name: true,
            also_match: Vec::new(),
            parent_id: None,
            value_kind: CategoryValueKind::Tag,
        };
        let request = ClassificationRequest {
            item_id: ItemId::new_v4(),
            text: "Book flights".to_string(),
            note: Some("Need a hotel near the conference".to_string()),
            when_date: None,
            manual_category_ids: Vec::new(),
            visible_view_name: None,
            visible_section_title: None,
            numeric_values: Vec::new(),
            literal_candidate_categories: Vec::new(),
            semantic_candidate_categories: vec![travel.clone()],
        };
        let settings = OllamaProviderSettings {
            enabled: true,
            model: "mistral".to_string(),
            ..OllamaProviderSettings::default()
        };
        let transport = FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.91,"rationale":"travel planning task"}]}"#
                    .to_string(),
            ),
            fail: false,
        };
        let provider = OllamaProvider {
            settings: &settings,
            transport: &transport,
            debug: false,
        };

        let out = provider.classify(&request).expect("classify");
        assert_eq!(out.candidates.len(), 1);
        assert_eq!(
            out.candidates[0].assignment,
            CandidateAssignment::Category(travel.id)
        );
        assert_eq!(out.candidates[0].provider, PROVIDER_ID_OLLAMA_OPENAI_COMPAT);
        assert_eq!(out.candidates[0].model.as_deref(), Some("mistral"));
        assert_eq!(out.candidates[0].confidence, Some(0.91));
        assert_eq!(
            out.candidates[0].rationale.as_deref(),
            Some("travel planning task")
        );
        assert_eq!(
            out.debug_summary.as_deref(),
            Some("semantic[mistral]: raw=1 kept=1 dropped_unknown=0 dropped_duplicate=0")
        );
    }

    #[test]
    fn ollama_provider_returns_empty_on_transport_error() {
        let request = ClassificationRequest {
            item_id: ItemId::new_v4(),
            text: "Book flights".to_string(),
            note: None,
            when_date: None,
            manual_category_ids: Vec::new(),
            visible_view_name: None,
            visible_section_title: None,
            numeric_values: Vec::new(),
            literal_candidate_categories: Vec::new(),
            semantic_candidate_categories: vec![CategoryDescriptor {
                id: CategoryId::new_v4(),
                name: "Travel".to_string(),
                match_category_name: true,
                also_match: Vec::new(),
                parent_id: None,
                value_kind: CategoryValueKind::Tag,
            }],
        };
        let settings = OllamaProviderSettings {
            enabled: true,
            ..OllamaProviderSettings::default()
        };
        let transport = FakeOllamaTransport {
            response: None,
            fail: true,
        };
        let provider = OllamaProvider {
            settings: &settings,
            transport: &transport,
            debug: false,
        };

        let out = provider
            .classify(&request)
            .expect("transport errors should be swallowed");
        assert!(out.candidates.is_empty());
        assert_eq!(
            out.debug_summary.as_deref(),
            Some("semantic[mistral]: transport error")
        );
    }

    #[test]
    fn build_request_excludes_semantic_disabled_categories_from_semantic_candidates() {
        let store = Store::open_memory().expect("open in-memory store");

        let mut literal_only = crate::model::Category::new("LiteralOnly".to_string());
        literal_only.enable_semantic_classification = false;
        let mut semantic_enabled = crate::model::Category::new("SemanticEnabled".to_string());
        semantic_enabled.enable_implicit_string = false;
        let mut numeric = crate::model::Category::new("Estimate".to_string());
        numeric.value_kind = CategoryValueKind::Numeric;

        store
            .create_category(&literal_only)
            .expect("create literal-only category");
        store
            .create_category(&semantic_enabled)
            .expect("create semantic-enabled category");
        store
            .create_category(&numeric)
            .expect("create numeric category");

        let item = Item::new("Plan a trip".to_string());
        store.create_item(&item).expect("create item");

        let service = ClassificationService::new(&store, Vec::new());
        let stored_item = store.get_item(item.id).expect("reload item");
        let request = service.build_request(&stored_item).expect("build request");

        let semantic_names: Vec<&str> = request
            .semantic_candidate_categories
            .iter()
            .map(|category| category.name.as_str())
            .collect();
        assert!(semantic_names.contains(&"SemanticEnabled"));
        assert!(!semantic_names.contains(&"LiteralOnly"));
        assert!(!semantic_names.contains(&"Estimate"));

        let literal_names: Vec<&str> = request
            .literal_candidate_categories
            .iter()
            .map(|category| category.name.as_str())
            .collect();
        assert!(literal_names.contains(&"LiteralOnly"));
        assert!(!literal_names.contains(&"SemanticEnabled"));
        assert!(!literal_names.contains(&"Estimate"));
    }
}
