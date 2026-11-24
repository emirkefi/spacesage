//! All drawing code. One submodule per screen area; `render_ui` picks the layout.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Block,
};

use crate::app::*;

mod analytics;
mod explorer;
mod footer;
mod header;
mod modals;
mod treemap;

pub(crate) use analytics::*;
pub(crate) use explorer::*;
pub(crate) use footer::*;
pub(crate) use header::*;
pub(crate) use modals::*;
pub(crate) use treemap::*;

pub(crate) fn render_ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let bg_block = Block::default().style(Style::default().bg(app.theme.bg));
    f.render_widget(bg_block, size);

    if app.view_mode == ViewMode::Treemap {
        // Dedicated fullscreen page: the treemap takes the whole terminal,
        // leaving only the 1-line footer for key hints.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Treemap page
                Constraint::Length(1), // Context-aware 1-line Footer
            ])
            .split(size);

        // The header (and its clickable breadcrumb) is not on screen here.
        app.breadcrumb_targets.clear();
        render_treemap_page(f, app, chunks[0]);
        render_footer(f, app, chunks[1]);
    } else {
        // The treemap page is not on screen, so drop its stale hit-boxes.
        app.treemap_rects.clear();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header: Breadcrumb path + Partition usage gauge + Sort & Progress
                Constraint::Length(5), // Drives & System Storage Health
                Constraint::Min(10),   // Content: Explorer / TopFiles + Details & Heatmap
                Constraint::Length(1), // Context-aware 1-line Footer
            ])
            .split(size);

        render_header(f, app, chunks[0]);
        render_disks(f, app, chunks[1]);
        render_body(f, app, chunks[2]);
        render_footer(f, app, chunks[3]);
    }

    if let Some(ref modal) = app.modal {
        match modal {
            Modal::Help => render_help_modal(f, app, size),
            Modal::DeleteConfirm(dialog) => render_delete_confirm_modal(f, app, dialog, size),
            Modal::SystemProtected(dialog) => render_system_protected_modal(f, app, dialog, size),
            Modal::Message(dialog) => render_message_modal(f, app, dialog, size),
        }
    }
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
