use super::*;

pub(super) fn generated_section(
    on_remove_unassign: HashSet<CategoryId>,
    on_insert_assign: HashSet<CategoryId>,
) -> Section {
    Section {
        title: "generated".to_string(),
        criteria: Query::default(),
        on_insert_assign,
        on_remove_unassign,
        show_children: false,
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
    if delta > 0 {
        current
            .saturating_add(delta as usize)
            .min(len.saturating_sub(1))
    } else if delta < 0 {
        current.saturating_sub((-delta) as usize)
    } else {
        current.min(len.saturating_sub(1))
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

pub(super) fn category_target_is_section(target: CategoryEditTarget) -> bool {
    matches!(
        target,
        CategoryEditTarget::SectionCriteriaInclude
            | CategoryEditTarget::SectionCriteriaExclude
            | CategoryEditTarget::SectionOnInsertAssign
            | CategoryEditTarget::SectionOnRemoveUnassign
    )
}

pub(super) fn bucket_target_is_section(target: BucketEditTarget) -> bool {
    matches!(
        target,
        BucketEditTarget::SectionVirtualInclude | BucketEditTarget::SectionVirtualExclude
    )
}

pub(super) fn category_target_label(target: CategoryEditTarget) -> &'static str {
    match target {
        CategoryEditTarget::ViewInclude => "View include categories",
        CategoryEditTarget::ViewExclude => "View exclude categories",
        CategoryEditTarget::SectionCriteriaInclude => "Section include criteria",
        CategoryEditTarget::SectionCriteriaExclude => "Section exclude criteria",
        CategoryEditTarget::SectionOnInsertAssign => "Section on-insert assign",
        CategoryEditTarget::SectionOnRemoveUnassign => "Section on-remove unassign",
    }
}

pub(super) fn bucket_target_label(target: BucketEditTarget) -> &'static str {
    match target {
        BucketEditTarget::ViewVirtualInclude => "View virtual include buckets",
        BucketEditTarget::ViewVirtualExclude => "View virtual exclude buckets",
        BucketEditTarget::SectionVirtualInclude => "Section virtual include buckets",
        BucketEditTarget::SectionVirtualExclude => "Section virtual exclude buckets",
    }
}

pub(super) fn category_target_set_mut<'a>(
    view: &'a mut View,
    section_index: usize,
    target: CategoryEditTarget,
) -> Option<&'a mut HashSet<CategoryId>> {
    match target {
        CategoryEditTarget::ViewInclude => Some(&mut view.criteria.include),
        CategoryEditTarget::ViewExclude => Some(&mut view.criteria.exclude),
        CategoryEditTarget::SectionCriteriaInclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.include),
        CategoryEditTarget::SectionCriteriaExclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.exclude),
        CategoryEditTarget::SectionOnInsertAssign => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.on_insert_assign),
        CategoryEditTarget::SectionOnRemoveUnassign => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.on_remove_unassign),
    }
}

pub(super) fn bucket_target_set_mut<'a>(
    view: &'a mut View,
    section_index: usize,
    target: BucketEditTarget,
) -> Option<&'a mut HashSet<WhenBucket>> {
    match target {
        BucketEditTarget::ViewVirtualInclude => Some(&mut view.criteria.virtual_include),
        BucketEditTarget::ViewVirtualExclude => Some(&mut view.criteria.virtual_exclude),
        BucketEditTarget::SectionVirtualInclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.virtual_include),
        BucketEditTarget::SectionVirtualExclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.virtual_exclude),
    }
}

pub(super) fn category_target_contains(
    view: &View,
    section_index: usize,
    target: CategoryEditTarget,
    category_id: CategoryId,
) -> bool {
    match target {
        CategoryEditTarget::ViewInclude => view.criteria.include.contains(&category_id),
        CategoryEditTarget::ViewExclude => view.criteria.exclude.contains(&category_id),
        CategoryEditTarget::SectionCriteriaInclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.include.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionCriteriaExclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.exclude.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionOnInsertAssign => view
            .sections
            .get(section_index)
            .map(|section| section.on_insert_assign.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionOnRemoveUnassign => view
            .sections
            .get(section_index)
            .map(|section| section.on_remove_unassign.contains(&category_id))
            .unwrap_or(false),
    }
}

pub(super) fn bucket_target_contains(
    view: &View,
    section_index: usize,
    target: BucketEditTarget,
    bucket: WhenBucket,
) -> bool {
    match target {
        BucketEditTarget::ViewVirtualInclude => view.criteria.virtual_include.contains(&bucket),
        BucketEditTarget::ViewVirtualExclude => view.criteria.virtual_exclude.contains(&bucket),
        BucketEditTarget::SectionVirtualInclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.virtual_include.contains(&bucket))
            .unwrap_or(false),
        BucketEditTarget::SectionVirtualExclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.virtual_exclude.contains(&bucket))
            .unwrap_or(false),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoardColumnWidths {
    marker: usize,
    note: usize,
    when: usize,
    item: usize,
    categories: usize,
}

pub(super) const BOARD_ROW_MARKER_WIDTH: usize = 2;
pub(super) const BOARD_NOTE_MARKER_WIDTH: usize = 2;
pub(super) const BOARD_COLUMN_SEPARATOR: &str = " | ";
pub(super) const NOTE_MARKER_SYMBOL: &str = "♪";
pub(super) const BOARD_WHEN_TARGET_WIDTH: usize = 19;
pub(super) const BOARD_WHEN_MIN_WIDTH: usize = 10;
pub(super) const BOARD_ITEM_MIN_WIDTH: usize = 12;
pub(super) const BOARD_CATEGORY_TARGET_WIDTH: usize = 34;
pub(super) const BOARD_CATEGORY_MIN_WIDTH: usize = 14;
pub(super) const BOARD_TRUNCATION_SUFFIX: &str = "...";
pub(super) const BOARD_DYNAMIC_ITEM_MIN_WIDTH: usize = 12;

#[derive(Clone, Debug)]
pub(super) struct BoardColumnLayout {
    marker: usize,
    note: usize,
    item: usize,
    item_label: String,
    columns: Vec<BoardColumnSpec>,
}

#[derive(Clone, Debug)]
pub(super) struct BoardColumnSpec {
    label: String,
    width: usize,
    kind: ColumnKind,
    child_ids: Vec<CategoryId>,
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
    let sep_count = view_columns.len();
    let separator_total = BOARD_COLUMN_SEPARATOR.len() * sep_count;
    let available = total.saturating_sub(marker + note + separator_total);

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
            BoardColumnSpec {
                label,
                width,
                kind: col.kind,
                child_ids,
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

pub(super) fn board_dynamic_header(layout: &BoardColumnLayout) -> String {
    let mut out = " ".repeat(layout.marker);
    out.push_str(&" ".repeat(layout.note));
    out.push_str(&fit_board_cell(&layout.item_label, layout.item));
    for col in &layout.columns {
        out.push_str(BOARD_COLUMN_SEPARATOR);
        out.push_str(&fit_board_cell(&col.label, col.width));
    }
    out
}

pub(super) fn board_dynamic_row(
    is_selected: bool,
    item: &Item,
    layout: &BoardColumnLayout,
    category_names: &HashMap<CategoryId, String>,
) -> String {
    let mut out = board_row_marker(is_selected, layout.marker);
    out.push_str(&board_note_cell(
        has_note_text(item.note.as_deref()),
        layout.note,
    ));
    let item_text = board_item_label(item);
    out.push_str(&fit_board_cell(&item_text, layout.item));
    for col in &layout.columns {
        out.push_str(BOARD_COLUMN_SEPARATOR);
        let cell = match col.kind {
            ColumnKind::When => item
                .when_date
                .map(|dt| dt.to_string())
                .unwrap_or_else(|| "\u{2013}".to_string()),
            ColumnKind::Standard => standard_column_value(item, &col.child_ids, category_names),
        };
        out.push_str(&fit_board_cell(&cell, col.width));
    }
    out
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

pub(super) fn board_note_cell(has_note: bool, width: usize) -> String {
    if has_note {
        fit_board_cell(NOTE_MARKER_SYMBOL, width)
    } else {
        " ".repeat(width)
    }
}

pub(super) fn board_column_widths(slot_width: u16) -> BoardColumnWidths {
    let total = slot_width as usize;
    let marker = BOARD_ROW_MARKER_WIDTH.min(total);
    let note = BOARD_NOTE_MARKER_WIDTH.min(total.saturating_sub(marker));
    let separator_total = BOARD_COLUMN_SEPARATOR.len() * 2;
    let available = total.saturating_sub(marker + note + separator_total);

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

pub(super) fn fit_board_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= width {
        return format!("{text:<width$}");
    }
    if width <= BOARD_TRUNCATION_SUFFIX.len() {
        return ".".repeat(width);
    }
    let keep = width - BOARD_TRUNCATION_SUFFIX.len();
    let prefix: String = text.chars().take(keep).collect();
    format!("{prefix}{BOARD_TRUNCATION_SUFFIX}")
}

pub(super) fn board_row_marker(is_selected: bool, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if is_selected {
        let mut marker = ">".to_string();
        marker.push_str(&" ".repeat(width.saturating_sub(1)));
        marker
    } else {
        " ".repeat(width)
    }
}

pub(super) fn board_annotation_header(widths: BoardColumnWidths) -> String {
    format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(widths.marker),
        fit_board_cell("When", widths.when),
        BOARD_COLUMN_SEPARATOR,
        " ".repeat(widths.note),
        fit_board_cell("Item", widths.item),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell("All Categories", widths.categories),
    )
}

pub(super) fn board_item_row(
    is_selected: bool,
    when: &str,
    item: &str,
    categories: &str,
    has_note: bool,
    widths: BoardColumnWidths,
) -> String {
    format!(
        "{}{}{}{}{}{}{}",
        board_row_marker(is_selected, widths.marker),
        fit_board_cell(when, widths.when),
        BOARD_COLUMN_SEPARATOR,
        board_note_cell(has_note, widths.note),
        fit_board_cell(item, widths.item),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell(categories, widths.categories),
    )
}

pub(super) fn selected_row_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
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

pub(super) fn item_edit_popup_area(area: Rect) -> Rect {
    centered_rect(84, 70, area)
}

pub(super) fn category_config_popup_area(area: Rect) -> Rect {
    centered_rect(84, 76, area)
}

pub(super) struct ItemEditPopupRegions {
    pub(super) heading: Rect,
    pub(super) text: Rect,
    pub(super) note: Rect,
    pub(super) note_inner: Rect,
    pub(super) buttons: Rect,
    pub(super) help: Rect,
}

pub(super) struct CategoryConfigPopupRegions {
    pub(super) heading: Rect,
    pub(super) toggles: Rect,
    pub(super) note: Rect,
    pub(super) note_inner: Rect,
    pub(super) buttons: Rect,
    pub(super) help: Rect,
}

pub(super) fn item_edit_popup_regions(area: Rect) -> Option<ItemEditPopupRegions> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height < 5 {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let note = chunks[2];
    let note_inner = Rect {
        x: note.x.saturating_add(1),
        y: note.y.saturating_add(1),
        width: note.width.saturating_sub(2),
        height: note.height.saturating_sub(2),
    };
    Some(ItemEditPopupRegions {
        heading: chunks[0],
        text: chunks[1],
        note,
        note_inner,
        buttons: chunks[3],
        help: chunks[4],
    })
}

pub(super) fn category_config_popup_regions(area: Rect) -> Option<CategoryConfigPopupRegions> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height < 5 {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let note = chunks[2];
    let note_inner = Rect {
        x: note.x.saturating_add(1),
        y: note.y.saturating_add(1),
        width: note.width.saturating_sub(2),
        height: note.height.saturating_sub(2),
    };
    Some(CategoryConfigPopupRegions {
        heading: chunks[0],
        toggles: chunks[1],
        note,
        note_inner,
        buttons: chunks[3],
        help: chunks[4],
    })
}

pub(super) fn note_cursor_line_col(note: &str, cursor_chars: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for c in note.chars().take(cursor_chars) {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(super) fn note_line_start_chars(note: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let mut char_index = 0usize;
    for c in note.chars() {
        char_index += 1;
        if c == '\n' {
            starts.push(char_index);
        }
    }
    starts
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
