use std::collections::{HashMap, HashSet};
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
            timeout_secs: 10,
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
    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>>;
    fn is_cheap(&self) -> bool {
        false
    }
}

pub trait OllamaTransport: Send + Sync {
    fn complete(
        &self,
        settings: &OllamaProviderSettings,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<Option<String>>;
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

    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>> {
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
        Ok(out)
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

    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>> {
        let Some(parsed) = self.parser.parse(&request.text, self.reference_date) else {
            return Ok(Vec::new());
        };
        let matched_text = request
            .text
            .get(parsed.span.0..parsed.span.1)
            .unwrap_or("")
            .to_string();

        Ok(vec![ClassificationCandidate {
            item_id: request.item_id,
            assignment: CandidateAssignment::When(parsed.datetime),
            provider: self.id().to_string(),
            model: None,
            confidence: Some(1.0),
            rationale: Some(format!("parsed date expression '{}'", matched_text)),
            context_hash: "request:v1".to_string(),
        }])
    }
}

pub struct OllamaProvider<'a> {
    pub settings: &'a OllamaProviderSettings,
    pub transport: &'a dyn OllamaTransport,
}

impl ClassificationProvider for OllamaProvider<'_> {
    fn id(&self) -> &'static str {
        PROVIDER_ID_OLLAMA_OPENAI_COMPAT
    }

    fn classify(&self, request: &ClassificationRequest) -> Result<Vec<ClassificationCandidate>> {
        if !self.settings.enabled
            || self.settings.model.trim().is_empty()
            || self.settings.base_url.trim().is_empty()
            || request.semantic_candidate_categories.is_empty()
        {
            return Ok(Vec::new());
        }

        let system_prompt = "You classify a single item into existing categories. Return strict JSON only with shape {\"suggestions\":[{\"category\":\"Exact Category Name\",\"confidence\":0.0,\"rationale\":\"short reason\"}]}. Use only exact category names from the provided list. Any category name not exactly present in the allowed list is invalid and will be discarded. Do not invent, expand, paraphrase, or rewrite category names. If nothing applies, return {\"suggestions\":[]}. Suggest at most 3 categories.";
        let user_prompt = build_ollama_user_prompt(request);
        let content = match self
            .transport
            .complete(self.settings, system_prompt, &user_prompt)
        {
            Ok(content) => content,
            Err(_) => return Ok(Vec::new()),
        };
        let Some(content) = content else {
            return Ok(Vec::new());
        };

        Ok(parse_ollama_suggestions(
            request,
            &content,
            &self.settings.model,
            self.id(),
        ))
    }
}

fn build_ollama_user_prompt(request: &ClassificationRequest) -> String {
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
    if request.manual_category_ids.is_empty() {
        lines.push("Manual categories: (none)".to_string());
    } else {
        let mut manual_ids: Vec<String> = request
            .manual_category_ids
            .iter()
            .map(ToString::to_string)
            .collect();
        manual_ids.sort();
        lines.push(format!("Manual category ids: {}", manual_ids.join(", ")));
    }
    if request.numeric_values.is_empty() {
        lines.push("Numeric assignments: (none)".to_string());
    } else {
        let mut numeric_values: Vec<String> = request
            .numeric_values
            .iter()
            .map(|(id, value)| format!("{id}={value}"))
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
            "(none)".to_string()
        } else {
            category.also_match.join(", ")
        };
        lines.push(format!(
            "- {} | aliases: {} | parent: {}",
            category.name,
            aliases,
            category
                .parent_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "(root)".to_string())
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
) -> Vec<ClassificationCandidate> {
    let Ok(parsed) = serde_json::from_str::<OllamaSuggestionEnvelope>(content) else {
        return Vec::new();
    };
    let category_map: HashMap<String, &CategoryDescriptor> = request
        .semantic_candidate_categories
        .iter()
        .map(|category| (category.name.to_ascii_lowercase(), category))
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for suggestion in parsed.suggestions.into_iter().take(3) {
        let key = suggestion.category.trim().to_ascii_lowercase();
        let Some(category) = category_map.get(&key) else {
            continue;
        };
        if !seen.insert(category.id) {
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

    out
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
    ) -> Result<(Item, String, Vec<ClassificationCandidate>)> {
        let item = self.store.get_item(item_id)?;
        let request = self.build_request(&item)?;
        let item_revision_hash = item_revision_hash(&item);
        let mut candidates = Vec::new();

        for provider in &self.providers {
            candidates.extend(provider.classify(&request)?);
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

        Ok((item, item_revision_hash, candidates))
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
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].assignment,
            CandidateAssignment::Category(category_a.id)
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
        };

        let out = provider.classify(&request).expect("classify");
        assert!(out.is_empty());
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
        };

        let out = provider.classify(&request).expect("classify");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].assignment, CandidateAssignment::Category(travel.id));
        assert_eq!(out[0].provider, PROVIDER_ID_OLLAMA_OPENAI_COMPAT);
        assert_eq!(out[0].model.as_deref(), Some("mistral"));
        assert_eq!(out[0].confidence, Some(0.91));
        assert_eq!(
            out[0].rationale.as_deref(),
            Some("travel planning task")
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
        };

        let out = provider.classify(&request).expect("transport errors should be swallowed");
        assert!(out.is_empty());
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
