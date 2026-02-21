use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub criteria: Query,
    #[serde(default)]
    pub columns: Vec<Column>,
    pub on_insert_assign: HashSet<CategoryId>,
    pub on_remove_unassign: HashSet<CategoryId>,
    pub show_children: bool,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Query {
    pub include: HashSet<CategoryId>,
    pub exclude: HashSet<CategoryId>,
    pub virtual_include: HashSet<WhenBucket>,
    pub virtual_exclude: HashSet<WhenBucket>,
    pub text_search: Option<String>,
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
        }
    }
}
