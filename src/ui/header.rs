//! Header (path, partition gauge, sort/scan status) and the drives panel.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::{path::PathBuf, time::Duration};

use crate::app::*;
use crate::model::{format_count, format_size};
use crate::safety::is_superuser;

pub(crate) fn render_header(f: &mut Frame, app: &mut App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(44), // Logo + Active Partition Usage Gauge
            Constraint::Min(24),    // Breadcrumb Path Bar + Search Prompt
            Constraint::Length(36), // Sort indicator + Live Scan Progress
        ])
        .split(area);

    let nerd = app.icon_mode.use_nerd_icons();
    let arch_icon = if nerd { " 󰣇 " } else { " " };
    let mut left_spans = vec![
        Span::styled(arch_icon, Style::default().fg(app.theme.accent).bold()),
        Span::styled("ARCH·DISK", Style::default().fg(app.theme.text).bold()),
    ];
    if is_superuser() {
        left_spans.push(Span::styled(
            " [ROOT]",
            Style::default().fg(app.theme.gauge_crit).bold(),
        ));
    }

    if let Some((_mount, used, total, pct)) = app.active_disk_usage() {
        let gauge_color = if pct >= 90.0 {
            app.theme.gauge_crit
        } else if pct >= 75.0 {
            app.theme.gauge_warn
        } else {
            app.theme.gauge_ok
        };
        let bar_w = 8usize;
        let filled = ((pct / 100.0) * bar_w as f64)
            .round()
            .clamp(0.0, bar_w as f64) as usize;
        left_spans.push(Span::styled("  [", Style::default().fg(app.theme.text_dim)));
        left_spans.push(Span::styled(
            "━".repeat(filled),
            Style::default().fg(gauge_color).bold(),
        ));
        left_spans.push(Span::styled(
            "─".repeat(bar_w.saturating_sub(filled)),
            Style::default().fg(app.theme.bar_empty),
        ));
        left_spans.push(Span::styled("] ", Style::default().fg(app.theme.text_dim)));
        left_spans.push(Span::styled(
            format!(
                "{} / {} · {:.0}%",
                format_size(used),
                format_size(total),
                pct
            ),
            Style::default().fg(gauge_color).bold(),
        ));
    }

    let left_block = Paragraph::new(Line::from(left_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_superuser() {
                Style::default().fg(app.theme.gauge_crit)
            } else {
                Style::default().fg(app.theme.border)
            }),
    );
    f.render_widget(left_block, header_chunks[0]);

    // Middle: Breadcrumb path bar (/ › home › emir › Downloads) + visible Search prompt + Toast
    app.breadcrumb_targets.clear();
    let mut mid_spans = Vec::new();
    let mut cur_x = header_chunks[1].x + 2;
    mid_spans.push(Span::raw(" "));

    let folder_icon = if nerd { "  " } else { "" };
    if !folder_icon.is_empty() {
        cur_x += folder_icon.chars().count() as u16;
        mid_spans.push(Span::styled(folder_icon, Style::default().fg(app.theme.accent)));
    }

    let components: Vec<_> = app.current_path.components().collect();
    let mut acc_path = PathBuf::new();
    for (idx, comp) in components.iter().enumerate() {
        acc_path.push(comp.as_os_str());
        let label = if idx == 0 && comp.as_os_str() == "/" {
            "/".to_string()
        } else {
            comp.as_os_str().to_string_lossy().to_string()
        };
        let is_last = idx + 1 == components.len();
        let style = if is_last {
            Style::default().fg(app.theme.accent).bold()
        } else {
            Style::default().fg(app.theme.text)
        };
        let start_x = cur_x;
        let width = label.chars().count() as u16;
        cur_x += width;
        app.breadcrumb_targets
            .push((start_x, cur_x, acc_path.clone()));
        mid_spans.push(Span::styled(label, style));

        if !is_last {
            mid_spans.push(Span::styled(" › ", Style::default().fg(app.theme.text_dim)));
            cur_x += 3;
        }
    }

    if app.is_filtering || !app.filter_query.is_empty() {
        let match_count = app.get_active_items().len();
        mid_spans.push(Span::styled(
            "   / ",
            Style::default().fg(app.theme.accent).bold(),
        ));
        mid_spans.push(Span::styled(
            app.filter_query.clone(),
            Style::default().fg(app.theme.text).bold(),
        ));
        if app.is_filtering {
            mid_spans.push(Span::styled(
                "_",
                Style::default().fg(app.theme.accent).bold(),
            ));
        }
        mid_spans.push(Span::styled(
            format!(" ({match_count} matches)"),
            Style::default().fg(app.theme.gauge_warn),
        ));
    } else if let Some((ref msg, time)) = app.status_notification
        && time.elapsed() < Duration::from_secs(4) {
            mid_spans.push(Span::styled(
                "  │ ",
                Style::default().fg(app.theme.text_dim),
            ));
            mid_spans.push(Span::styled(
                msg.clone(),
                Style::default().fg(app.theme.gauge_ok).bold(),
            ));
        }

    let path_widget = Paragraph::new(Line::from(mid_spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if app.is_filtering {
                    Style::default().fg(app.theme.accent)
                } else {
                    Style::default().fg(app.theme.border)
                }),
        )
        .alignment(Alignment::Left);
    f.render_widget(path_widget, header_chunks[1]);

    // Right: Sort indicator (Sort: Size ↓) + Scan spinner (files scanned · size so far)
    let sort_badge = app.sort_badge();
    let status_line = if app.scanning {
        let spinner = match (app.scan_start_time.elapsed().as_millis() / 120) % 6 {
            0 => "⠋",
            1 => "⠙",
            2 => "⠹",
            3 => "⠸",
            4 => "⠼",
            _ => "⠴",
        };
        Line::from(vec![
            Span::styled(
                format!(" {} ", sort_badge),
                Style::default().fg(app.theme.accent).bold(),
            ),
            Span::styled("│ ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!("{spinner} "),
                Style::default().fg(app.theme.gauge_warn).bold(),
            ),
            Span::styled(
                format!(
                    "{} · {} ",
                    format_count(app.scanned_files_count),
                    format_size(app.scanned_bytes_count)
                ),
                Style::default().fg(app.theme.text),
            ),
        ])
    } else {
        let secs = app.scan_duration.map(|d| d.as_secs_f32()).unwrap_or(0.0);
        Line::from(vec![
            Span::styled(
                format!(" {} ", sort_badge),
                Style::default().fg(app.theme.accent).bold(),
            ),
            Span::styled("│ ", Style::default().fg(app.theme.text_dim)),
            Span::styled("✓ ", Style::default().fg(app.theme.gauge_ok).bold()),
            Span::styled(
                format!("{} ({:.2}s) ", format_count(app.scanned_files_count), secs),
                Style::default().fg(app.theme.text_dim),
            ),
        ])
    };

    let status_widget = Paragraph::new(status_line)
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if app.scanning {
                    Style::default().fg(app.theme.gauge_warn)
                } else {
                    Style::default().fg(app.theme.border)
                }),
        );
    f.render_widget(status_widget, header_chunks[2]);
}

pub(crate) fn render_disks(f: &mut Frame, app: &App, area: Rect) {
    let mut disk_lines = Vec::new();
    let nerd = app.icon_mode.use_nerd_icons();

    for disk in &app.disks {
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);
        let ratio = if total > 0 {
            used as f64 / total as f64
        } else {
            0.0
        };
        let pct = ratio * 100.0;

        let mount = disk.mount_point().to_string_lossy();
        let name = disk.name().to_string_lossy();
        let fs = disk.file_system().to_string_lossy();

        let gauge_color = if pct >= 90.0 {
            app.theme.gauge_crit
        } else if pct >= 75.0 {
            app.theme.gauge_warn
        } else {
            app.theme.gauge_ok
        };

        let meter_width = 18;
        let filled = ((pct / 100.0) * meter_width as f64).round() as usize;
        let filled = filled.clamp(0, meter_width);
        let bar_filled = "━".repeat(filled);
        let bar_empty = "─".repeat(meter_width - filled);
        let disk_icon = if nerd { "  " } else { " * " };

        disk_lines.push(Line::from(vec![
            Span::styled(disk_icon, Style::default().fg(app.theme.accent).bold()),
            Span::styled(
                format!("{:<10} ", name),
                Style::default().fg(app.theme.text).bold(),
            ),
            Span::styled(
                format!("({:<5}) ", fs),
                Style::default().fg(app.theme.text_dim),
            ),
            Span::styled("on ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!("{:<12} ", mount),
                Style::default().fg(app.theme.gauge_warn),
            ),
            Span::styled("[", Style::default().fg(app.theme.text_dim)),
            Span::styled(bar_filled, Style::default().fg(gauge_color).bold()),
            Span::styled(bar_empty, Style::default().fg(app.theme.bar_empty)),
            Span::styled("] ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!(
                    "{} / {} ({:4.1}%) · {} free",
                    format_size(used),
                    format_size(total),
                    pct,
                    format_size(available)
                ),
                Style::default().fg(gauge_color).bold(),
            ),
        ]));
    }

    if disk_lines.is_empty() {
        disk_lines.push(Line::from(Span::styled(
            " No mounts detected",
            Style::default().fg(app.theme.text_dim),
        )));
    }

    let title_icon = if nerd { "  " } else { " " };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.border))
        .title(Span::styled(
            format!("{title_icon}Partitions & Mount Telemetry "),
            Style::default().fg(app.theme.accent).bold(),
        ));

    let disk_widget = Paragraph::new(disk_lines).block(block);
    f.render_widget(disk_widget, area);
}
