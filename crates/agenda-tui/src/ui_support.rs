use super::*;

pub(super) fn generated_section(
    on_remove_unassign: HashSet<CategoryId>,
    on_insert_assign: HashSet<CategoryId>,
) -> Section {
    Section {
        title: "generated".to_string(),
        criteria: Query::default(),
        columns: Vec::new(),
        item_column_index: 0,
        on_insert_assign,
        on_remove_unassign,
        show_children: false,
        board_display_mode_override: None,
    }
}

pub(super) fn next_index(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    if delta > 0 {
        (current + delta as usize) % len
    } else {
        let amount = (-delta) as usize % len;
        (current + len - amount) % len
    }
}

pub(super) fn next_index_clamped(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => current
            .saturating_add(delta as usize)
            .min(len.saturating_sub(1)),
        std::cmp::Ordering::Less => current.saturating_sub((-delta) as usize),
        std::cmp::Ordering::Equal => current.min(len.saturating_sub(1)),
    }
}

pub(super) fn when_bucket_options() -> &'static [WhenBucket] {
    &[
        WhenBucket::Overdue,
        WhenBucket::Today,
        WhenBucket::Tomorrow,
        WhenBucket::ThisWeek,
        WhenBucket::NextWeek,
        WhenBucket::ThisMonth,
        WhenBucket::Future,
        WhenBucket::NoDate,
    ]
}

pub(super) fn when_bucket_label(bucket: WhenBucket) -> &'static str {
    match bucket {
        WhenBucket::Overdue => "Overdue",
        WhenBucket::Today => "Today",
        WhenBucket::Tomorrow => "Tomorrow",
        WhenBucket::ThisWeek => "ThisWeek",
        WhenBucket::NextWeek => "NextWeek",
        WhenBucket::ThisMonth => "ThisMonth",
        WhenBucket::Future => "Future",
        WhenBucket::NoDate => "NoDate",
    }
}

pub(super) fn bucket_target_set_mut(
    view: &mut View,
    target: BucketEditTarget,
) -> Option<&mut HashSet<WhenBucket>> {
    match target {
        BucketEditTarget::ViewVirtualInclude => Some(&mut view.criteria.virtual_include),
        BucketEditTarget::ViewVirtualExclude => Some(&mut view.criteria.virtual_exclude),
    }
}

pub(super) fn list_scroll_for_selected_line(area: Rect, selected_line: Option<usize>) -> u16 {
    let Some(selected_line) = selected_line else {
        return 0;
    };
    let viewport_rows = area.height.saturating_sub(2) as usize;
    if viewport_rows == 0 {
        return 0;
    }
    selected_line
        .saturating_add(1)
        .saturating_sub(viewport_rows)
        .min(u16::MAX as usize) as u16
}

pub(super) fn should_render_unmatched_lane(unmatched_items: &[Item]) -> bool {
    !unmatched_items.is_empty()
}

pub(super) fn item_text_matches(item: &Item, needle_lower_ascii: &str) -> bool {
    if item.text.to_ascii_lowercase().contains(needle_lower_ascii) {
        return true;
    }

    item.note
        .as_ref()
        .map(|note| note.to_ascii_lowercase().contains(needle_lower_ascii))
        .unwrap_or(false)
}

pub(super) fn category_name_map(categories: &[Category]) -> HashMap<CategoryId, String> {
    categories
        .iter()
        .map(|category| (category.id, category.name.clone()))
        .collect()
}

pub(super) fn item_assignment_labels(
    item: &Item,
    category_names: &HashMap<CategoryId, String>,
) -> Vec<String> {
    let mut labels: Vec<String> = item
        .assignments
        .keys()
        .map(|category_id| {
            category_names
                .get(category_id)
                .cloned()
                .unwrap_or_else(|| category_id.to_string())
        })
        .collect();
    labels.sort_by_key(|name| name.to_ascii_lowercase());
    labels
}

pub(super) const BOARD_MULTI_CATEGORY_LINE_CAP: usize = 8;

#[allow(dead_code)]
pub(super) fn format_category_values_single_line(labels: &[String]) -> String {
    if labels.is_empty() {
        "-".to_string()
    } else {
        labels.join(", ")
    }
}

pub(super) fn format_category_values_multi_line(
    labels: &[String],
    max_lines: usize,
) -> Vec<String> {
    if labels.is_empty() {
        return vec!["-".to_string()];
    }
    if max_lines == 0 {
        return vec![];
    }
    if labels.len() <= max_lines {
        return labels.to_vec();
    }
    if max_lines == 1 {
        return vec![format!("+{} more", labels.len())];
    }
    let mut lines: Vec<String> = labels.iter().take(max_lines - 1).cloned().collect();
    lines.push(format!("+{} more", labels.len() - (max_lines - 1)));
    lines
}

pub(super) fn wrap_text_for_board_cell(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let current_len = current.chars().count();
        let word_len = word.chars().count();
        if current.is_empty() {
            if word_len <= width {
                current.push_str(word);
            } else {
                let mut chunk = String::new();
                for ch in word.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() >= width {
                        lines.push(chunk.clone());
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    current.push_str(&chunk);
                }
            }
            continue;
        }
        if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = String::new();
            if word_len <= width {
                current.push_str(word);
            } else {
                let mut chunk = String::new();
                for ch in word.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() >= width {
                        lines.push(chunk.clone());
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    current.push_str(&chunk);
                }
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoardColumnWidths {
    pub(super) marker: usize,
    pub(super) note: usize,
    pub(super) when: usize,
    pub(super) item: usize,
    pub(super) categories: usize,
}

pub(super) const BOARD_ROW_MARKER_WIDTH: usize = 2;
pub(super) const BOARD_NOTE_MARKER_WIDTH: usize = 2;
pub(super) const NOTE_MARKER_SYMBOL: &str = "♪";
pub(super) const BOARD_WHEN_TARGET_WIDTH: usize = 19;
pub(super) const BOARD_WHEN_MIN_WIDTH: usize = 10;
pub(super) const BOARD_ITEM_MIN_WIDTH: usize = 12;
pub(super) const BOARD_CATEGORY_TARGET_WIDTH: usize = 34;
pub(super) const BOARD_CATEGORY_MIN_WIDTH: usize = 14;
pub(super) const BOARD_DYNAMIC_ITEM_MIN_WIDTH: usize = 12;
pub(super) const BOARD_TRUNCATION_SUFFIX: &str = "...";

#[derive(Clone, Debug)]
pub(super) struct BoardColumnLayout {
    pub(super) marker: usize,
    pub(super) note: usize,
    pub(super) item: usize,
    pub(super) item_label: String,
    pub(super) columns: Vec<BoardColumnSpec>,
}

/// Returns true if the category is eligible to be used as a board/section column heading.
///
/// Rules:
/// - "Entry" is never valid (reserved for item text).
/// - "When" is valid only if top-level.
/// - Numeric categories are always valid (they are leaf column heads).
/// - All other categories must be non-leaf (have children).
pub(super) fn is_valid_column_heading(category: &Category) -> bool {
    if category.name.eq_ignore_ascii_case("Entry") {
        return false;
    }
    if category.name.eq_ignore_ascii_case("When") {
        return category.parent.is_none();
    }
    if category.value_kind == CategoryValueKind::Numeric {
        return true;
    }
    !category.children.is_empty()
}

/// Determine the ColumnKind for a given heading category.
pub(super) fn column_kind_for_heading(category: &Category) -> ColumnKind {
    if category.name.eq_ignore_ascii_case("When") {
        ColumnKind::When
    } else {
        ColumnKind::Standard
    }
}

#[derive(Clone, Debug)]
pub(super) struct BoardColumnSpec {
    pub(super) label: String,
    pub(super) width: usize,
    pub(super) kind: ColumnKind,
    pub(super) child_ids: Vec<CategoryId>,
    pub(super) heading_id: CategoryId,
    pub(super) heading_value_kind: CategoryValueKind,
}

pub(super) fn compute_board_layout(
    view_columns: &[Column],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
    item_label: &str,
    slot_width: u16,
) -> BoardColumnLayout {
    let total = slot_width as usize;
    let marker = BOARD_ROW_MARKER_WIDTH.min(total);
    let note = BOARD_NOTE_MARKER_WIDTH.min(total.saturating_sub(marker));
    let available = total.saturating_sub(marker + note);

    let cat_by_id: HashMap<CategoryId, &Category> = categories.iter().map(|c| (c.id, c)).collect();

    let mut configured_widths: Vec<usize> = view_columns
        .iter()
        .map(|col| (col.width as usize).max(8))
        .collect();
    let configured_total: usize = configured_widths.iter().sum();
    let mut item_width = available.saturating_sub(configured_total);

    if item_width < BOARD_DYNAMIC_ITEM_MIN_WIDTH && !configured_widths.is_empty() {
        let deficit = BOARD_DYNAMIC_ITEM_MIN_WIDTH.saturating_sub(item_width);
        let shrinkable: usize = configured_widths.iter().map(|w| w.saturating_sub(8)).sum();
        let actual_shrink = deficit.min(shrinkable);
        if actual_shrink > 0 {
            let mut remaining = actual_shrink;
            for w in configured_widths.iter_mut().rev() {
                let can_take = w.saturating_sub(8);
                let take = can_take.min(remaining);
                *w -= take;
                remaining -= take;
                if remaining == 0 {
                    break;
                }
            }
        }
        let new_total: usize = configured_widths.iter().sum();
        item_width = available.saturating_sub(new_total);
    }

    item_width = item_width.max(if available > 0 { 1 } else { 0 });

    // Redistribute excess Item width to columns when Item is disproportionately wide.
    // Cap Item at 50% of available space; distribute surplus evenly across columns.
    if !configured_widths.is_empty() && available > 0 {
        let max_item = available / 2;
        if item_width > max_item {
            let surplus = item_width - max_item;
            let per_col = surplus / configured_widths.len();
            let mut leftover = surplus % configured_widths.len();
            for w in configured_widths.iter_mut() {
                *w += per_col;
                if leftover > 0 {
                    *w += 1;
                    leftover -= 1;
                }
            }
            item_width = max_item;
        }
    }

    let columns: Vec<BoardColumnSpec> = view_columns
        .iter()
        .zip(configured_widths.iter())
        .map(|(col, &width)| {
            let label = category_names
                .get(&col.heading)
                .cloned()
                .unwrap_or_else(|| "(deleted)".to_string());
            let child_ids = match col.kind {
                ColumnKind::Standard => cat_by_id
                    .get(&col.heading)
                    .map(|c| c.children.clone())
                    .unwrap_or_default(),
                ColumnKind::When => Vec::new(),
            };
            let heading_value_kind = cat_by_id
                .get(&col.heading)
                .map(|c| c.value_kind)
                .unwrap_or_default();
            BoardColumnSpec {
                label,
                width,
                kind: col.kind,
                child_ids,
                heading_id: col.heading,
                heading_value_kind,
            }
        })
        .collect();

    BoardColumnLayout {
        marker,
        note,
        item: item_width,
        item_label: item_label.to_string(),
        columns,
    }
}

pub(super) fn standard_column_value(
    item: &Item,
    child_ids: &[CategoryId],
    category_names: &HashMap<CategoryId, String>,
) -> String {
    let mut matches: Vec<String> = child_ids
        .iter()
        .filter(|cid| item.assignments.contains_key(cid))
        .map(|cid| {
            category_names
                .get(cid)
                .cloned()
                .unwrap_or_else(|| cid.to_string())
        })
        .collect();
    if matches.is_empty() {
        return "\u{2013}".to_string();
    }
    matches.sort_by_key(|n| n.to_ascii_lowercase());
    matches.join(", ")
}

/// Format a numeric value for display in a board cell.
///
/// Returns the en-dash placeholder when value is None.
pub(super) fn format_numeric_cell(
    value: Option<rust_decimal::Decimal>,
    format: Option<&NumericFormat>,
) -> String {
    let Some(v) = value else {
        return "\u{2013}".to_string();
    };
    let fmt = format.cloned().unwrap_or_default();
    let rounded = v.round_dp(fmt.decimal_places as u32);
    let raw = format!("{:.prec$}", rounded, prec = fmt.decimal_places as usize);

    let formatted = if fmt.use_thousands_separator {
        add_thousands_separator(&raw)
    } else {
        raw
    };

    match &fmt.currency_symbol {
        Some(sym) => format!("{sym}{formatted}"),
        None => formatted,
    }
}

fn add_thousands_separator(s: &str) -> String {
    let (integer_part, decimal_part) = match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos..])),
        None => (s, None),
    };
    let negative = integer_part.starts_with('-');
    let digits = if negative {
        &integer_part[1..]
    } else {
        integer_part
    };
    let mut result = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let reversed: String = result.chars().rev().collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&reversed);
    if let Some(dec) = decimal_part {
        out.push_str(dec);
    }
    out
}

/// Aggregate numeric values across items for a given column.
#[derive(Default, Clone, Debug)]
pub(super) struct NumericAggregate {
    pub(super) count: usize,
    pub(super) sum: rust_decimal::Decimal,
}

impl NumericAggregate {
    pub(super) fn push(&mut self, v: rust_decimal::Decimal) {
        self.count += 1;
        self.sum += v;
    }

    pub(super) fn avg(&self) -> Option<rust_decimal::Decimal> {
        if self.count > 0 {
            Some(self.sum / rust_decimal::Decimal::from(self.count as u32))
        } else {
            None
        }
    }
}

/// Compute per-column aggregates for numeric columns from a list of items.
pub(super) fn compute_column_aggregates(
    items: &[&Item],
    columns: &[BoardColumnSpec],
) -> Vec<Option<NumericAggregate>> {
    columns
        .iter()
        .map(|col| {
            if col.heading_value_kind != CategoryValueKind::Numeric {
                return None;
            }
            let mut agg = NumericAggregate::default();
            for item in items {
                if let Some(val) = item
                    .assignments
                    .get(&col.heading_id)
                    .and_then(|a| a.numeric_value)
                {
                    agg.push(val);
                }
            }
            Some(agg)
        })
        .collect()
}

/// Right-align text within a cell by left-padding with spaces.
/// Truncates from the left if the text exceeds the width.
pub(super) fn right_pad_cell(text: &str, width: usize) -> String {
    let char_count = text.chars().count();
    if char_count >= width {
        // Truncate: keep the rightmost `width` characters
        text.chars().skip(char_count - width).collect()
    } else {
        format!("{:>width$}", text, width = width)
    }
}

pub(super) fn has_note_text(note: Option<&str>) -> bool {
    note.map(|text| !text.trim().is_empty()).unwrap_or(false)
}

pub(super) fn with_note_marker(label: String, has_note: bool) -> String {
    if has_note {
        format!("{label} {NOTE_MARKER_SYMBOL}")
    } else {
        label
    }
}

pub(super) fn board_item_label(item: &Item) -> String {
    if item.is_done {
        format!("[done] {}", item.text)
    } else {
        item.text.clone()
    }
}

pub(super) fn board_column_widths(slot_width: u16) -> BoardColumnWidths {
    let total = slot_width as usize;
    let marker = BOARD_ROW_MARKER_WIDTH.min(total);
    let note = BOARD_NOTE_MARKER_WIDTH.min(total.saturating_sub(marker));
    let available = total.saturating_sub(marker + note);

    if available == 0 {
        return BoardColumnWidths {
            marker,
            note,
            when: 0,
            item: 0,
            categories: 0,
        };
    }

    let mut when = BOARD_WHEN_TARGET_WIDTH.min(available);
    let mut categories = BOARD_CATEGORY_TARGET_WIDTH.min(available.saturating_sub(when));
    let mut item = available.saturating_sub(when + categories);

    let min_item = BOARD_ITEM_MIN_WIDTH.min(available);
    if item < min_item {
        let needed = min_item - item;
        let min_categories = BOARD_CATEGORY_MIN_WIDTH.min(categories);
        let category_shift = needed.min(categories.saturating_sub(min_categories));
        categories -= category_shift;
        item += category_shift;

        let needed = min_item.saturating_sub(item);
        let min_when = BOARD_WHEN_MIN_WIDTH.min(when);
        let when_shift = needed.min(when.saturating_sub(min_when));
        when -= when_shift;
        item += when_shift;
    }

    if item == 0 && available > 0 {
        if categories > 0 {
            categories -= 1;
            item += 1;
        } else if when > 0 {
            when -= 1;
            item += 1;
        }
    }

    let used = when + item + categories;
    if used < available {
        item += available - used;
    }

    BoardColumnWidths {
        marker,
        note,
        when,
        item,
        categories,
    }
}

pub(super) fn truncate_board_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width <= BOARD_TRUNCATION_SUFFIX.len() {
        return ".".repeat(width);
    }
    let keep = width - BOARD_TRUNCATION_SUFFIX.len();
    let prefix: String = text.chars().take(keep).collect();
    format!("{prefix}{BOARD_TRUNCATION_SUFFIX}")
}

pub(super) fn selected_row_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
}

pub(super) fn selected_board_row_style() -> Style {
    Style::default().bg(Color::DarkGray)
}

pub(super) fn focused_cell_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn build_category_rows(categories: &[Category]) -> Vec<CategoryListRow> {
    let parent_by_id: HashMap<CategoryId, Option<CategoryId>> = categories
        .iter()
        .map(|category| (category.id, category.parent))
        .collect();

    categories
        .iter()
        .map(|category| CategoryListRow {
            id: category.id,
            name: category.name.clone(),
            depth: category_depth(category.id, &parent_by_id, categories.len()),
            is_reserved: is_reserved_category_name(&category.name),
            has_note: has_note_text(category.note.as_deref()),
            is_exclusive: category.is_exclusive,
            is_actionable: category.is_actionable,
            enable_implicit_string: category.enable_implicit_string,
            value_kind: category.value_kind,
        })
        .collect()
}

pub(super) fn build_reparent_options(
    category_rows: &[CategoryListRow],
    categories: &[Category],
    selected_category_id: CategoryId,
) -> Vec<ReparentOptionRow> {
    let descendants = descendant_category_ids(categories, selected_category_id);
    let mut options = vec![ReparentOptionRow {
        parent_id: None,
        label: "(root)".to_string(),
    }];

    for row in category_rows {
        if row.id == selected_category_id {
            continue;
        }
        if descendants.contains(&row.id) {
            continue;
        }
        options.push(ReparentOptionRow {
            parent_id: Some(row.id),
            label: format!("{}{}", "  ".repeat(row.depth), row.name),
        });
    }

    options
}

pub(super) fn descendant_category_ids(
    categories: &[Category],
    root_id: CategoryId,
) -> HashSet<CategoryId> {
    let children_by_parent: HashMap<CategoryId, Vec<CategoryId>> = categories
        .iter()
        .filter_map(|category| category.parent.map(|parent| (parent, category.id)))
        .fold(HashMap::new(), |mut acc, (parent, child)| {
            acc.entry(parent).or_default().push(child);
            acc
        });

    let mut seen = HashSet::new();
    let mut stack = vec![root_id];
    while let Some(current) = stack.pop() {
        let Some(children) = children_by_parent.get(&current) else {
            continue;
        };
        for child in children {
            if seen.insert(*child) {
                stack.push(*child);
            }
        }
    }

    seen
}

pub(super) fn category_depth(
    category_id: CategoryId,
    parent_by_id: &HashMap<CategoryId, Option<CategoryId>>,
    max_depth: usize,
) -> usize {
    let mut depth = 0usize;
    let mut cursor = parent_by_id.get(&category_id).copied().flatten();

    while let Some(parent_id) = cursor {
        depth += 1;
        if depth > max_depth {
            break;
        }
        cursor = parent_by_id.get(&parent_id).copied().flatten();
    }

    depth
}

pub(super) fn is_reserved_category_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("When")
        || name.eq_ignore_ascii_case("Entry")
        || name.eq_ignore_ascii_case("Done")
}

pub(super) fn filter_category_ids_by_query(
    scope_ids: &[CategoryId],
    categories: &[Category],
    query: &str,
    empty_query_returns_all: bool,
    exclude_when: bool,
) -> Vec<CategoryId> {
    let trimmed = query.trim();
    if trimmed.is_empty() && !empty_query_returns_all {
        return Vec::new();
    }
    let query_lower = trimmed.to_ascii_lowercase();
    scope_ids
        .iter()
        .filter(|id| {
            categories
                .iter()
                .find(|c| c.id == **id)
                .map(|c| {
                    if exclude_when && c.name.eq_ignore_ascii_case("When") {
                        return false;
                    }
                    trimmed.is_empty() || c.name.to_ascii_lowercase().contains(&query_lower)
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

pub(super) fn exact_category_name_match_in_scope(
    scope_ids: &[CategoryId],
    categories: &[Category],
    name: &str,
) -> Option<CategoryId> {
    let target_name = name.trim();
    if target_name.is_empty() {
        return None;
    }
    scope_ids.iter().copied().find(|id| {
        categories
            .iter()
            .find(|c| c.id == *id)
            .map(|c| c.name.eq_ignore_ascii_case(target_name))
            .unwrap_or(false)
    })
}

#[cfg(test)]
pub(super) fn filter_child_categories(
    child_ids: &[CategoryId],
    categories: &[Category],
    query: &str,
) -> Vec<CategoryId> {
    filter_category_ids_by_query(child_ids, categories, query, false, true)
}

pub(super) fn first_non_reserved_category_index(category_rows: &[CategoryListRow]) -> usize {
    category_rows
        .iter()
        .position(|row| !row.is_reserved)
        .unwrap_or(0)
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

pub(super) fn input_panel_popup_area(area: Rect) -> Rect {
    centered_rect(84, 70, area)
}

pub(super) struct InputPanelPopupRegions {
    pub(super) heading: Rect,
    pub(super) text: Rect,
    /// Present for AddItem / EditItem; absent for NameInput.
    pub(super) note: Option<Rect>,
    pub(super) note_inner: Option<Rect>,
    pub(super) categories: Option<Rect>,
    /// Region for numeric value rows (only when there are numeric values).
    pub(super) numeric_values: Option<Rect>,
    pub(super) preview: Option<Rect>,
    pub(super) buttons: Rect,
    pub(super) help: Rect,
}

pub(super) fn input_panel_popup_regions(
    area: Rect,
    kind: crate::input_panel::InputPanelKind,
    numeric_count: usize,
) -> Option<InputPanelPopupRegions> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if inner.width == 0 {
        return None;
    }

    use crate::input_panel::InputPanelKind;
    match kind {
        InputPanelKind::NameInput => {
            // heading + text + buttons + help = 4 lines minimum
            if inner.height < 4 {
                return None;
            }
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(inner);
            Some(InputPanelPopupRegions {
                heading: chunks[0],
                text: chunks[1],
                note: None,
                note_inner: None,
                categories: None,
                numeric_values: None,
                preview: None,
                buttons: chunks[3],
                help: chunks[4],
            })
        }
        InputPanelKind::AddItem | InputPanelKind::EditItem => {
            let numeric_rows = numeric_count as u16;
            // heading + text + note(min 3) + categories + numeric + preview + buttons + help
            let min_height = 9 + numeric_rows;
            if inner.height < min_height.min(9) {
                // Require at least 9 rows even with numeric values
                return None;
            }
            let mut constraints = vec![
                Constraint::Length(1), // heading
                Constraint::Length(1), // text
                Constraint::Min(3),    // note
                Constraint::Length(1), // categories
            ];
            if numeric_rows > 0 {
                constraints.push(Constraint::Length(numeric_rows)); // numeric values
            }
            constraints.push(Constraint::Length(1)); // preview
            constraints.push(Constraint::Length(1)); // buttons
            constraints.push(Constraint::Length(1)); // help

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(inner);
            let note = chunks[2];
            let note_inner = Rect {
                x: note.x.saturating_add(1),
                y: note.y.saturating_add(1),
                width: note.width.saturating_sub(2),
                height: note.height.saturating_sub(2),
            };
            let (numeric_idx, preview_idx, buttons_idx, help_idx) = if numeric_rows > 0 {
                (Some(4), 5, 6, 7)
            } else {
                (None, 4, 5, 6)
            };
            Some(InputPanelPopupRegions {
                heading: chunks[0],
                text: chunks[1],
                note: Some(note),
                note_inner: Some(note_inner),
                categories: Some(chunks[3]),
                numeric_values: numeric_idx.map(|i| chunks[i]),
                preview: Some(chunks[preview_idx]),
                buttons: chunks[buttons_idx],
                help: chunks[help_idx],
            })
        }
    }
}

pub(super) fn add_capture_status_message(
    parsed_when: Option<NaiveDateTime>,
    unknown_hashtags: &[String],
) -> String {
    let warning = if unknown_hashtags.is_empty() {
        String::new()
    } else {
        format!(" | warning unknown_hashtags={}", unknown_hashtags.join(","))
    };
    match parsed_when {
        Some(when) => format!("Item added (parsed when: {when}{warning})"),
        None => format!("Item added{warning}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenda_core::model::Category;
    use chrono::Utc;

    fn make_category(name: &str) -> Category {
        Category {
            id: CategoryId::new_v4(),
            name: name.to_string(),
            parent: None,
            children: Vec::new(),
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
            note: None,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            conditions: Vec::new(),
            actions: Vec::new(),
            value_kind: Default::default(),
            numeric_format: None,
        }
    }

    #[test]
    fn filter_basic_substring_match() {
        let high = make_category("High");
        let medium = make_category("Medium");
        let low = make_category("Low");
        let categories = vec![high.clone(), medium.clone(), low.clone()];
        let child_ids: Vec<CategoryId> = categories.iter().map(|c| c.id).collect();

        let result = filter_child_categories(&child_ids, &categories, "hig");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], high.id);
    }

    #[test]
    fn filter_case_insensitive() {
        let high = make_category("High");
        let categories = vec![high.clone()];
        let child_ids: Vec<CategoryId> = vec![high.id];

        let result = filter_child_categories(&child_ids, &categories, "HIG");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], high.id);
    }

    #[test]
    fn filter_empty_query_returns_empty() {
        let high = make_category("High");
        let categories = vec![high.clone()];
        let child_ids: Vec<CategoryId> = vec![high.id];

        let result = filter_child_categories(&child_ids, &categories, "");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_excludes_when() {
        let when_cat = make_category("When");
        let high = make_category("High");
        let categories = vec![when_cat.clone(), high.clone()];
        let child_ids: Vec<CategoryId> = vec![when_cat.id, high.id];

        let result = filter_child_categories(&child_ids, &categories, "h");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], high.id);
    }

    #[test]
    fn filter_includes_done() {
        let done = make_category("Done");
        let categories = vec![done.clone()];
        let child_ids: Vec<CategoryId> = vec![done.id];

        let result = filter_child_categories(&child_ids, &categories, "done");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], done.id);
    }

    #[test]
    fn filter_multiple_matches() {
        let high = make_category("High");
        let medium = make_category("Medium");
        let low = make_category("Low");
        let categories = vec![high.clone(), medium.clone(), low.clone()];
        let child_ids: Vec<CategoryId> = categories.iter().map(|c| c.id).collect();

        let result = filter_child_categories(&child_ids, &categories, "m");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], medium.id);
    }

    #[test]
    fn format_category_values_multi_line_caps_with_overflow_summary() {
        let labels = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];
        let lines = format_category_values_multi_line(&labels, 3);
        assert_eq!(lines, vec!["A", "B", "+2 more"]);
    }

    #[test]
    fn wrap_text_for_board_cell_wraps_on_word_boundaries() {
        let lines = wrap_text_for_board_cell("alpha beta gamma", 6);
        assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn is_valid_column_heading_accepts_numeric_leaf() {
        let mut cat = make_category("Cost");
        cat.value_kind = CategoryValueKind::Numeric;
        assert!(is_valid_column_heading(&cat));
    }

    #[test]
    fn is_valid_column_heading_rejects_childless_tag() {
        let cat = make_category("Orphan");
        assert!(!is_valid_column_heading(&cat));
    }

    #[test]
    fn is_valid_column_heading_accepts_non_leaf_tag() {
        let mut cat = make_category("Status");
        cat.children = vec![CategoryId::new_v4()];
        assert!(is_valid_column_heading(&cat));
    }

    #[test]
    fn is_valid_column_heading_rejects_entry() {
        let mut cat = make_category("Entry");
        cat.children = vec![CategoryId::new_v4()];
        assert!(!is_valid_column_heading(&cat));
    }

    #[test]
    fn is_valid_column_heading_accepts_toplevel_when() {
        let cat = make_category("When");
        assert!(is_valid_column_heading(&cat));
    }

    #[test]
    fn is_valid_column_heading_rejects_non_toplevel_when() {
        let mut cat = make_category("When");
        cat.parent = Some(CategoryId::new_v4());
        assert!(!is_valid_column_heading(&cat));
    }

    // --- format_numeric_cell tests ---

    #[test]
    fn format_numeric_cell_none_returns_dash() {
        assert_eq!(format_numeric_cell(None, None), "\u{2013}");
    }

    #[test]
    fn format_numeric_cell_default_format() {
        use rust_decimal::Decimal;
        let result = format_numeric_cell(Some(Decimal::new(24596, 2)), None);
        assert_eq!(result, "245.96");
    }

    #[test]
    fn format_numeric_cell_with_currency_and_thousands() {
        use rust_decimal::Decimal;
        let fmt = NumericFormat {
            decimal_places: 2,
            currency_symbol: Some("$".to_string()),
            use_thousands_separator: true,
        };
        let result = format_numeric_cell(Some(Decimal::new(123456789, 2)), Some(&fmt));
        assert_eq!(result, "$1,234,567.89");
    }

    #[test]
    fn format_numeric_cell_rounds_to_decimal_places() {
        use rust_decimal::Decimal;
        let fmt = NumericFormat {
            decimal_places: 0,
            currency_symbol: None,
            use_thousands_separator: false,
        };
        let result = format_numeric_cell(Some(Decimal::new(2567, 2)), Some(&fmt));
        assert_eq!(result, "26");
    }

    #[test]
    fn format_numeric_cell_integer_shows_decimals() {
        use rust_decimal::Decimal;
        let result = format_numeric_cell(Some(Decimal::new(42, 0)), None);
        assert_eq!(result, "42.00");
    }

    // --- right_pad_cell tests ---

    #[test]
    fn right_pad_cell_pads_short_text() {
        assert_eq!(right_pad_cell("42", 8), "      42");
    }

    #[test]
    fn right_pad_cell_truncates_long_text() {
        assert_eq!(right_pad_cell("$1,234,567.89", 10), "234,567.89");
    }

    // --- NumericAggregate tests ---

    #[test]
    fn numeric_aggregate_sum_and_avg() {
        use rust_decimal::Decimal;
        let mut agg = NumericAggregate::default();
        agg.push(Decimal::new(100, 0));
        agg.push(Decimal::new(200, 0));
        agg.push(Decimal::new(300, 0));
        assert_eq!(agg.count, 3);
        assert_eq!(agg.sum, Decimal::new(600, 0));
        assert_eq!(agg.avg(), Some(Decimal::new(200, 0)));
    }

    #[test]
    fn numeric_aggregate_empty_avg_is_none() {
        let agg = NumericAggregate::default();
        assert_eq!(agg.avg(), None);
    }

    // --- compute_column_aggregates tests ---

    #[test]
    fn compute_column_aggregates_ignores_tag_columns() {
        use rust_decimal::Decimal;
        let mut item = Item::new("test".to_string());
        let cat_id = CategoryId::new_v4();
        item.assignments.insert(
            cat_id,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: Utc::now(),
                sticky: true,
                origin: None,
                numeric_value: Some(Decimal::new(100, 0)),
            },
        );
        let col = BoardColumnSpec {
            label: "Status".to_string(),
            width: 12,
            kind: ColumnKind::Standard,
            child_ids: vec![],
            heading_id: cat_id,
            heading_value_kind: CategoryValueKind::Tag,
        };
        let items: Vec<&Item> = vec![&item];
        let result = compute_column_aggregates(&items, &[col]);
        assert!(result[0].is_none());
    }

    #[test]
    fn compute_column_aggregates_sums_numeric_columns() {
        use rust_decimal::Decimal;
        let cat_id = CategoryId::new_v4();
        let mut item1 = Item::new("a".to_string());
        item1.assignments.insert(
            cat_id,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: Utc::now(),
                sticky: true,
                origin: None,
                numeric_value: Some(Decimal::new(100, 0)),
            },
        );
        let mut item2 = Item::new("b".to_string());
        item2.assignments.insert(
            cat_id,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: Utc::now(),
                sticky: true,
                origin: None,
                numeric_value: Some(Decimal::new(250, 0)),
            },
        );
        let item3 = Item::new("c".to_string()); // no assignment

        let col = BoardColumnSpec {
            label: "Cost".to_string(),
            width: 12,
            kind: ColumnKind::Standard,
            child_ids: vec![],
            heading_id: cat_id,
            heading_value_kind: CategoryValueKind::Numeric,
        };
        let items: Vec<&Item> = vec![&item1, &item2, &item3];
        let result = compute_column_aggregates(&items, &[col]);
        let agg = result[0].as_ref().unwrap();
        assert_eq!(agg.count, 2);
        assert_eq!(agg.sum, Decimal::new(350, 0));
        assert_eq!(agg.avg(), Some(Decimal::new(175, 0)));
    }
}
