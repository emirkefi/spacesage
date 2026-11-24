//! Popup dialogs: help, delete confirmation, system-protected and message.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::model::format_size;
use crate::ui::*;

pub(crate) fn render_help_modal(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(72, 78, area);
    f.render_widget(Clear, popup_area);

    let key_style = Style::default().fg(app.theme.gauge_warn).bold();
    let val_style = Style::default().fg(app.theme.text);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard & Mouse Controls",
            Style::default().fg(app.theme.accent).bold(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j / k / ↑ / ↓ / Scroll  ", key_style),
            Span::styled("Move selection up / down", val_style),
        ]),
        Line::from(vec![
            Span::styled("  g / G                   ", key_style),
            Span::styled("Jump to top / bottom of list", val_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d / Ctrl+u         ", key_style),
            Span::styled("Scroll half-page down / up", val_style),
        ]),
        Line::from(vec![
            Span::styled("  l / Enter / Dbl-Click   ", key_style),
            Span::styled("Enter directory (or jump to file in Top-N view)", val_style),
        ]),
        Line::from(vec![
            Span::styled("  h / Backspace / R-Click ", key_style),
            Span::styled("Return to parent directory (remembers cursor)", val_style),
        ]),
        Line::from(vec![
            Span::styled("  ~                       ", key_style),
            Span::styled("Jump to home directory ($HOME)", val_style),
        ]),
        Line::from(vec![
            Span::styled("  s / S                   ", key_style),
            Span::styled(
                "Cycle sort mode (Size / Name / Count) / Reverse direction",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  /                       ", key_style),
            Span::styled(
                "Filter items with live match count & highlighting",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  t                       ", key_style),
            Span::styled(
                "Toggle the fullscreen Treemap page (t / Esc returns to list)",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  f                       ", key_style),
            Span::styled(
                "Toggle Top-N largest files across the entire scan",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space                   ", key_style),
            Span::styled(
                "Mark / unmark item for multi-select batch actions",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  d                       ", key_style),
            Span::styled(
                "Move selected/marked item(s) to FreeDesktop Trash",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  D / Delete              ",
                Style::default().fg(app.theme.gauge_crit).bold(),
            ),
            Span::styled("Permanently delete selected/marked item(s)", val_style),
        ]),
        Line::from(vec![
            Span::styled("  y / o                   ", key_style),
            Span::styled(
                "Copy path to clipboard (y) / Open in $EDITOR or file manager (o)",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  T / r                   ", key_style),
            Span::styled(
                "Cycle color theme (T) / Refresh partition telemetry (r)",
                val_style,
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  q / Ctrl+c              ",
                Style::default().fg(app.theme.gauge_crit).bold(),
            ),
            Span::styled("Quit application safely", val_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press [Esc] or [?] to close this window.",
            Style::default().fg(app.theme.text_dim),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.bg))
        .title(Span::styled(
            " HELP & CHEATSHEET ",
            Style::default().fg(app.theme.gauge_warn).bold(),
        ));

    let popup = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);
    f.render_widget(popup, popup_area);
}

pub(crate) fn render_delete_confirm_modal(
    f: &mut Frame,
    app: &App,
    dialog: &DeleteConfirmDialog,
    area: Rect,
) {
    let popup_area = centered_rect(68, 56, area);
    f.render_widget(Clear, popup_area);

    let border_color = if dialog.any_system || dialog.permanent {
        app.theme.gauge_crit
    } else {
        app.theme.gauge_warn
    };

    let title = if dialog.any_system {
        " CAUTION: SUPERUSER SYSTEM DELETION "
    } else if dialog.permanent {
        " CONFIRM PERMANENT DELETION "
    } else {
        " MOVE TO TRASH "
    };

    let prompt = if dialog.permanent {
        "  Are you sure you want to PERMANENTLY delete the following?"
    } else {
        "  Move the following item(s) to the system Trash?"
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            prompt,
            Style::default().fg(app.theme.text).bold(),
        )),
        Line::from(""),
    ];

    if dialog.targets.len() == 1 {
        let t = &dialog.targets[0];
        lines.push(Line::from(vec![
            Span::styled("  Item Type:  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                if t.is_dir {
                    "Directory / Folder (Recursive)"
                } else {
                    "Regular File"
                },
                Style::default().fg(app.theme.accent).bold(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Item Name:  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(&t.name, Style::default().fg(app.theme.gauge_warn).bold()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Location:   ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                t.path.to_string_lossy(),
                Style::default().fg(app.theme.text),
            ),
        ]));
        if t.is_dir {
            lines.push(Line::from(vec![
                Span::styled("  Contains:   ", Style::default().fg(app.theme.text_dim)),
                Span::styled(
                    format!("{} subfolders, {} files", t.dir_count, t.file_count),
                    Style::default().fg(app.theme.text),
                ),
            ]));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Batch Size: ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!("{} marked items", dialog.targets.len()),
                Style::default().fg(app.theme.gauge_warn).bold(),
            ),
        ]));
        for t in dialog.targets.iter().take(4) {
            lines.push(Line::from(vec![
                Span::styled("    • ", Style::default().fg(app.theme.text_dim)),
                Span::styled(&t.name, Style::default().fg(app.theme.text)),
                Span::styled(
                    format!(" ({})", format_size(t.size)),
                    Style::default().fg(app.theme.text_dim),
                ),
            ]));
        }
        if dialog.targets.len() > 4 {
            lines.push(Line::from(Span::styled(
                format!("    …and {} more", dialog.targets.len() - 4),
                Style::default().fg(app.theme.text_dim),
            )));
        }
    }

    lines.push(Line::from(vec![
        Span::styled("  Total Size: ", Style::default().fg(app.theme.text_dim)),
        Span::styled(
            format_size(dialog.total_size),
            Style::default().fg(border_color).bold(),
        ),
    ]));
    lines.push(Line::from(""));

    if dialog.any_system {
        lines.push(Line::from(Span::styled(
            "  EXTREME CAUTION: Superuser privilege active on system path!",
            Style::default().fg(app.theme.gauge_crit).bold(),
        )));
    } else if dialog.permanent {
        lines.push(Line::from(Span::styled(
            "  CAUTION: Permanent deletion bypasses Trash and cannot be undone.",
            Style::default().fg(app.theme.gauge_crit).bold(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Safe Action: Items will be moved to your FreeDesktop Trash folder.",
            Style::default().fg(app.theme.gauge_ok),
        )));
    }

    lines.push(Line::from(""));
    let confirm_label = if dialog.permanent {
        " Confirm Permanent Delete    "
    } else {
        " Confirm Move to Trash       "
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            " [ y / Enter ] ",
            Style::default()
                .bg(border_color)
                .fg(app.theme.match_fg)
                .bold(),
        ),
        Span::styled(confirm_label, Style::default().fg(border_color).bold()),
        Span::styled(
            "[ n / Esc ] ",
            Style::default()
                .bg(app.theme.highlight_bg)
                .fg(app.theme.text)
                .bold(),
        ),
        Span::styled(" Cancel", Style::default().fg(app.theme.text_dim)),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg))
        .title(Span::styled(
            title,
            Style::default().fg(border_color).bold(),
        ));

    let popup = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(popup, popup_area);
}

pub(crate) fn render_system_protected_modal(
    f: &mut Frame,
    app: &App,
    dialog: &SystemProtectedDialog,
    area: Rect,
) {
    let popup_area = centered_rect(68, 52, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  CANNOT DELETE SYSTEM FILES OR FOLDERS",
            Style::default().fg(app.theme.gauge_crit).bold(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Target:    ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                &dialog.target_name,
                Style::default().fg(app.theme.gauge_warn).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Path:      ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                dialog.target_path.to_string_lossy(),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Security Policy:",
            Style::default().fg(app.theme.accent).bold(),
        )),
        Line::from(Span::styled(
            "    System components and root-owned files are protected from modification.",
            Style::default().fg(app.theme.text),
        )),
        Line::from(Span::styled(
            "    To delete system files, re-launch with sudo:",
            Style::default().fg(app.theme.text_dim),
        )),
        Line::from(vec![
            Span::raw("      "),
            Span::styled(
                "sudo spacesage ",
                Style::default().fg(app.theme.gauge_ok).bold(),
            ),
            Span::styled("<path>", Style::default().fg(app.theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " [ Esc / Enter ] ",
                Style::default()
                    .bg(app.theme.highlight_bg)
                    .fg(app.theme.text)
                    .bold(),
            ),
            Span::styled(" Dismiss", Style::default().fg(app.theme.text_dim)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.theme.gauge_crit))
        .style(Style::default().bg(app.theme.bg))
        .title(Span::styled(
            " ACCESS RESTRICTED - SYSTEM PROTECTED ",
            Style::default().fg(app.theme.gauge_crit).bold(),
        ));

    let popup = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(popup, popup_area);
}

pub(crate) fn render_message_modal(f: &mut Frame, app: &App, dialog: &MessageDialog, area: Rect) {
    let popup_area = centered_rect(60, 35, area);
    f.render_widget(Clear, popup_area);

    let border_color = if dialog.is_error {
        app.theme.gauge_crit
    } else {
        app.theme.accent
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", dialog.message),
            Style::default().fg(app.theme.text),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " [ Esc / Enter ] ",
                Style::default()
                    .bg(app.theme.highlight_bg)
                    .fg(app.theme.text)
                    .bold(),
            ),
            Span::styled(" Dismiss", Style::default().fg(app.theme.text_dim)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg))
        .title(Span::styled(
            format!(" {} ", dialog.title),
            Style::default().fg(border_color).bold(),
        ));

    let popup = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(popup, popup_area);
}
