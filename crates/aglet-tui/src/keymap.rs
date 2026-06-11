//! Single source of truth for TUI key documentation.
//!
//! The help panel, the Normal-mode footer hint row, and the README keybinding
//! cheat sheet all render from `NORMAL_KEYMAP`; modal footer hint rows render
//! from the per-mode tables below. Adding or changing a binding's
//! documentation should happen here, in one place (UX audit P2-2).

use crate::App;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HelpSection {
    CurrentItem,
    Selection,
    Navigation,
    Search,
    Columns,
    Views,
    Datebook,
    Global,
}

impl HelpSection {
    pub(crate) const ALL: [HelpSection; 8] = [
        HelpSection::CurrentItem,
        HelpSection::Selection,
        HelpSection::Navigation,
        HelpSection::Search,
        HelpSection::Columns,
        HelpSection::Views,
        HelpSection::Datebook,
        HelpSection::Global,
    ];

    /// Section header as rendered in the help panel.
    pub(crate) fn help_label(self) -> &'static str {
        match self {
            HelpSection::CurrentItem => "CURRENT ITEM",
            HelpSection::Selection => "SELECTION",
            HelpSection::Navigation => "NAVIGATION",
            HelpSection::Search => "SEARCH",
            HelpSection::Columns => "COLUMNS",
            HelpSection::Views => "VIEWS",
            HelpSection::Datebook => "DATEBOOK VIEWS",
            HelpSection::Global => "GLOBAL",
        }
    }

    /// "Area" column value in the README cheat sheet.
    #[cfg(test)]
    pub(crate) fn readme_label(self) -> &'static str {
        match self {
            HelpSection::CurrentItem => "Items",
            HelpSection::Selection => "Selection",
            HelpSection::Navigation => "Navigation",
            HelpSection::Search => "Search",
            HelpSection::Columns => "Columns",
            HelpSection::Views => "Views",
            HelpSection::Datebook => "Datebook",
            HelpSection::Global => "Global",
        }
    }
}

/// Footer-visibility gate for a binding. `Always` entries are stable hints;
/// the rest appear only while their state is active, so conditional hints
/// never evict unrelated ones.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyContext {
    Always,
    DatebookView,
    UndoAvailable,
    RedoAvailable,
    SectionFilterActive,
    NumericColumnFocused,
}

pub(crate) struct KeyBinding {
    /// Key label as shown in the help panel and README (may combine
    /// variants: `e / Enter`).
    pub(crate) keys: &'static str,
    /// Help panel / README description. Empty for footer-only rows.
    pub(crate) desc: &'static str,
    /// Footer hint as `(key, short description)`. `None` keeps the binding
    /// out of the footer (documented in help/README only).
    pub(crate) hint: Option<(&'static str, &'static str)>,
    /// Footer visibility gate. Does not affect help panel / README rows.
    pub(crate) context: KeyContext,
    pub(crate) section: HelpSection,
}

const fn doc(section: HelpSection, keys: &'static str, desc: &'static str) -> KeyBinding {
    KeyBinding {
        keys,
        desc,
        hint: None,
        context: KeyContext::Always,
        section,
    }
}

const fn doc_hint(
    section: HelpSection,
    keys: &'static str,
    desc: &'static str,
    hint: (&'static str, &'static str),
) -> KeyBinding {
    KeyBinding {
        keys,
        desc,
        hint: Some(hint),
        context: KeyContext::Always,
        section,
    }
}

const fn doc_hint_when(
    section: HelpSection,
    keys: &'static str,
    desc: &'static str,
    hint: (&'static str, &'static str),
    context: KeyContext,
) -> KeyBinding {
    KeyBinding {
        keys,
        desc,
        hint: Some(hint),
        context,
        section,
    }
}

/// Footer-only row: contributes a contextual hint but no help/README line
/// (used where one help row documents several footer hints).
const fn hint_only(
    section: HelpSection,
    hint: (&'static str, &'static str),
    context: KeyContext,
) -> KeyBinding {
    KeyBinding {
        keys: "",
        desc: "",
        hint: Some(hint),
        context,
        section,
    }
}

/// Normal-mode (board) keymap: powers the help panel, the Normal-mode footer
/// hint row (entries with hints, ≤10 stable + contextual), and the README
/// cheat sheet. Table order is footer order.
pub(crate) static NORMAL_KEYMAP: &[KeyBinding] = &[
    // ── Current item ──
    doc_hint(
        HelpSection::CurrentItem,
        "n",
        "Add a new item to the focused section",
        ("n", "new"),
    ),
    doc_hint(
        HelpSection::CurrentItem,
        "e / Enter",
        "Edit selected item; Enter adds when empty",
        ("e", "edit"),
    ),
    doc_hint(
        HelpSection::CurrentItem,
        "a",
        "Assign categories to current item or selection",
        ("a", "assign"),
    ),
    doc_hint(
        HelpSection::CurrentItem,
        "d / D",
        "Toggle done on selected item(s)",
        ("d", "done"),
    ),
    doc_hint(
        HelpSection::CurrentItem,
        "r / x",
        "Remove from view / delete selected item(s)",
        ("x", "delete"),
    ),
    doc(
        HelpSection::CurrentItem,
        "b / B",
        "Open dependency link wizard (blocked-by / blocks)",
    ),
    doc(
        HelpSection::CurrentItem,
        "=",
        "Classify selected item(s) now",
    ),
    doc_hint(
        HelpSection::CurrentItem,
        "p / i/o",
        "Toggle preview sidebar / cycle preview mode",
        ("p", "preview"),
    ),
    // ── Selection ──
    doc(
        HelpSection::Selection,
        "Space",
        "Toggle selection on current item",
    ),
    doc(
        HelpSection::Selection,
        "a/d/x/=",
        "Batch assign, done, delete, or classify",
    ),
    doc(
        HelpSection::Selection,
        "b / B",
        "Link selected items with a dependency",
    ),
    doc(HelpSection::Selection, "Esc", "Clear selection"),
    // ── Navigation ──
    doc(
        HelpSection::Navigation,
        "\u{2191}/k \u{2193}/j",
        "Move items; scroll preview when focused",
    ),
    doc(
        HelpSection::Navigation,
        "\u{2190}/h \u{2192}/l",
        "Move between sections or columns",
    ),
    doc(
        HelpSection::Navigation,
        "Tab/S-Tab",
        "Next / previous section; J/K jump section",
    ),
    doc(
        HelpSection::Navigation,
        "[/] or S-\u{2191}/S-\u{2193}",
        "Move item to previous / next section",
    ),
    doc(
        HelpSection::Navigation,
        "m / z",
        "Cycle lane layout / card size",
    ),
    // ── Search ──
    doc_hint(
        HelpSection::Search,
        "/ / g/",
        "Search focused section / all sections",
        ("/", "search"),
    ),
    doc_hint_when(
        HelpSection::Search,
        "Esc",
        "Clear active section filter",
        ("Esc", "clear search"),
        KeyContext::SectionFilterActive,
    ),
    // ── Columns ──
    doc(
        HelpSection::Columns,
        "Enter",
        "Edit column value (on a column cell)",
    ),
    doc(HelpSection::Columns, "+/-", "Add / remove board column"),
    doc(
        HelpSection::Columns,
        "H/L",
        "Move board column left / right",
    ),
    doc_hint_when(
        HelpSection::Columns,
        "f",
        "Cycle numeric column format",
        ("f", "col fmt"),
        KeyContext::NumericColumnFocused,
    ),
    doc_hint_when(
        HelpSection::Columns,
        "F",
        "Cycle numeric column summary (Sum/Avg/Min/Max)",
        ("F", "col summary"),
        KeyContext::NumericColumnFocused,
    ),
    doc(
        HelpSection::Columns,
        "s/S or </>",
        "Sort section by column (asc / desc)",
    ),
    // ── Views ──
    doc_hint(
        HelpSection::Views,
        "v/V/F8 ,/. ga",
        "Views, previous/next view, All Items",
        ("v", "views"),
    ),
    // ── Datebook ──
    doc(
        HelpSection::Datebook,
        "{/} (/) 0",
        "Step previous / next bucket; (/) step window; 0 today",
    ),
    hint_only(
        HelpSection::Datebook,
        ("{/}", "bucket"),
        KeyContext::DatebookView,
    ),
    hint_only(
        HelpSection::Datebook,
        ("(/)", "window"),
        KeyContext::DatebookView,
    ),
    hint_only(
        HelpSection::Datebook,
        ("0", "today"),
        KeyContext::DatebookView,
    ),
    // ── Global ──
    doc_hint(
        HelpSection::Global,
        "C",
        "Review pending classification suggestions",
        ("C", "review"),
    ),
    doc(HelpSection::Global, "g s / F10", "Open Global Settings"),
    doc(HelpSection::Global, "c / F9", "Open the category manager"),
    doc(
        HelpSection::Global,
        "u",
        "Toggle hide-dependent-items filter",
    ),
    doc(
        HelpSection::Global,
        "Ctrl-G",
        "Open $EDITOR for text/note (in item editor)",
    ),
    doc(HelpSection::Global, "Ctrl-L", "Reload data from disk"),
    doc_hint_when(
        HelpSection::Global,
        "Ctrl-Z",
        "Undo",
        ("Ctrl-Z", "undo"),
        KeyContext::UndoAvailable,
    ),
    doc_hint_when(
        HelpSection::Global,
        "C-S-Z",
        "Redo",
        ("Ctrl-Shift-Z", "redo"),
        KeyContext::RedoAvailable,
    ),
    doc(HelpSection::Global, "?", "Toggle this help panel"),
    doc_hint(HelpSection::Global, "q", "Quit", ("q", "quit")),
];

/// Normal-mode footer hints while a multi-selection is active (replaces the
/// base hint row; batch verbs first).
pub(crate) static NORMAL_SELECTION_HINTS: &[(&str, &str)] = &[
    ("Space", "toggle"),
    ("a", "assign"),
    ("b/B", "link"),
    ("x", "delete"),
    ("Esc", "clear sel"),
    ("/", "search"),
    ("J/K", "jump"),
    ("[/]", "move"),
    ("=", "classify"),
    ("C", "review"),
    ("g/", "global"),
    ("v", "views"),
    ("p", "preview"),
    ("q", "quit"),
];

// ── Per-mode footer hint tables (static modal modes) ──────────────────────
//
// Sub-state-dependent modes (CategoryManager, ViewEdit, InputPanel, …) keep
// their dispatch in `footer_hint_pairs` but select from these tables so the
// copy lives here.

pub(crate) static HELP_PANEL_HINTS: &[(&str, &str)] = &[
    ("j/k", "scroll"),
    ("PgUp/PgDn", "page"),
    ("Esc", "close"),
    ("?", "close"),
];

pub(crate) static SUGGESTION_REVIEW_HINTS: &[(&str, &str)] = &[
    ("Tab", "pane"),
    ("Space", "toggle"),
    ("Enter", "confirm"),
    ("s", "skip"),
    ("A", "accept all"),
    ("Esc", "close"),
];

pub(crate) static VIEW_PICKER_HINTS: &[(&str, &str)] = &[
    ("Enter", "switch"),
    ("n", "new"),
    ("c", "clone"),
    ("r", "rename"),
    ("e", "edit"),
    ("x", "delete"),
    ("Esc", "cancel"),
];

pub(crate) static CONFIRM_HINTS: &[(&str, &str)] = &[("y", "confirm"), ("Esc", "cancel")];

pub(crate) static DONE_BLOCKER_HINTS: &[(&str, &str)] = &[
    ("y", "remove links + done"),
    ("n", "done only"),
    ("Esc", "cancel"),
];

pub(crate) static DISCARD_CONFIRM_HINTS: &[(&str, &str)] = &[
    ("y", "save & close"),
    ("n", "discard"),
    ("Esc", "keep editing"),
];

pub(crate) static ITEM_ASSIGN_INPUT_HINTS: &[(&str, &str)] = &[
    ("Enter", "assign"),
    ("Tab/\u{2193}", "to list"),
    ("Esc", "cancel"),
];

pub(crate) static LINK_WIZARD_HINTS: &[(&str, &str)] = &[
    ("Tab", "focus"),
    ("Enter", "apply"),
    ("/", "target"),
    ("Esc", "cancel"),
];

pub(crate) static CATEGORY_DIRECT_EDIT_HINTS: &[(&str, &str)] = &[
    ("S", "save"),
    ("Tab", "focus"),
    ("Enter", "resolve"),
    ("x", "remove"),
    ("Esc", "cancel"),
];

pub(crate) static CATEGORY_COLUMN_PICKER_HINTS: &[(&str, &str)] =
    &[("Space", "toggle"), ("Enter", "save"), ("Esc", "cancel")];

pub(crate) static BOARD_ADD_COLUMN_HINTS: &[(&str, &str)] =
    &[("Enter", "insert"), ("Tab", "complete"), ("Esc", "cancel")];

pub(crate) static INSPECT_UNASSIGN_HINTS: &[(&str, &str)] =
    &[("Enter", "unassign"), ("Esc", "cancel")];

/// Category manager tree-focus hints. `J/K` reorder among siblings and
/// `H/L` change nesting level — keys must match `handle_category_manager_key`
/// (the old footer advertised `S-↑/↓`, which never moved anything).
pub(crate) static CATEGORY_MANAGER_TREE_HINTS: &[(&str, &str)] = &[
    ("n", "new sibling"),
    ("N", "new child"),
    ("r", "rename"),
    ("x", "delete"),
    ("J/K", "move"),
    ("H/L", "level"),
    ("/", "filter"),
    ("Tab", "details"),
    ("Esc", "close"),
];

/// Number of content lines the help panel renders in single-column layout
/// (section headers + entries + blank separators) — the upper bound for
/// scroll clamping in `handle_help_panel_key`.
pub(crate) fn help_panel_content_line_count() -> usize {
    let mut total = 0usize;
    let mut sections = 0usize;
    for section in HelpSection::ALL {
        let entries = NORMAL_KEYMAP
            .iter()
            .filter(|b| b.section == section && !b.desc.is_empty())
            .count();
        if entries > 0 {
            sections += 1;
            total += entries + 1;
        }
    }
    total + sections.saturating_sub(1)
}

/// Renders a hint table as `key:desc  key:desc …` for status-line copy, so
/// status rows and the footer hint row can never disagree about keys.
pub(crate) fn hint_summary(hints: &[(&str, &str)]) -> String {
    hints
        .iter()
        .map(|(key, desc)| format!("{key}:{desc}"))
        .collect::<Vec<_>>()
        .join("  ")
}

impl App {
    pub(crate) fn key_context_active(&self, context: KeyContext) -> bool {
        match context {
            KeyContext::Always => true,
            KeyContext::DatebookView => self
                .current_view()
                .is_some_and(|view| view.datebook_config.is_some()),
            KeyContext::UndoAvailable => self.undo.has_undo(),
            KeyContext::RedoAvailable => self.undo.has_redo(),
            KeyContext::SectionFilterActive => self.section_filters.iter().any(|f| f.is_some()),
            KeyContext::NumericColumnFocused => self.focused_numeric_board_column(),
        }
    }

    /// Normal-mode footer hint row, derived from `NORMAL_KEYMAP`.
    pub(crate) fn normal_footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        if self.selected_count() > 0 {
            return NORMAL_SELECTION_HINTS.to_vec();
        }
        NORMAL_KEYMAP
            .iter()
            .filter(|binding| self.key_context_active(binding.context))
            .filter_map(|binding| binding.hint)
            .collect()
    }
}

/// README cheat-sheet table generated from `NORMAL_KEYMAP`. The README test
/// fails when the committed table drifts from this output.
#[cfg(test)]
pub(crate) fn readme_cheatsheet_markdown() -> String {
    let mut out = String::from("| Area | Keys | Action |\n| --- | --- | --- |\n");
    for section in HelpSection::ALL {
        for binding in NORMAL_KEYMAP
            .iter()
            .filter(|b| b.section == section && !b.desc.is_empty())
        {
            out.push_str(&format!(
                "| {} | `{}` | {} |\n",
                section.readme_label(),
                binding.keys,
                binding.desc
            ));
        }
    }
    out
}

#[cfg(test)]
pub(crate) const README_KEYMAP_BEGIN: &str =
    "<!-- BEGIN GENERATED KEYMAP (regenerate: UPDATE_README=1 cargo test -p aglet-tui readme_keymap) -->";
#[cfg(test)]
pub(crate) const README_KEYMAP_END: &str = "<!-- END GENERATED KEYMAP -->";
