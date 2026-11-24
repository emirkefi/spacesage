//! Fullscreen treemap page.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::time::Duration;

use crate::app::*;
use crate::model::{format_count, format_size};

/// Maximum number of blocks drawn (and navigable) on the treemap page.
pub(crate) const TREEMAP_MAX_BLOCKS: usize = 48;

/// Slice-and-dice treemap layout. Only needs each item's size, so callers can
/// pass plain numbers instead of whole `DiskNode` subtrees.
pub(crate) fn compute_treemap_rects(
    area: Rect,
    sizes: &[u64],
    indices: &[usize],
    horizontal: bool,
    out: &mut Vec<(Rect, usize)>,
) {
    if indices.is_empty() || area.width < 2 || area.height < 2 {
        return;
    }
    if indices.len() == 1 {
        out.push((area, indices[0]));
        return;
    }

    let total: u64 = indices.iter().map(|&i| sizes[i].max(1)).sum();
    if total == 0 {
        out.push((area, indices[0]));
        return;
    }

    let mut half_sum = 0u64;
    let mut split_pos = 1usize;
    for (pos, &idx) in indices.iter().enumerate() {
        half_sum += sizes[idx].max(1);
        if half_sum >= total / 2 && pos + 1 < indices.len() {
            split_pos = pos + 1;
            break;
        }
    }

    let first_weight: u64 = indices[..split_pos].iter().map(|&i| sizes[i].max(1)).sum();
    let ratio = (first_weight as f64 / total as f64).clamp(0.15, 0.85);

    if horizontal {
        let w1 = ((area.width as f64) * ratio).round() as u16;
        let w1 = w1.clamp(1, area.width.saturating_sub(1));
        let r1 = Rect {
            x: area.x,
            y: area.y,
            width: w1,
            height: area.height,
        };
        let r2 = Rect {
            x: area.x + w1,
            y: area.y,
            width: area.width - w1,
            height: area.height,
        };
        compute_treemap_rects(r1, sizes, &indices[..split_pos], !horizontal, out);
        compute_treemap_rects(r2, sizes, &indices[split_pos..], !horizontal, out);
    } else {
        let h1 = ((area.height as f64) * ratio).round() as u16;
        let h1 = h1.clamp(1, area.height.saturating_sub(1));
        let r1 = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: h1,
        };
        let r2 = Rect {
            x: area.x,
            y: area.y + h1,
            width: area.width,
            height: area.height - h1,
        };
        compute_treemap_rects(r1, sizes, &indices[..split_pos], !horizontal, out);
        compute_treemap_rects(r2, sizes, &indices[split_pos..], !horizontal, out);
    }
}

/// Lightweight per-block snapshot so the treemap page can be drawn without
/// deep-cloning directory subtrees on every frame.
pub(crate) struct TreemapBlock {
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) is_dir: bool,
    pub(crate) file_count: usize,
    pub(crate) dir_count: usize,
    pub(crate) access_denied: bool,
    pub(crate) color: Color,
}

/// Keep the end of a long path (the part that matters), prefixed with an ellipsis.
pub(crate) fn tail_ellipsis(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars || max_chars < 2 {
        return s.to_string();
    }
    let tail: String = s.chars().skip(count - (max_chars - 1)).collect();
    format!("…{tail}")
}

/// Fullscreen treemap page. Replaces the header, drives panel and analytics
/// side panel, so everything the user still needs (path, size, filter, sort,
/// scan progress, selected item details, toasts) lives in the block's titles.
pub(crate) fn render_treemap_page(f: &mut Frame, app: &mut App, area: Rect) {
    let current_total_size = app.current_node().map(|n| n.size.max(1)).unwrap_or(1);

    let all_items = app.get_active_items();
    let total_items = all_items.len();
    let blocks: Vec<TreemapBlock> = all_items
        .into_iter()
        .take(TREEMAP_MAX_BLOCKS)
        .map(|n| TreemapBlock {
            name: n.name.clone(),
            size: n.size,
            is_dir: n.is_dir,
            file_count: n.file_count,
            dir_count: n.dir_count,
            access_denied: n.access_denied,
            color: n.category().color(&app.theme),
        })
        .collect();

    let selected_idx = app.list_state.selected().unwrap_or(0);
    let dim = Style::default().fg(app.theme.text_dim);

    // Top-left: where we are + total size (+ live filter)
    let max_path_chars = (area.width as usize).saturating_sub(44).max(12);
    let path_str = tail_ellipsis(&app.current_path.to_string_lossy(), max_path_chars);
    let mut title_spans = vec![
        Span::styled(" Treemap ", Style::default().fg(app.theme.accent).bold()),
        Span::styled("· ", dim),
        Span::styled(path_str, Style::default().fg(app.theme.text).bold()),
        Span::styled(format!(" · {} ", format_size(current_total_size)), dim),
    ];
    if app.is_filtering || !app.filter_query.is_empty() {
        title_spans.push(Span::styled(
            "/ ",
            Style::default().fg(app.theme.accent).bold(),
        ));
        title_spans.push(Span::styled(
            app.filter_query.clone(),
            Style::default().fg(app.theme.text).bold(),
        ));
        if app.is_filtering {
            title_spans.push(Span::styled(
                "_",
                Style::default().fg(app.theme.accent).bold(),
            ));
        }
        title_spans.push(Span::styled(
            format!(" ({total_items} matches) "),
            Style::default().fg(app.theme.gauge_warn),
        ));
    }

    // Top-right: sort order + scan progress (the header is hidden on this page)
    let sort_badge = app.sort_badge();
    let mut right_spans = vec![
        Span::styled(
            format!(" {sort_badge} "),
            Style::default().fg(app.theme.accent).bold(),
        ),
        Span::styled("│ ", dim),
    ];
    if app.scanning {
        let spinner = match (app.scan_start_time.elapsed().as_millis() / 120) % 6 {
            0 => "⠋",
            1 => "⠙",
            2 => "⠹",
            3 => "⠸",
            4 => "⠼",
            _ => "⠴",
        };
        right_spans.push(Span::styled(
            format!("{spinner} "),
            Style::default().fg(app.theme.gauge_warn).bold(),
        ));
        right_spans.push(Span::styled(
            format!(
                "{} · {} ",
                format_count(app.scanned_files_count),
                format_size(app.scanned_bytes_count)
            ),
            Style::default().fg(app.theme.text),
        ));
    } else {
        right_spans.push(Span::styled(
            "✓ ",
            Style::default().fg(app.theme.gauge_ok).bold(),
        ));
        right_spans.push(Span::styled(
            format!("{} ", format_count(app.scanned_files_count)),
            dim,
        ));
    }

    // Bottom-left: details of the selected block (replaces the analytics panel)
    let mut detail_spans = Vec::new();
    if let Some(b) = blocks.get(selected_idx) {
        let pct = (b.size as f64 / current_total_size as f64) * 100.0;
        detail_spans.push(Span::styled(
            format!(" {} ", b.name),
            Style::default().fg(b.color).bold(),
        ));
        detail_spans.push(Span::styled(
            format!("· {} ({:.1}%) ", format_size(b.size), pct),
            Style::default().fg(app.theme.text),
        ));
        if b.is_dir {
            detail_spans.push(Span::styled(
                format!("· {} files, {} dirs ", b.file_count, b.dir_count),
                dim,
            ));
        }
        if b.access_denied {
            detail_spans.push(Span::styled(
                "· permission denied ",
                Style::default().fg(app.theme.gauge_crit),
            ));
        }
    }
    if total_items > blocks.len() {
        detail_spans.push(Span::styled(
            format!("· showing {} of {} ", blocks.len(), total_items),
            dim,
        ));
    }

    // Bottom-right: toast messages (copy/theme/permission notices)
    let mut toast_spans = Vec::new();
    if let Some((ref msg, time)) = app.status_notification
        && time.elapsed() < Duration::from_secs(4) {
            toast_spans.push(Span::styled(
                format!(" {msg} "),
                Style::default().fg(app.theme.gauge_ok).bold(),
            ));
        }

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border_focused))
        .title(Line::from(title_spans))
        .title(Line::from(right_spans).right_aligned())
        .title_bottom(Line::from(detail_spans))
        .title_bottom(Line::from(toast_spans).right_aligned());
    let inner = outer_block.inner(area);
    f.render_widget(outer_block, area);

    // Mouse hit-testing (handle_mouse_event) reads these two fields.
    app.explorer_inner_area = inner;
    app.treemap_rects.clear();

    if blocks.is_empty() || inner.width < 4 || inner.height < 3 {
        let msg = Paragraph::new("  Empty directory or terminal too small for treemap")
            .style(Style::default().fg(app.theme.text_dim));
        f.render_widget(msg, inner);
        return;
    }

    let sizes: Vec<u64> = blocks.iter().map(|b| b.size).collect();
    let indices: Vec<usize> = (0..blocks.len()).collect();
    let mut rects = Vec::new();
    compute_treemap_rects(inner, &sizes, &indices, true, &mut rects);
    app.treemap_rects = rects.clone();

    for (rect, idx) in rects {
        if rect.width < 2 || rect.height < 1 {
            continue;
        }
        let node = &blocks[idx];
        let color = node.color;
        let is_selected = idx == selected_idx;
        let pct = (node.size as f64 / current_total_size as f64) * 100.0;

        let border_type = if is_selected {
            BorderType::Double
        } else {
            BorderType::Rounded
        };
        let border_style = if is_selected {
            Style::default().fg(app.theme.text).bold()
        } else {
            Style::default().fg(color)
        };

        if rect.width >= 6 && rect.height >= 3 {
            let mut lines = Vec::new();
            let max_chars = rect.width.saturating_sub(2) as usize;
            let label: String = node.name.chars().take(max_chars).collect();
            lines.push(Line::from(Span::styled(
                label,
                Style::default()
                    .fg(if is_selected { app.theme.text } else { color })
                    .bold(),
            )));
            if rect.height >= 4 {
                let sz = format!("{} ({:.0}%)", format_size(node.size), pct);
                let sz_trunc: String = sz.chars().take(max_chars).collect();
                lines.push(Line::from(Span::styled(
                    sz_trunc,
                    Style::default().fg(app.theme.text_dim),
                )));
            }
            if rect.height >= 5 && node.is_dir {
                let cnt = format!("{} files", format_count(node.file_count));
                let cnt_trunc: String = cnt.chars().take(max_chars).collect();
                lines.push(Line::from(Span::styled(
                    cnt_trunc,
                    Style::default().fg(app.theme.text_dim),
                )));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(border_style)
                .style(if is_selected {
                    Style::default().bg(app.theme.highlight_bg)
                } else {
                    Style::default()
                });
            f.render_widget(Paragraph::new(lines).block(block), rect);
        } else {
            let block = Block::default().style(Style::default().bg(color));
            f.render_widget(block, rect);
        }
    }
}
