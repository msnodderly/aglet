use ratatui::style::{Color, Modifier, Style};

// ── Focus & structure ─────────────────────────────────────────────────────────

/// Border / highlight color for the pane that currently owns keyboard focus.
pub(crate) const COLOR_FOCUS: Color = Color::Cyan;

/// Border color for visible but unfocused panes.
pub(crate) const COLOR_IDLE: Color = Color::DarkGray;

/// Border / highlight color when the app is in a special or modal mode
/// (e.g. classification).
pub(crate) const COLOR_SPECIAL_MODE: Color = Color::Magenta;

// ── Selection & editing ───────────────────────────────────────────────────────

/// Background for the selected row in a focused list.
/// Pair with `Color::Black` foreground.
pub(crate) const COLOR_SELECTED_BG: Color = Color::Cyan;

/// Background for the cursor row in an unfocused list.
pub(crate) const COLOR_CURSOR_BG: Color = Color::DarkGray;

/// Background for a text area / input box that is actively being edited.
/// Dark amber — warm "you are typing here" signal without harsh contrast.
/// Pair with `Color::White` foreground + `BOLD`.
pub(crate) const COLOR_EDIT_BG: Color = Color::Rgb(65, 55, 10);

/// Background for a single table-cell cursor (high-contrast, small target).
/// Pair with `Color::Black` foreground.
pub(crate) const COLOR_CELL_CURSOR: Color = Color::Yellow;

/// Background for a marked / flagged board row.
pub(crate) const COLOR_MARKED_BG: Color = Color::Rgb(40, 70, 120);

// ── Status & outcome ──────────────────────────────────────────────────────────

/// Foreground for something not yet saved, resolved, or confirmed.
pub(crate) const COLOR_PENDING: Color = Color::Yellow;

/// Foreground for accepted / saved / successful state.
pub(crate) const COLOR_SUCCESS: Color = Color::LightGreen;

/// Foreground for rejected / failed / destructive state.
pub(crate) const COLOR_ERROR: Color = Color::LightRed;

// ── Content hierarchy ─────────────────────────────────────────────────────────

/// Primary readable content.
pub(crate) const COLOR_TEXT_PRIMARY: Color = Color::White;

/// Labels, column headers, UI chrome, key-binding hints.
pub(crate) const COLOR_TEXT_SECONDARY: Color = Color::Rgb(170, 178, 198);

/// Placeholders, hints, disabled fields.
pub(crate) const COLOR_TEXT_MUTED: Color = Color::Rgb(140, 140, 140);

// ── Style constructors ────────────────────────────────────────────────────────

pub(crate) fn style_focus_border() -> Style {
    Style::default().fg(COLOR_FOCUS)
}

pub(crate) fn style_idle_border() -> Style {
    Style::default().fg(COLOR_IDLE)
}

pub(crate) fn style_selected_row() -> Style {
    Style::default().fg(Color::Black).bg(COLOR_SELECTED_BG)
}

pub(crate) fn style_cursor_row() -> Style {
    Style::default().bg(COLOR_CURSOR_BG)
}

pub(crate) fn style_edit_area() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(COLOR_EDIT_BG)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn style_cell_cursor() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(COLOR_CELL_CURSOR)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn style_pending() -> Style {
    Style::default().fg(COLOR_PENDING)
}

pub(crate) fn style_success() -> Style {
    Style::default().fg(COLOR_SUCCESS)
}

pub(crate) fn style_error() -> Style {
    Style::default().fg(COLOR_ERROR)
}

pub(crate) fn style_text_secondary() -> Style {
    Style::default().fg(COLOR_TEXT_SECONDARY)
}

pub(crate) fn style_text_muted() -> Style {
    Style::default().fg(COLOR_TEXT_MUTED)
}
