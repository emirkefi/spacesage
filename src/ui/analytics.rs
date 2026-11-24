//! Side panel with details, category breakdown and heatmap.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::app::*;
use crate::model::{DiskNode, format_count, format_size};
use super::render_inline_bar;

pub(crate) fn render_analytics(
    f: &mut Frame,
    app: &App,
    selected_idx: Option<usize>,
    current_total: u64,
    items: &[&DiskNode],
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(8)])
        .split(area);

    let selected_node = selected_idx
        .and_then(|idx| items.get(idx).copied())
        .or_else(|| items.first().copied());

    let mut details_lines = Vec::new();
    if let Some(target) = selected_node {
        let pct = if current_total > 0 {
            (target.size as f64 / current_total as f64) * 100.0
        } else {
            0.0
        };
        let cat = target.category();
        let cat_color = cat.color(&app.theme);

        details_lines.push(Line::from(vec![
            Span::styled(" Name:      ", Style::default().fg(app.theme.text_dim)),
            Span::styled(&target.name, Style::default().fg(cat_color).bold()),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled(" Category:  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(cat.label(), Style::default().fg(cat_color)),
            if target.access_denied {
                Span::styled(
                    " (Permission Denied - needs sudo)",
                    Style::default().fg(app.theme.gauge_crit).bold(),
                )
            } else if target.is_symlink {
                Span::styled(" (Symlink)", Style::default().fg(app.theme.text_dim))
            } else {
                Span::raw("")
            },
        ]));
        details_lines.push(Line::from(vec![
            Span::styled(" Full Path: ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                target.path.to_string_lossy(),
                Style::default().fg(app.theme.text),
            ),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled(" Disk Size: ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format_size(target.size),
                Style::default().fg(app.theme.gauge_ok).bold(),
            ),
            Span::styled(
                format!("  ({:.2}% of scope)", pct),
                Style::default().fg(app.theme.accent),
            ),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled(" Contains:  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!(
                    "{} subdirs, {} files",
                    format_count(target.dir_count),
                    format_count(target.file_count)
                ),
                Style::default().fg(app.theme.text),
            ),
        ]));
    } else {
        details_lines.push(Line::from(Span::styled(
            " No item selected",
            Style::default().fg(app.theme.text_dim),
        )));
    }

    let nerd = app.icon_mode.use_nerd_icons();
    let inspect_icon = if nerd { " 󰍉 " } else { " " };
    let detail_widget = Paragraph::new(details_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.border))
            .title(Span::styled(
                format!("{inspect_icon}Target Inspection "),
                Style::default().fg(app.theme.gauge_warn).bold(),
            )),
    );
    f.render_widget(detail_widget, chunks[0]);

    let mut treemap_lines = Vec::new();
    treemap_lines.push(Line::from(vec![
        Span::styled(
            " Top Space Consumers ",
            Style::default().fg(app.theme.accent).bold(),
        ),
        Span::styled(
            "([t] 2D treemap · [f] top files):",
            Style::default().fg(app.theme.text_dim),
        ),
    ]));
    treemap_lines.push(Line::from(""));

    let top_items: Vec<&DiskNode> = items.iter().copied().take(6).collect();

    if top_items.is_empty() {
        treemap_lines.push(Line::from(Span::styled(
            "  No space consumers in current view",
            Style::default().fg(app.theme.text_dim),
        )));
    } else {
        for (i, child) in top_items.iter().enumerate() {
            let color = child.category().color(&app.theme);
            let pct = if current_total > 0 {
                (child.size as f64 / current_total as f64) * 100.0
            } else {
                0.0
            };

            let name_disp = if child.name.chars().count() > 18 {
                let s: String = child.name.chars().take(17).collect();
                format!("{s}…")
            } else {
                child.name.clone()
            };

            treemap_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:02} ", i + 1),
                    Style::default().fg(app.theme.text_dim),
                ),
                Span::styled(
                    format!("{:<18}", name_disp),
                    Style::default().fg(color).bold(),
                ),
                Span::styled(
                    format!(" {:>9} ", format_size(child.size)),
                    Style::default().fg(app.theme.text),
                ),
                Span::styled(format!(" {:>5.1}%", pct), Style::default().fg(color)),
            ]));

            let bar_width = (area.width as usize).saturating_sub(8).clamp(8, 60);
            let mut bar_line = vec![Span::raw("    ")];
            bar_line.extend(render_inline_bar(pct, bar_width, color, app.theme.bar_empty));
            treemap_lines.push(Line::from(bar_line));
            treemap_lines.push(Line::from(""));
        }
    }

    let heatmap_icon = if nerd { " 󰗡 " } else { " " };
    let treemap_widget = Paragraph::new(treemap_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.secondary))
                .title(Span::styled(
                    format!("{heatmap_icon}Space Allocation & Heatmap "),
                    Style::default().fg(app.theme.secondary).bold(),
                )),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(treemap_widget, chunks[1]);
}
