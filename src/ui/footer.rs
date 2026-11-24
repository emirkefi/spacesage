//! One-line, context-aware footer with key hints.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::format_size;
use crate::ui::*;

pub(crate) fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    let (marked_count, marked_bytes) = app.marked_summary();
    if marked_count > 0 {
        spans.push(Span::styled(
            format!(
                " ● {marked_count} selected · {} ",
                format_size(marked_bytes)
            ),
            Style::default()
                .fg(app.theme.match_fg)
                .bg(app.theme.gauge_warn)
                .bold(),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(app.theme.text_dim)));
    }

    if app.modal.is_some() {
        spans.push(Span::styled(
            "[Esc/n]",
            Style::default().fg(app.theme.accent).bold(),
        ));
        spans.push(Span::styled(
            " close/cancel · ",
            Style::default().fg(app.theme.text_dim),
        ));
        spans.push(Span::styled(
            "[Enter/y]",
            Style::default().fg(app.theme.gauge_ok).bold(),
        ));
        spans.push(Span::styled(
            " confirm",
            Style::default().fg(app.theme.text_dim),
        ));
    } else if app.is_filtering {
        spans.push(Span::styled(
            "[Type]",
            Style::default().fg(app.theme.accent).bold(),
        ));
        spans.push(Span::styled(
            " to filter · ",
            Style::default().fg(app.theme.text_dim),
        ));
        spans.push(Span::styled(
            "[↑/↓]",
            Style::default().fg(app.theme.accent).bold(),
        ));
        spans.push(Span::styled(
            " select · ",
            Style::default().fg(app.theme.text_dim),
        ));
        spans.push(Span::styled(
            "[Enter]",
            Style::default().fg(app.theme.gauge_ok).bold(),
        ));
        spans.push(Span::styled(
            " apply · ",
            Style::default().fg(app.theme.text_dim),
        ));
        spans.push(Span::styled(
            "[Esc]",
            Style::default().fg(app.theme.gauge_crit).bold(),
        ));
        spans.push(Span::styled(
            " clear filter",
            Style::default().fg(app.theme.text_dim),
        ));
    } else {
        let hints: &[(&str, &str)] = match app.view_mode {
            ViewMode::TopFiles => &[
                ("j/k", "move"),
                ("Enter", "locate"),
                ("Space", "mark"),
                ("d", "trash"),
                ("D", "perm-del"),
                ("y", "copy"),
                ("o", "open"),
                ("f/Esc", "list"),
                ("?", "help"),
            ],
            ViewMode::Treemap => &[
                ("j/k", "move"),
                ("l/Enter", "open"),
                ("h", "back"),
                ("s/S", "sort"),
                ("d", "trash"),
                ("/", "search"),
                ("t/Esc", "list view"),
                ("?", "help"),
            ],
            ViewMode::List => &[
                ("j/k", "move"),
                ("l", "open"),
                ("h", "back"),
                ("Space", "mark"),
                ("d", "trash"),
                ("D", "delete"),
                ("s/S", "sort"),
                ("t", "treemap"),
                ("f", "top files"),
                ("/", "search"),
                ("?", "help"),
            ],
        };

        for (i, (key, desc)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", Style::default().fg(app.theme.text_dim)));
            }
            spans.push(Span::styled(
                format!("[{key}]"),
                Style::default().fg(app.theme.accent).bold(),
            ));
            spans.push(Span::styled(
                format!(" {desc}"),
                Style::default().fg(app.theme.text_dim),
            ));
        }
    }

    let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    f.render_widget(footer, area);
}
