//! The keyboard-shortcut overlay opened with `?`. Its only state is the
//! scroll offset, carried in `Mode::HelpPanel { scroll }`.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::keymap::{self, KeyBinding, NORMAL_KEYMAP};
use crate::App;

/// Apply a key to the panel. Returns true when the panel should close.
pub(crate) fn handle_key(scroll: &mut usize, key: KeyEvent) -> bool {
    // Upper bound on scrolling; `render` clamps further to the active layout
    // (two-column mode needs less).
    let max_scroll = keymap::help_panel_content_line_count().saturating_sub(1);
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q') => return true,
        KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1).min(max_scroll),
        KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
        KeyCode::PageDown => *scroll = scroll.saturating_add(10).min(max_scroll),
        KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
        KeyCode::Home => *scroll = 0,
        KeyCode::End => *scroll = max_scroll,
        _ => {}
    }
    false
}

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, scroll: usize) {
    if area.width < 8 || area.height < 8 {
        return;
    }

    let header = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let help_entry = |key: &str, desc: &str, gutter: usize| -> Line<'static> {
        // Pad by character count, not byte length — unicode arrows in key
        // labels must not break the description column.
        let pad = gutter.saturating_sub(key.chars().count());
        Line::from(vec![
            Span::raw("  "),
            Span::styled(key.to_string(), key_style),
            Span::raw(" ".repeat(pad)),
            Span::raw(desc.to_string()),
        ])
    };

    // One block of lines per help section (header + entries); blocks are
    // packed into one or two columns below.
    let mut blocks: Vec<Vec<Line<'static>>> = Vec::new();
    for section in keymap::HelpSection::ALL {
        let entries: Vec<&KeyBinding> = NORMAL_KEYMAP
            .iter()
            .filter(|binding| binding.section == section && !binding.desc.is_empty())
            .collect();
        if entries.is_empty() {
            continue;
        }
        // Per-section key gutter: aligned within the section, and one
        // long key label cannot push every description off-panel.
        let gutter = entries
            .iter()
            .map(|binding| binding.keys.chars().count())
            .max()
            .unwrap_or(10)
            .saturating_add(2);
        let mut block_lines = vec![Line::from(Span::styled(section.help_label(), header))];
        for binding in entries {
            block_lines.push(help_entry(binding.keys, binding.desc, gutter));
        }
        blocks.push(block_lines);
    }

    let block = Block::default()
        .title("Keyboard Shortcuts")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.height < 2 || inner.width < 4 {
        return;
    }

    // Last inner row is a fixed hint/indicator line; content scrolls above.
    let content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height - 1,
    };
    let hint_area = Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    };

    let join_blocks = |blocks: &[Vec<Line<'static>>]| -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        for block_lines in blocks {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.extend(block_lines.iter().cloned());
        }
        lines
    };

    // Wide panels render two columns; content comfortably fits side by side.
    let two_columns = inner.width >= 120;
    let columns: Vec<Vec<Line<'static>>> = if two_columns {
        let total: usize = blocks.iter().map(|b| b.len() + 1).sum();
        let mut split_at = blocks.len();
        let mut running = 0usize;
        for (idx, block_lines) in blocks.iter().enumerate() {
            running += block_lines.len() + 1;
            if running >= total / 2 {
                split_at = idx + 1;
                break;
            }
        }
        vec![
            join_blocks(&blocks[..split_at]),
            join_blocks(&blocks[split_at..]),
        ]
    } else {
        vec![join_blocks(&blocks)]
    };

    let viewport = content_area.height as usize;
    let tallest = columns.iter().map(Vec::len).max().unwrap_or(0);
    let max_scroll = tallest.saturating_sub(viewport);
    let scroll = scroll.min(max_scroll);

    let column_areas: Vec<Rect> = if two_columns {
        let half = content_area.width / 2;
        vec![
            Rect {
                width: half.saturating_sub(2),
                ..content_area
            },
            Rect {
                x: content_area.x + half,
                width: content_area.width - half,
                ..content_area
            },
        ]
    } else {
        vec![content_area]
    };
    for (lines, column_area) in columns.into_iter().zip(column_areas) {
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll as u16, 0)),
            column_area,
        );
    }
    if max_scroll > 0 {
        App::render_vertical_scrollbar(frame, content_area, tallest, scroll);
    }

    let more_indicator = if scroll < max_scroll {
        "\u{2026} more \u{2193}   "
    } else {
        ""
    };
    let hint_text = if max_scroll > 0 {
        format!("{more_indicator}j/k scroll   Esc / Enter / ? to close")
    } else {
        "Esc / Enter / ? to close".to_string()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {hint_text}"),
            Style::default().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}
