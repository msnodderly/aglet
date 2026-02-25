use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub type CategoryId = Uuid;
pub type ItemId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub text: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub entry_date: NaiveDate,
    pub when_date: Option<NaiveDateTime>,
    pub done_date: Option<NaiveDateTime>,
    pub is_done: bool,
    pub assignments: HashMap<CategoryId, Assignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub source: AssignmentSource,
    pub assigned_at: DateTime<Utc>,
    pub sticky: bool,
    pub origin: Option<String>,
    #[serde(default)]
    pub numeric_value: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentSource {
    Manual,
    AutoMatch,
    Action,
    Subsumption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub parent: Option<CategoryId>,
    pub children: Vec<CategoryId>,
    pub is_exclusive: bool,
    pub is_actionable: bool,
    pub enable_implicit_string: bool,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub value_kind: CategoryValueKind,
    #[serde(default)]
    pub numeric_format: Option<NumericFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CategoryValueKind {
    #[default]
    Tag,
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumericFormat {
    #[serde(default = "default_numeric_decimal_places")]
    pub decimal_places: u8,
    #[serde(default)]
    pub currency_symbol: Option<String>,
    #[serde(default)]
    pub use_thousands_separator: bool,
}

const fn default_numeric_decimal_places() -> u8 {
    2
}

impl Default for NumericFormat {
    fn default() -> Self {
        Self {
            decimal_places: default_numeric_decimal_places(),
            currency_symbol: None,
            use_thousands_separator: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    ImplicitString,
    Profile { criteria: Box<Query> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Assign { targets: HashSet<CategoryId> },
    Remove { targets: HashSet<CategoryId> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub id: Uuid,
    pub name: String,
    pub criteria: Query,
    pub sections: Vec<Section>,
    pub show_unmatched: bool,
    pub unmatched_label: String,
    pub remove_from_view_unassign: HashSet<CategoryId>,
    #[serde(default)]
    pub item_column_label: Option<String>,
    #[serde(default)]
    pub board_display_mode: BoardDisplayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardDisplayMode {
    #[default]
    SingleLine,
    MultiLine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub criteria: Query,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub item_column_index: usize,
    pub on_insert_assign: HashSet<CategoryId>,
    pub on_remove_unassign: HashSet<CategoryId>,
    pub show_children: bool,
    #[serde(default)]
    pub board_display_mode_override: Option<BoardDisplayMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColumnKind {
    When,
    #[default]
    Standard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    #[serde(default)]
    pub kind: ColumnKind,
    pub heading: CategoryId,
    pub width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionMode {
    And,
    Not,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub mode: CriterionMode,
    pub category_id: CategoryId,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub criteria: Vec<Criterion>,
    pub virtual_include: HashSet<WhenBucket>,
    pub virtual_exclude: HashSet<WhenBucket>,
    pub text_search: Option<String>,
}

impl Query {
    /// Iterator over And-mode category IDs.
    pub fn and_category_ids(&self) -> impl Iterator<Item = CategoryId> + '_ {
        self.criteria
            .iter()
            .filter(|c| c.mode == CriterionMode::And)
            .map(|c| c.category_id)
    }

    /// Iterator over Not-mode category IDs.
    pub fn not_category_ids(&self) -> impl Iterator<Item = CategoryId> + '_ {
        self.criteria
            .iter()
            .filter(|c| c.mode == CriterionMode::Not)
            .map(|c| c.category_id)
    }

    /// Iterator over Or-mode category IDs.
    pub fn or_category_ids(&self) -> impl Iterator<Item = CategoryId> + '_ {
        self.criteria
            .iter()
            .filter(|c| c.mode == CriterionMode::Or)
            .map(|c| c.category_id)
    }

    /// Add or replace a criterion for the given category ID. No duplicate cat_ids.
    pub fn set_criterion(&mut self, mode: CriterionMode, category_id: CategoryId) {
        if let Some(existing) = self
            .criteria
            .iter_mut()
            .find(|c| c.category_id == category_id)
        {
            existing.mode = mode;
        } else {
            self.criteria.push(Criterion { mode, category_id });
        }
    }

    /// Remove criterion by category ID.
    pub fn remove_criterion(&mut self, category_id: CategoryId) {
        self.criteria.retain(|c| c.category_id != category_id);
    }

    /// Get the mode for a category ID, if present.
    pub fn mode_for(&self, category_id: CategoryId) -> Option<CriterionMode> {
        self.criteria
            .iter()
            .find(|c| c.category_id == category_id)
            .map(|c| c.mode)
    }
}

// Custom serde for Query: serializes the new `criteria` format, but can
// deserialize both the old format (include/exclude HashSets) and the new one.

impl Serialize for Query {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct NewFormat<'a> {
            criteria: &'a Vec<Criterion>,
            virtual_include: &'a HashSet<WhenBucket>,
            virtual_exclude: &'a HashSet<WhenBucket>,
            text_search: &'a Option<String>,
        }

        NewFormat {
            criteria: &self.criteria,
            virtual_include: &self.virtual_include,
            virtual_exclude: &self.virtual_exclude,
            text_search: &self.text_search,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Query {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawQuery {
            // New format field
            #[serde(default)]
            criteria: Option<Vec<Criterion>>,
            // Old format fields
            #[serde(default)]
            include: Option<HashSet<CategoryId>>,
            #[serde(default)]
            exclude: Option<HashSet<CategoryId>>,
            // Common fields
            #[serde(default)]
            virtual_include: HashSet<WhenBucket>,
            #[serde(default)]
            virtual_exclude: HashSet<WhenBucket>,
            #[serde(default)]
            text_search: Option<String>,
        }

        let raw = RawQuery::deserialize(deserializer)?;

        let criteria = if let Some(criteria) = raw.criteria {
            criteria
        } else {
            // Migrate from old format
            let mut migrated = Vec::new();
            if let Some(include) = raw.include {
                for id in include {
                    migrated.push(Criterion {
                        mode: CriterionMode::And,
                        category_id: id,
                    });
                }
            }
            if let Some(exclude) = raw.exclude {
                for id in exclude {
                    migrated.push(Criterion {
                        mode: CriterionMode::Not,
                        category_id: id,
                    });
                }
            }
            migrated
        };

        Ok(Query {
            criteria,
            virtual_include: raw.virtual_include,
            virtual_exclude: raw.virtual_exclude,
            text_search: raw.text_search,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WhenBucket {
    Overdue,
    Today,
    Tomorrow,
    ThisWeek,
    NextWeek,
    ThisMonth,
    Future,
    NoDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionLogEntry {
    pub id: Uuid,
    pub item_id: Uuid,
    pub text: String,
    pub note: Option<String>,
    pub entry_date: NaiveDate,
    pub when_date: Option<NaiveDateTime>,
    pub done_date: Option<NaiveDateTime>,
    pub is_done: bool,
    pub assignments_json: String,
    pub deleted_at: DateTime<Utc>,
    pub deleted_by: String,
}

impl Item {
    pub fn new(text: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            text,
            note: None,
            created_at: now,
            modified_at: now,
            entry_date: now.date_naive(),
            when_date: None,
            done_date: None,
            is_done: false,
            assignments: HashMap::new(),
        }
    }
}

impl Category {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            parent: None,
            children: Vec::new(),
            is_exclusive: false,
            is_actionable: true,
            enable_implicit_string: true,
            note: None,
            created_at: now,
            modified_at: now,
            conditions: Vec::new(),
            actions: Vec::new(),
            value_kind: CategoryValueKind::Tag,
            numeric_format: None,
        }
    }
}

impl View {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            criteria: Query::default(),
            sections: Vec::new(),
            show_unmatched: true,
            unmatched_label: "Unassigned".to_string(),
            remove_from_view_unassign: HashSet::new(),
            item_column_label: None,
            board_display_mode: BoardDisplayMode::SingleLine,
        }
    }
}
