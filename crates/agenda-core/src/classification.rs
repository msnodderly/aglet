use jiff::civil::{Date, DateTime};
use jiff::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dates::{BasicDateParser, DateParser};
use crate::error::Result;
use crate::matcher::{Classifier, ImplicitMatchSource};
use crate::model::{
    AssignmentSource, CategoryId, CategoryValueKind, Item, ItemId, RESERVED_CATEGORY_NAMES,
};
use crate::store::Store;

pub const CLASSIFICATION_CONFIG_KEY: &str = "classification.config.v1";
pub const PROVIDER_ID_IMPLICIT_STRING: &str = "implicit_string";
pub const PROVIDER_ID_WHEN_PARSER: &str = "when_parser";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClassificationConfig {
    pub enabled: bool,
    pub continuous_mode: ContinuousMode,
    pub run_on_item_save: bool,
    pub run_on_category_change: bool,
    pub enabled_providers: Vec<ProviderConfig>,
}

impl Default for ClassificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            continuous_mode: ContinuousMode::AutoApply,
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
            ],
        }
    }
}

impl ClassificationConfig {
    pub fn should_run_continuously(&self) -> bool {
        self.enabled && self.continuous_mode != ContinuousMode::Off
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContinuousMode {
    Off,
    AutoApply,
    SuggestReview,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryDescriptor {
    pub id: CategoryId,
    pub name: String,
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
    pub candidate_categories: Vec<CategoryDescriptor>,
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
        for category in &request.candidate_categories {
            let Some(matched) =
                self.classifier
                    .classify(&match_text, &category.name, &category.also_match)
            else {
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
            if !provider.is_cheap() {
                continue;
            }
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
        let candidate_categories = categories
            .into_iter()
            .filter(|category| {
                category.enable_implicit_string
                    && category.value_kind != CategoryValueKind::Numeric
                    && !RESERVED_CATEGORY_NAMES
                        .iter()
                        .any(|reserved| reserved.eq_ignore_ascii_case(&category.name))
            })
            .map(|category| CategoryDescriptor {
                id: category.id,
                name: category.name,
                also_match: category.also_match,
                parent_id: category.parent,
                value_kind: category.value_kind,
            })
            .collect();

        Ok(ClassificationRequest {
            item_id: item.id,
            text: item.text.clone(),
            note: item.note.clone(),
            when_date: item.when_date,
            manual_category_ids,
            visible_view_name: None,
            visible_section_title: None,
            numeric_values,
            candidate_categories,
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
