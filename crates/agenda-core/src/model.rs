use jiff::civil::{Date, DateTime, Time};
use jiff::Timestamp;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// Names of the three built-in categories that are always present and cannot
/// be renamed or deleted.
pub const RESERVED_CATEGORY_NAME_WHEN: &str = "When";
pub const RESERVED_CATEGORY_NAME_ENTRY: &str = "Entry";
pub const RESERVED_CATEGORY_NAME_DONE: &str = "Done";
pub const RESERVED_CATEGORY_NAMES: [&str; 3] = [
    RESERVED_CATEGORY_NAME_WHEN,
    RESERVED_CATEGORY_NAME_ENTRY,
    RESERVED_CATEGORY_NAME_DONE,
];

pub type CategoryId = Uuid;
pub type ItemId = Uuid;

/// Canonical `origin` string constants used in [`Assignment::origin`] and [`ItemLink::origin`].
///
/// These are stored as-is in the database, so changing a value here is a breaking
/// change that requires a migration. For subsumption origins, the category name is
/// appended: `format!("{}:{category_name}", ORIGIN_SUBSUMPTION)`.
pub mod origin {
    /// Explicit user assignment (generic).
    pub const MANUAL: &str = "manual";
    /// User marked the item done.
    pub const MANUAL_DONE: &str = "manual:done";
    /// User entered a numeric value for a category.
    pub const MANUAL_NUMERIC: &str = "manual:numeric";
    /// User created an item link.
    pub const MANUAL_LINK: &str = "manual:link";
    /// User explicitly edited the When datetime.
    pub const MANUAL_WHEN: &str = "manual:when";
    /// NLP date parser inferred a When date.
    pub const NLP_DATE: &str = "nlp:date";
    /// Engine auto-assigned via category hierarchy subsumption.
    /// Full value: `format!("{}:{category_name}", SUBSUMPTION)`.
    pub const SUBSUMPTION: &str = "subsumption";
    /// Assignment carried forward from a completed recurrence parent.
    pub const RECURRENCE_CARRY: &str = "recurrence:carry";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemLinkKind {
    #[serde(rename = "depends-on")]
    DependsOn,
    #[serde(rename = "related")]
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLink {
    /// Endpoint semantics depend on `kind`:
    /// - DependsOn: item_id = dependent, other_item_id = dependency
    /// - Related: normalized unordered pair (item_id < other_item_id)
    pub item_id: ItemId,
    pub other_item_id: ItemId,
    pub kind: ItemLinkKind,
    pub created_at: Timestamp,
    /// How this link was created. See [`origin`] for canonical values.
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ItemLinksForItem {
    pub depends_on: Vec<ItemId>,
    pub blocks: Vec<ItemId>,
    pub related: Vec<ItemId>,
}

/// The frequency component of a [`RecurrenceRule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceFrequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// Defines how a recurring item generates its next instance when marked done.
///
/// The rule is stored on the item itself. When `mark_item_done` fires, the engine
/// uses this rule to compute the successor's `when_date` from the completed item's
/// anchor date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub frequency: RecurrenceFrequency,
    /// Repeat every `interval` units (1 = every, 2 = every other, etc.).
    pub interval: u16,
    /// For weekly: which day of the week (1=Mon .. 7=Sun). `None` means same weekday
    /// as the anchor. Stored as u8 because jiff::civil::Weekday doesn't impl Serialize.
    pub weekday: Option<u8>,
    /// For monthly/yearly: which day of the month (1–31). Clamped to the last day
    /// of the target month when the month has fewer days.
    pub day_of_month: Option<u8>,
    /// For yearly: which month (1–12).
    pub month: Option<u8>,
    /// When `true` on a Daily rule, skip Saturday and Sunday.
    #[serde(default)]
    pub weekdays_only: Option<bool>,
}

impl RecurrenceRule {
    /// Compute the next occurrence date after `anchor`.
    ///
    /// For monthly rules with `day_of_month` exceeding the target month's length,
    /// the date is clamped to the last day of that month.
    pub fn next_date(&self, anchor: jiff::civil::DateTime) -> jiff::civil::DateTime {
        use jiff::civil::Date;

        match self.frequency {
            RecurrenceFrequency::Daily => {
                let span = jiff::Span::new().days(i64::from(self.interval));
                let mut next = anchor.checked_add(span).expect("daily advance overflow");
                if self.weekdays_only == Some(true) {
                    use jiff::civil::Weekday;
                    loop {
                        match next.date().weekday() {
                            Weekday::Saturday => {
                                next = next
                                    .checked_add(jiff::Span::new().days(2))
                                    .expect("weekday skip overflow");
                            }
                            Weekday::Sunday => {
                                next = next
                                    .checked_add(jiff::Span::new().days(1))
                                    .expect("weekday skip overflow");
                            }
                            _ => break,
                        }
                    }
                }
                next
            }
            RecurrenceFrequency::Weekly => {
                let target_wd = self
                    .weekday
                    .map(weekday_from_u8)
                    .unwrap_or_else(|| anchor.date().weekday());
                let base = anchor
                    .checked_add(jiff::Span::new().weeks(i64::from(self.interval)))
                    .expect("weekly advance overflow");
                // Adjust to target weekday within the same week
                let current_wd = base.date().weekday();
                let diff = (target_wd.to_monday_zero_offset() as i64)
                    - (current_wd.to_monday_zero_offset() as i64);
                base.checked_add(jiff::Span::new().days(diff))
                    .expect("weekday adjust overflow")
            }
            RecurrenceFrequency::Monthly => {
                let target_day = self.day_of_month.unwrap_or(anchor.date().day() as u8);
                let months_ahead = self.interval as i16;
                let raw_month = i16::from(anchor.date().month()) + months_ahead;
                let year = anchor.date().year() + (raw_month - 1) / 12;
                let month = ((raw_month - 1) % 12 + 1) as u8;
                let max_day = days_in_month(i32::from(year), month);
                let day = target_day.min(max_day);
                let date = Date::new(year, month as i8, day as i8).expect("valid monthly date");
                date.at(anchor.hour(), anchor.minute(), anchor.second(), 0)
            }
            RecurrenceFrequency::Yearly => {
                let target_month = self.month.unwrap_or(anchor.date().month() as u8);
                let target_day = self.day_of_month.unwrap_or(anchor.date().day() as u8);
                let year = anchor.date().year() + self.interval as i16;
                let max_day = days_in_month(i32::from(year), target_month);
                let day = target_day.min(max_day);
                let date =
                    Date::new(year, target_month as i8, day as i8).expect("valid yearly date");
                date.at(anchor.hour(), anchor.minute(), anchor.second(), 0)
            }
        }
    }

    /// Human-readable description of the rule.
    pub fn display(&self) -> String {
        match self.frequency {
            RecurrenceFrequency::Daily if self.weekdays_only == Some(true) => {
                "every weekday".to_string()
            }
            RecurrenceFrequency::Daily if self.interval == 1 => "daily".to_string(),
            RecurrenceFrequency::Daily => format!("every {} days", self.interval),
            RecurrenceFrequency::Weekly if self.interval == 1 => match self.weekday {
                Some(wd) => format!("every {}", weekday_name(weekday_from_u8(wd))),
                None => "weekly".to_string(),
            },
            RecurrenceFrequency::Weekly => match self.weekday {
                Some(wd) => format!(
                    "every {} weeks on {}",
                    self.interval,
                    weekday_name(weekday_from_u8(wd))
                ),
                None => format!("every {} weeks", self.interval),
            },
            RecurrenceFrequency::Monthly if self.interval == 1 => {
                match self.day_of_month {
                    Some(d) => format!("monthly on the {}", ordinal(d)),
                    None => "monthly".to_string(),
                }
            }
            RecurrenceFrequency::Monthly
                if self.interval == 3 && self.day_of_month == Some(1) =>
            {
                "quarterly".to_string()
            }
            RecurrenceFrequency::Monthly => {
                match self.day_of_month {
                    Some(d) => format!("every {} months on the {}", self.interval, ordinal(d)),
                    None => format!("every {} months", self.interval),
                }
            }
            RecurrenceFrequency::Yearly if self.interval == 1 => match (self.month, self.day_of_month) {
                (Some(m), Some(d)) => format!("every {} {}", month_name(m), d),
                _ => "yearly".to_string(),
            },
            RecurrenceFrequency::Yearly => format!("every {} years", self.interval),
        }
    }
}

/// Convert 1=Mon .. 7=Sun to jiff::civil::Weekday. Clamps invalid values to Monday.
pub fn weekday_from_u8(n: u8) -> jiff::civil::Weekday {
    use jiff::civil::Weekday;
    match n {
        1 => Weekday::Monday,
        2 => Weekday::Tuesday,
        3 => Weekday::Wednesday,
        4 => Weekday::Thursday,
        5 => Weekday::Friday,
        6 => Weekday::Saturday,
        7 => Weekday::Sunday,
        _ => Weekday::Monday,
    }
}

/// Convert jiff::civil::Weekday to 1=Mon .. 7=Sun.
pub fn weekday_to_u8(wd: jiff::civil::Weekday) -> u8 {
    wd.to_monday_one_offset() as u8
}

pub(crate) fn weekday_name(wd: jiff::civil::Weekday) -> &'static str {
    match wd {
        jiff::civil::Weekday::Monday => "Monday",
        jiff::civil::Weekday::Tuesday => "Tuesday",
        jiff::civil::Weekday::Wednesday => "Wednesday",
        jiff::civil::Weekday::Thursday => "Thursday",
        jiff::civil::Weekday::Friday => "Friday",
        jiff::civil::Weekday::Saturday => "Saturday",
        jiff::civil::Weekday::Sunday => "Sunday",
    }
}

pub(crate) fn month_name(m: u8) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "???",
    }
}

fn ordinal(n: u8) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Returns the number of days in the given month (1–12) for the given year.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub text: String,
    pub note: Option<String>,
    /// When set, the item's note is stored in an external markdown file
    /// instead of the inline `note` column. The value is a bare filename
    /// (e.g., `build-auth-a3f8b2c1.md`) resolved relative to the notes directory.
    #[serde(default)]
    pub note_file: Option<String>,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
    pub when_date: Option<jiff::civil::DateTime>,
    pub done_date: Option<jiff::civil::DateTime>,
    pub is_done: bool,
    pub assignments: HashMap<CategoryId, Assignment>,
    /// Recurrence rule for succession-based recurring items.
    #[serde(default)]
    pub recurrence_rule: Option<RecurrenceRule>,
    /// Groups all items in the same recurrence series.
    #[serde(default)]
    pub recurrence_series_id: Option<Uuid>,
    /// Points to the completed item that spawned this successor.
    #[serde(default)]
    pub recurrence_parent_item_id: Option<ItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub source: AssignmentSource,
    pub assigned_at: Timestamp,
    pub sticky: bool,
    /// How this assignment was created. See [`origin`] for canonical values.
    pub origin: Option<String>,
    #[serde(default)]
    pub explanation: Option<AssignmentExplanation>,
    #[serde(default)]
    pub numeric_value: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentSource {
    Manual,
    AutoMatch,
    AutoClassified,
    SuggestionAccepted,
    Action,
    Subsumption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextMatchSource {
    CategoryName,
    AlsoMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentActionKind {
    Assign,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentExplanation {
    Manual {
        origin: Option<String>,
    },
    ImplicitMatch {
        matched_term: String,
        matched_source: TextMatchSource,
    },
    ProfileCondition {
        owner_category_name: String,
        condition_index: usize,
        rendered_rule: String,
    },
    DateCondition {
        owner_category_name: String,
        condition_index: usize,
        rendered_rule: String,
    },
    ConditionGroup {
        owner_category_name: String,
        match_mode: ConditionMatchMode,
        rendered_rules: Vec<String>,
    },
    Action {
        trigger_category_name: String,
        kind: AssignmentActionKind,
    },
    Subsumption {
        parent_category_name: String,
        via_child_category_name: String,
    },
    SuggestionAccepted {
        provider_id: String,
        model: Option<String>,
        rationale: Option<String>,
    },
    AutoClassified {
        provider_id: String,
        model: Option<String>,
        rationale: Option<String>,
    },
}

impl AssignmentExplanation {
    pub fn summary(&self) -> String {
        match self {
            Self::Manual { .. } => "Assigned manually".to_string(),
            Self::ImplicitMatch {
                matched_term,
                matched_source,
            } => match matched_source {
                TextMatchSource::CategoryName => {
                    format!("Matched category name \"{matched_term}\"")
                }
                TextMatchSource::AlsoMatch => format!("Matched alias \"{matched_term}\""),
            },
            Self::ProfileCondition {
                owner_category_name,
                condition_index,
                rendered_rule,
            } => format!(
                "Derived from profile rule {} on {}: {}",
                condition_index + 1,
                owner_category_name,
                rendered_rule
            ),
            Self::DateCondition {
                owner_category_name,
                condition_index,
                rendered_rule,
            } => format!(
                "Derived from date rule {} on {}: {}",
                condition_index + 1,
                owner_category_name,
                rendered_rule
            ),
            Self::ConditionGroup {
                owner_category_name,
                match_mode,
                rendered_rules,
            } => {
                let mode = match match_mode {
                    ConditionMatchMode::Any => "ANY",
                    ConditionMatchMode::All => "ALL",
                };
                if rendered_rules.is_empty() {
                    format!("Derived from {mode} rules on {owner_category_name}")
                } else {
                    format!(
                        "Derived from {mode} rules on {}: {}",
                        owner_category_name,
                        rendered_rules.join(" AND ")
                    )
                }
            }
            Self::Action {
                trigger_category_name,
                kind,
            } => match kind {
                AssignmentActionKind::Assign => {
                    format!("Assigned by action on {trigger_category_name}")
                }
                AssignmentActionKind::Remove => {
                    format!("Removed by action on {trigger_category_name}")
                }
            },
            Self::Subsumption {
                via_child_category_name,
                ..
            } => format!("Inherited from child {via_child_category_name}"),
            Self::SuggestionAccepted {
                provider_id,
                rationale,
                ..
            } => rationale
                .as_ref()
                .map(|rationale| format!("Accepted suggestion from {provider_id}: {rationale}"))
                .unwrap_or_else(|| format!("Accepted suggestion from {provider_id}")),
            Self::AutoClassified {
                provider_id,
                rationale,
                ..
            } => rationale
                .as_ref()
                .map(|rationale| format!("Auto-classified by {provider_id}: {rationale}"))
                .unwrap_or_else(|| format!("Auto-classified by {provider_id}")),
        }
    }

    pub fn removal_summary(&self) -> String {
        match self {
            Self::Manual { .. } => "Removed manually".to_string(),
            Self::ImplicitMatch {
                matched_term,
                matched_source,
            } => match matched_source {
                TextMatchSource::CategoryName => {
                    format!("Text no longer matched category name \"{matched_term}\"")
                }
                TextMatchSource::AlsoMatch => {
                    format!("Text no longer matched alias \"{matched_term}\"")
                }
            },
            Self::ProfileCondition {
                owner_category_name,
                condition_index,
                ..
            } => format!(
                "Profile rule {} on {} no longer matched",
                condition_index + 1,
                owner_category_name
            ),
            Self::DateCondition {
                owner_category_name,
                condition_index,
                ..
            } => format!(
                "Date rule {} on {} no longer matched",
                condition_index + 1,
                owner_category_name
            ),
            Self::ConditionGroup {
                owner_category_name,
                match_mode,
                ..
            } => match match_mode {
                ConditionMatchMode::Any => {
                    format!("No rules on {} matched anymore", owner_category_name)
                }
                ConditionMatchMode::All => {
                    format!("Not all rules on {} matched anymore", owner_category_name)
                }
            },
            Self::Action {
                trigger_category_name,
                kind,
            } => match kind {
                AssignmentActionKind::Assign => {
                    format!("Removed after action-triggered assignment from {trigger_category_name}")
                }
                AssignmentActionKind::Remove => {
                    format!("Removed by action on {trigger_category_name}")
                }
            },
            Self::Subsumption {
                via_child_category_name,
                ..
            } => format!(
                "Supporting child {} is no longer assigned",
                via_child_category_name
            ),
            Self::SuggestionAccepted { provider_id, .. } => {
                format!("Accepted suggestion from {provider_id} was removed")
            }
            Self::AutoClassified { provider_id, .. } => {
                format!("Auto-classified assignment from {provider_id} was removed")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentEventKind {
    Assigned,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentEvent {
    pub kind: AssignmentEventKind,
    pub category_id: CategoryId,
    pub category_name: String,
    pub summary: String,
}

impl AssignmentEvent {
    pub fn concise_summary(&self) -> String {
        match self.kind {
            AssignmentEventKind::Assigned if self.summary == "Assigned manually" => {
                format!("Added {}", self.category_name)
            }
            AssignmentEventKind::Removed if self.summary == "Removed manually" => {
                format!("Removed {}", self.category_name)
            }
            AssignmentEventKind::Assigned => {
                format!("Auto-added {} ({})", self.category_name, self.summary)
            }
            AssignmentEventKind::Removed => {
                format!("Auto-removed {} ({})", self.category_name, self.summary)
            }
        }
    }
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
    #[serde(default = "default_true")]
    pub enable_semantic_classification: bool,
    #[serde(default = "default_true")]
    pub match_category_name: bool,
    #[serde(default)]
    pub also_match: Vec<String>,
    pub note: Option<String>,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
    #[serde(default)]
    pub condition_match_mode: ConditionMatchMode,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub value_kind: CategoryValueKind,
    #[serde(default)]
    pub numeric_format: Option<NumericFormat>,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CategoryValueKind {
    #[default]
    Tag,
    Numeric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConditionMatchMode {
    #[default]
    Any,
    All,
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
    Date {
        source: DateSource,
        matcher: DateMatcher,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateSource {
    When,
    Entry,
    Done,
}

impl DateSource {
    pub fn next(self) -> Self {
        match self {
            Self::When => Self::Entry,
            Self::Entry => Self::Done,
            Self::Done => Self::When,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::When => "When",
            Self::Entry => "Entry",
            Self::Done => "Done",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateCompareOp {
    On,
    Before,
    After,
    AtOrBefore,
    AtOrAfter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateValueExpr {
    Today,
    Tomorrow,
    DaysFromToday(i32),
    DaysAgo(i32),
    AbsoluteDate(Date),
    AbsoluteDateTime(DateTime),
    TimeToday(Time),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateMatcher {
    Compare {
        op: DateCompareOp,
        value: DateValueExpr,
    },
    Range {
        from: DateValueExpr,
        through: DateValueExpr,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Assign { targets: HashSet<CategoryId> },
    Remove { targets: HashSet<CategoryId> },
}

impl Action {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Assign { .. } => "Assign",
            Self::Remove { .. } => "Remove",
        }
    }

    pub fn category_targets(&self) -> Option<&HashSet<CategoryId>> {
        match self {
            Self::Assign { targets } | Self::Remove { targets } => Some(targets),
        }
    }
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
    pub category_aliases: BTreeMap<CategoryId, String>,
    #[serde(default)]
    pub item_column_label: Option<String>,
    #[serde(default)]
    pub board_display_mode: BoardDisplayMode,
    #[serde(default)]
    pub section_flow: SectionFlow,
    #[serde(default)]
    pub hide_dependent_items: bool,
    #[serde(default)]
    pub datebook_config: Option<DatebookConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardDisplayMode {
    #[default]
    SingleLine,
    MultiLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SectionFlow {
    #[default]
    Vertical,
    Horizontal,
}

// ── Datebook view types ─────────────────────────────────────────────

/// The total time span shown in a datebook view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookPeriod {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl DatebookPeriod {
    pub fn next(self) -> Self {
        match self {
            Self::Day => Self::Week,
            Self::Week => Self::Month,
            Self::Month => Self::Quarter,
            Self::Quarter => Self::Year,
            Self::Year => Self::Day,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Week => "Week",
            Self::Month => "Month",
            Self::Quarter => "Quarter",
            Self::Year => "Year",
        }
    }
}

/// The granularity of each auto-generated section within the period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookInterval {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl DatebookInterval {
    pub fn next(self) -> Self {
        match self {
            Self::Hourly => Self::Daily,
            Self::Daily => Self::Weekly,
            Self::Weekly => Self::Monthly,
            Self::Monthly => Self::Hourly,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hourly => "Hourly",
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
        }
    }
}

/// How the base date of the datebook window is anchored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookAnchor {
    Today,
    StartOfWeek,
    StartOfMonth,
    StartOfQuarter,
    StartOfYear,
    Absolute(jiff::civil::Date),
}

impl DatebookAnchor {
    pub fn next(&self) -> Self {
        match self {
            Self::Today => Self::StartOfWeek,
            Self::StartOfWeek => Self::StartOfMonth,
            Self::StartOfMonth => Self::StartOfQuarter,
            Self::StartOfQuarter => Self::StartOfYear,
            Self::StartOfYear => Self::Today,
            Self::Absolute(_) => Self::Today,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::StartOfWeek => "Start of week",
            Self::StartOfMonth => "Start of month",
            Self::StartOfQuarter => "Start of quarter",
            Self::StartOfYear => "Start of year",
            Self::Absolute(_) => "Absolute date",
        }
    }
}

/// Controls how empty sections are displayed on the board.
///
/// Defined as a standalone enum so it can later be promoted to a view-level
/// setting without restructuring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EmptySections {
    /// All sections rendered with equal space (default).
    #[default]
    Show,
    /// Empty sections collapse to a single header line.
    Collapse,
    /// Empty sections are hidden entirely.
    Hide,
}

impl EmptySections {
    pub fn next(self) -> Self {
        match self {
            Self::Show => Self::Collapse,
            Self::Collapse => Self::Hide,
            Self::Hide => Self::Show,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Show => "Show all",
            Self::Collapse => "Collapse",
            Self::Hide => "Hide",
        }
    }
}

/// Configuration for a datebook (time-interval) view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatebookConfig {
    pub period: DatebookPeriod,
    pub interval: DatebookInterval,
    pub anchor: DatebookAnchor,
    pub date_source: DateSource,
    /// How to display empty time-slot sections.
    #[serde(default)]
    pub empty_sections: EmptySections,
    /// Signed offset: +1 = shift forward by one period, -1 backward.
    #[serde(default)]
    pub browse_offset: i32,
}

impl DatebookConfig {
    /// Validate that the interval is finer than the period.
    pub fn is_valid(&self) -> bool {
        match self.period {
            DatebookPeriod::Day => matches!(self.interval, DatebookInterval::Hourly),
            DatebookPeriod::Week => matches!(
                self.interval,
                DatebookInterval::Hourly | DatebookInterval::Daily
            ),
            DatebookPeriod::Month => matches!(
                self.interval,
                DatebookInterval::Daily | DatebookInterval::Weekly
            ),
            DatebookPeriod::Quarter => matches!(
                self.interval,
                DatebookInterval::Weekly | DatebookInterval::Monthly
            ),
            DatebookPeriod::Year => matches!(
                self.interval,
                DatebookInterval::Weekly | DatebookInterval::Monthly
            ),
        }
    }
}

impl Default for DatebookConfig {
    fn default() -> Self {
        Self {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            empty_sections: EmptySections::default(),
            browse_offset: 0,
        }
    }
}

// ── Section ─────────────────────────────────────────────────────────

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryFn {
    #[default]
    None,
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl SummaryFn {
    /// Cycle to the next summary function variant.
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Sum,
            Self::Sum => Self::Avg,
            Self::Avg => Self::Min,
            Self::Min => Self::Max,
            Self::Max => Self::Count,
            Self::Count => Self::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sum => "sum",
            Self::Avg => "avg",
            Self::Min => "min",
            Self::Max => "max",
            Self::Count => "count",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    #[serde(default)]
    pub kind: ColumnKind,
    pub heading: CategoryId,
    pub width: u16,
    #[serde(default)]
    pub summary_fn: Option<SummaryFn>,
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

    /// Format criteria as a human-readable trigger description.
    ///
    /// `resolve` maps CategoryId → display name.
    ///
    /// Examples:
    ///   - "Waiting/Blocked" (single OR)
    ///   - "Bug + Core" (multiple AND)
    ///   - "Work, not in Delegated" (AND + NOT)
    ///   - "Mom or Dad" (multiple OR)
    pub fn format_trigger(&self, resolve: &impl Fn(CategoryId) -> String) -> String {
        let and_names: Vec<String> = self.and_category_ids().map(&resolve).collect();
        let not_names: Vec<String> = self.not_category_ids().map(&resolve).collect();
        let or_names: Vec<String> = self.or_category_ids().map(&resolve).collect();

        let mut parts = Vec::new();

        if !and_names.is_empty() {
            parts.push(and_names.join(" + "));
        }
        if !or_names.is_empty() {
            parts.push(or_names.join(" or "));
        }
        if !not_names.is_empty() {
            let not_part = format!("not in {}", not_names.join(", "));
            parts.push(not_part);
        }

        if parts.is_empty() {
            "(empty)".to_string()
        } else {
            parts.join(", ")
        }
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
    pub when_date: Option<jiff::civil::DateTime>,
    pub done_date: Option<jiff::civil::DateTime>,
    pub is_done: bool,
    pub assignments_json: String,
    pub deleted_at: Timestamp,
    pub deleted_by: String,
}

impl Item {
    pub fn new(text: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: Uuid::new_v4(),
            text,
            note: None,
            note_file: None,
            created_at: now,
            modified_at: now,
            when_date: None,
            done_date: None,
            is_done: false,
            assignments: HashMap::new(),
            recurrence_rule: None,
            recurrence_series_id: None,
            recurrence_parent_item_id: None,
        }
    }
}

impl Category {
    pub fn new(name: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: Uuid::new_v4(),
            name,
            parent: None,
            children: Vec::new(),
            is_exclusive: false,
            is_actionable: true,
            enable_implicit_string: true,
            enable_semantic_classification: true,
            match_category_name: true,
            also_match: Vec::new(),
            note: None,
            created_at: now,
            modified_at: now,
            condition_match_mode: ConditionMatchMode::Any,
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
            category_aliases: BTreeMap::new(),
            item_column_label: None,
            board_display_mode: BoardDisplayMode::SingleLine,
            section_flow: SectionFlow::Vertical,
            hide_dependent_items: false,
            datebook_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Column, ItemLinkKind, RecurrenceFrequency, RecurrenceRule, SectionFlow, SummaryFn, View,
    };
    use serde_json::Value;
    use uuid::Uuid;

    fn dt(year: i16, month: i8, day: i8, hour: i8, min: i8) -> jiff::civil::DateTime {
        jiff::civil::date(year, month, day).at(hour, min, 0, 0)
    }

    #[test]
    fn recurrence_daily_advances_one_day() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 1, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 4, 2, 9, 0));
    }

    #[test]
    fn recurrence_daily_interval_3() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 3,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 1, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 4, 4, 9, 0));
    }

    #[test]
    fn recurrence_weekly_same_weekday() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        // 2026-04-01 is a Wednesday
        let anchor = dt(2026, 4, 1, 9, 0);
        let next = rule.next_date(anchor);
        assert_eq!(next, dt(2026, 4, 8, 9, 0));
        assert_eq!(next.date().weekday(), jiff::civil::Weekday::Wednesday);
    }

    #[test]
    fn recurrence_weekly_specific_weekday() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 1,
            weekday: Some(5), // Friday
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        // 2026-04-01 is a Wednesday; next week's Friday = 2026-04-10
        let anchor = dt(2026, 4, 1, 9, 0);
        let next = rule.next_date(anchor);
        assert_eq!(next.date().weekday(), jiff::civil::Weekday::Friday);
        assert_eq!(next, dt(2026, 4, 10, 9, 0));
    }

    #[test]
    fn recurrence_every_2_weeks() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 2,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 1, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 4, 15, 9, 0));
    }

    #[test]
    fn recurrence_monthly_same_day() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 15, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 5, 15, 9, 0));
    }

    #[test]
    fn recurrence_monthly_specific_day() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(1),
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 15, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 5, 1, 9, 0));
    }

    #[test]
    fn recurrence_monthly_31st_clamps_to_feb_28() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(31),
            month: None,
            weekdays_only: None,
        };
        // Jan 31 → Feb: should clamp to 28 (non-leap 2026)
        let anchor = dt(2026, 1, 31, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 2, 28, 9, 0));
    }

    #[test]
    fn recurrence_monthly_31st_clamps_to_feb_29_leap() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(31),
            month: None,
            weekdays_only: None,
        };
        // Jan 31, 2028 (leap year) → Feb: should clamp to 29
        let anchor = dt(2028, 1, 31, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2028, 2, 29, 9, 0));
    }

    #[test]
    fn recurrence_monthly_31st_clamps_to_apr_30() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(31),
            month: None,
            weekdays_only: None,
        };
        // Mar 31 → Apr: should clamp to 30
        let anchor = dt(2026, 3, 31, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 4, 30, 9, 0));
    }

    #[test]
    fn recurrence_monthly_crosses_year_boundary() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(15),
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 12, 15, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2027, 1, 15, 9, 0));
    }

    #[test]
    fn recurrence_every_3_months() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 3,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 1, 15, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2026, 4, 15, 9, 0));
    }

    #[test]
    fn recurrence_yearly() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Yearly,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let anchor = dt(2026, 4, 1, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2027, 4, 1, 9, 0));
    }

    #[test]
    fn recurrence_yearly_feb_29_clamps_in_non_leap() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Yearly,
            interval: 1,
            weekday: None,
            day_of_month: Some(29),
            month: Some(2),
            weekdays_only: None,
        };
        // 2028 is a leap year, 2029 is not
        let anchor = dt(2028, 2, 29, 9, 0);
        assert_eq!(rule.next_date(anchor), dt(2029, 2, 28, 9, 0));
    }

    #[test]
    fn recurrence_rule_serde_roundtrip() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 2,
            weekday: Some(5),
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: RecurrenceRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, parsed);
    }

    #[test]
    fn recurrence_rule_display() {
        let daily = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        assert_eq!(daily.display(), "daily");

        let weekly_fri = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 1,
            weekday: Some(5),
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        assert_eq!(weekly_fri.display(), "every Friday");

        let monthly_15th = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 1,
            weekday: None,
            day_of_month: Some(15),
            month: None,
            weekdays_only: None,
        };
        assert_eq!(monthly_15th.display(), "monthly on the 15th");

        let every_2_weeks = RecurrenceRule {
            frequency: RecurrenceFrequency::Weekly,
            interval: 2,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: None,
        };
        assert_eq!(every_2_weeks.display(), "every 2 weeks");
    }

    #[test]
    fn item_link_kind_serde_names_are_stable() {
        let depends_on = serde_json::to_string(&ItemLinkKind::DependsOn).unwrap();
        let related = serde_json::to_string(&ItemLinkKind::Related).unwrap();

        assert_eq!(depends_on, "\"depends-on\"");
        assert_eq!(related, "\"related\"");

        let parsed_depends_on: ItemLinkKind = serde_json::from_str("\"depends-on\"").unwrap();
        let parsed_related: ItemLinkKind = serde_json::from_str("\"related\"").unwrap();

        assert_eq!(parsed_depends_on, ItemLinkKind::DependsOn);
        assert_eq!(parsed_related, ItemLinkKind::Related);
    }

    #[test]
    fn view_serde_defaults_missing_category_aliases_to_empty() {
        let view = View::new("Example".to_string());
        let mut json: Value = serde_json::to_value(view).expect("serialize view");
        json.as_object_mut()
            .expect("view object")
            .remove("category_aliases");
        json.as_object_mut()
            .expect("view object")
            .remove("hide_dependent_items");
        json.as_object_mut()
            .expect("view object")
            .remove("section_flow");

        let parsed: View = serde_json::from_value(json).expect("deserialize view");
        assert!(
            parsed.category_aliases.is_empty(),
            "missing category_aliases should default to empty"
        );
        assert!(
            !parsed.hide_dependent_items,
            "missing hide_dependent_items should default to false"
        );
        assert_eq!(
            parsed.section_flow,
            SectionFlow::Vertical,
            "missing section_flow should default to vertical"
        );
    }

    #[test]
    fn view_serde_roundtrips_category_aliases() {
        let mut view = View::new("Aliases".to_string());
        let category_id = Uuid::new_v4();
        view.category_aliases
            .insert(category_id, "Customer".to_string());

        let json = serde_json::to_string(&view).expect("serialize view");
        let parsed: View = serde_json::from_str(&json).expect("deserialize view");
        assert_eq!(
            parsed
                .category_aliases
                .get(&category_id)
                .map(String::as_str),
            Some("Customer")
        );
    }

    #[test]
    fn column_serde_defaults_missing_summary_fn_to_none() {
        let mut json = serde_json::json!({
            "kind": "Standard",
            "heading": Uuid::new_v4(),
            "width": 24
        });
        json.as_object_mut()
            .expect("column object")
            .remove("summary_fn");

        let parsed: Column = serde_json::from_value(json).expect("deserialize column");
        assert_eq!(parsed.summary_fn, None);
    }

    #[test]
    fn summary_fn_serde_names_are_stable() {
        assert_eq!(serde_json::to_string(&SummaryFn::Sum).unwrap(), "\"sum\"");
        assert_eq!(serde_json::to_string(&SummaryFn::Avg).unwrap(), "\"avg\"");
        assert_eq!(serde_json::to_string(&SummaryFn::Min).unwrap(), "\"min\"");
        assert_eq!(serde_json::to_string(&SummaryFn::Max).unwrap(), "\"max\"");
        assert_eq!(
            serde_json::to_string(&SummaryFn::Count).unwrap(),
            "\"count\""
        );
        assert_eq!(serde_json::to_string(&SummaryFn::None).unwrap(), "\"none\"");
    }
}
