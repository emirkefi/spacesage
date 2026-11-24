//! The list view: file rows, inline bars and search highlighting.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::model::{DiskNode, format_size};
use crate::ui::*;

pub(crate) fn render_body(f: &mut Frame, app: &mut App, area: Rect) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    app.explorer_inner_area = Rect {
        x: body_chunks[0].x + 1,
        y: body_chunks[0].y + 1,
        width: body_chunks[0].width.saturating_sub(2),
        height: body_chunks[0].height.saturating_sub(2),
    };

    let current_node_info = app.current_node().map(|n| (n.size.max(1), n.access_denied));
    let (current_total_size, current_access_denied) = current_node_info.unwrap_or((1, false));

    let items = app.get_active_items();
    let selected_idx = app.list_state.selected();

    render_analytics(
        f,
        app,
        selected_idx,
        current_total_size,
        &items,
        body_chunks[1],
    );

    let list_widget = build_explorer_list(
        app,
        current_total_size,
        current_access_denied,
        &items,
        body_chunks[0],
    );
    drop(items);

    f.render_stateful_widget(list_widget, body_chunks[0], &mut app.list_state);
}

pub(crate) fn render_inline_bar(
    pct: f64,
    width: usize,
    color: Color,
    empty_color: Color,
) -> Vec<Span<'static>> {
    const BLOCKS: [&str; 8] = ["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    let clamped = pct.clamp(0.0, 100.0) / 100.0;
    let total_eighths = (clamped * (width * 8) as f64).round() as usize;
    let full_cells = total_eighths / 8;
    let rem = total_eighths % 8;

    let mut filled = "█".repeat(full_cells.min(width));
    let mut used_cells = full_cells.min(width);
    if used_cells < width && rem > 0 {
        filled.push_str(BLOCKS[rem - 1]);
        used_cells += 1;
    } else if used_cells == 0 && pct > 0.1 {
        filled.push('▏');
        used_cells = 1;
    }
    let empty = "░".repeat(width.saturating_sub(used_cells));

    vec![
        Span::styled(filled, Style::default().fg(color)),
        Span::styled(empty, Style::default().fg(empty_color)),
    ]
}

pub(crate) fn highlight_search_spans(
    text: &str,
    query: &str,
    normal_style: Style,
    match_style: Style,
) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    if let Some(byte_idx) = lower_text.find(&lower_query) {
        let end_idx = (byte_idx + lower_query.len()).min(text.len());
        if text.is_char_boundary(byte_idx) && text.is_char_boundary(end_idx) {
            let before = text[..byte_idx].to_string();
            let matched = text[byte_idx..end_idx].to_string();
            let after = text[end_idx..].to_string();
            let mut spans = Vec::with_capacity(3);
            if !before.is_empty() {
                spans.push(Span::styled(before, normal_style));
            }
            spans.push(Span::styled(matched, match_style));
            if !after.is_empty() {
                spans.push(Span::styled(after, normal_style));
            }
            return spans;
        }
    }
    vec![Span::styled(text.to_string(), normal_style)]
}

pub(crate) fn build_explorer_list(
    app: &App,
    current_total_size: u64,
    current_access_denied: bool,
    items: &[&DiskNode],
    area: Rect,
) -> List<'static> {
    let mut list_items = Vec::with_capacity(items.len().min(200));
    let bar_width = 10usize;
    let max_name_cols = (area.width as usize).saturating_sub(36).max(14);
    let top_denom = if app.view_mode == ViewMode::TopFiles {
        items.first().map(|n| n.size.max(1)).unwrap_or(1)
    } else {
        current_total_size.max(1)
    };

    for child in items {
        let cat = child.category();
        let color = cat.color(&app.theme);
        let pct = if top_denom > 0 {
            (child.size as f64 / top_denom as f64) * 100.0
        } else {
            0.0
        };
        let dir_share_pct = if current_total_size > 0 {
            (child.size as f64 / current_total_size as f64) * 100.0
        } else {
            0.0
        };

        let is_marked = app.marked_paths.contains(&child.path);
        let mark_span = if is_marked {
            Span::styled("● ", Style::default().fg(app.theme.gauge_warn).bold())
        } else {
            Span::raw("  ")
        };

        let icon = cat.icon(
            app.icon_mode,
            child.is_dir,
            child.is_symlink,
            child.access_denied,
        );

        let display_name = if app.view_mode == ViewMode::TopFiles {
            if let Ok(rel) = child.path.strip_prefix(&app.current_path) {
                rel.to_string_lossy().to_string()
            } else {
                child.name.clone()
            }
        } else {
            child.name.clone()
        };

        let char_count = display_name.chars().count();
        let name_trimmed = if char_count > max_name_cols {
            let truncated: String = display_name
                .chars()
                .take(max_name_cols.saturating_sub(1))
                .collect();
            format!("{truncated}…")
        } else {
            format!("{:<width$}", display_name, width = max_name_cols)
        };

        let name_style = if child.access_denied {
            Style::default().fg(app.theme.gauge_crit)
        } else if child.is_dir {
            Style::default().fg(app.theme.text).bold()
        } else {
            Style::default().fg(color)
        };
        let match_style = Style::default()
            .fg(app.theme.match_fg)
            .bg(app.theme.match_bg)
            .bold();

        let mut row_spans = Vec::with_capacity(12);
        row_spans.push(mark_span);
        row_spans.extend(render_inline_bar(
            pct,
            bar_width,
            color,
            app.theme.bar_empty,
        ));
        row_spans.push(Span::styled(
            format!(" {:>5.1}% ", dir_share_pct),
            Style::default().fg(app.theme.text_dim),
        ));
        row_spans.push(Span::styled(
            format!("{:>9}  ", format_size(child.size)),
            Style::default().fg(color).bold(),
        ));
        row_spans.push(Span::styled(icon, Style::default().fg(color)));
        row_spans.extend(highlight_search_spans(
            &name_trimmed,
            &app.filter_query,
            name_style,
            match_style,
        ));

        if child.access_denied {
            row_spans.push(Span::styled(
                " [needs sudo]",
                Style::default().fg(app.theme.gauge_crit).bold(),
            ));
        }

        list_items.push(ListItem::new(Line::from(row_spans)));
    }

    if list_items.is_empty() {
        let msg = if app.scanning {
            "  ⠋ Scanning directory in background..."
        } else if current_access_denied {
            "  ! Permission denied — re-run with sudo to inspect this folder"
        } else if !app.filter_query.is_empty() {
            "  No items matched search query (press Esc to clear)"
        } else {
            "  Empty folder (0 B) — press h or Backspace to go back"
        };
        let msg_color = if current_access_denied {
            app.theme.gauge_crit
        } else {
            app.theme.text_dim
        };
        list_items.push(ListItem::new(Line::from(Span::styled(
            msg,
            Style::default().fg(msg_color),
        ))));
    }

    let nerd = app.icon_mode.use_nerd_icons();
    let title = if app.view_mode == ViewMode::TopFiles {
        let icon = if nerd { " 󰈔 " } else { " " };
        format!(
            "{icon}Top Largest Files ({} files) — [f] back to directory ",
            items.len()
        )
    } else {
        let icon = if nerd { "  " } else { " " };
        format!(
            "{icon}Explorer ({} items) · {} — {} ",
            items.len(),
            format_size(current_total_size),
            app.sort_badge()
        )
    };

    List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_focused))
                .title(Span::styled(
                    title,
                    Style::default().fg(app.theme.accent).bold(),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ")
}
