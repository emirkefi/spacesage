//! Keyboard and mouse handling, and the main event loop.

use crossterm::{
    cursor,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    error::Error,
    io::Stdout,
    process::Command,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

use crate::app::*;
use crate::model::DiskNode;
use crate::scanner::*;
use crate::theme::Theme;
use crate::ui::*;

pub(crate) fn open_selected_external(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn Error>> {
    let target = {
        let items = app.get_active_items();
        app.list_state
            .selected()
            .and_then(|i| items.get(i))
            .map(|n| (n.path.clone(), n.is_dir))
    };
    let Some((path, is_dir)) = target else {
        return Ok(());
    };

    if !is_dir
        && let Ok(editor) = std::env::var("EDITOR")
            && !editor.trim().is_empty() {
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture,
                    cursor::Show
                )?;
                let status = Command::new(editor).arg(&path).status();
                enable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    EnterAlternateScreen,
                    EnableMouseCapture,
                    cursor::Hide
                )?;
                terminal.clear()?;
                if status.is_ok() {
                    app.status_notification = Some((
                        format!("Opened '{}' in $EDITOR", path.display()),
                        Instant::now(),
                    ));
                    return Ok(());
                }
            }

    let res = Command::new("xdg-open")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match res {
        Ok(_) => {
            app.status_notification =
                Some((format!("Opened '{}'", path.display()), Instant::now()));
        }
        Err(e) => {
            app.status_notification = Some((format!("Failed to open: {e}"), Instant::now()));
        }
    }

    Ok(())
}

pub(crate) fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    mut rx: Receiver<ScanMessage>,
    tick_rate: Duration,
    last_tick: &mut Instant,
) -> Result<(), Box<dyn Error>> {
    let mut last_draw = Instant::now();
    let min_draw_interval = Duration::from_millis(30);

    loop {
        let mut had_updates = false;
        let drain_start = Instant::now();

        while drain_start.elapsed() < Duration::from_millis(15) {
            match rx.try_recv() {
                Ok(ScanMessage::Batch(batch)) => {
                    for item in batch {
                        app.root_node.insert_ext(
                            &item.path,
                            item.size,
                            item.is_dir,
                            item.is_symlink,
                            item.access_denied,
                            &app.root_path,
                        );
                    }
                    app.scanned_bytes_count = app.root_node.size;
                    had_updates = true;
                }
                Ok(ScanMessage::Progress {
                    files,
                    bytes,
                    current_path,
                }) => {
                    app.scanned_files_count = files;
                    app.scanned_bytes_count = bytes.max(app.root_node.size);
                    if !current_path.is_empty() {
                        app.current_scanning_item = current_path;
                    }
                    had_updates = true;
                }
                Ok(ScanMessage::Finished) => {
                    app.scanning = false;
                    app.scanned_bytes_count = app.root_node.size;
                    app.scan_duration = Some(app.scan_start_time.elapsed());
                    had_updates = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    app.scanning = false;
                    break;
                }
            }
        }

        let now = Instant::now();
        if (had_updates || now.duration_since(last_draw) >= tick_rate)
            && now.duration_since(last_draw) >= min_draw_interval {
                terminal.draw(|f| render_ui(f, app))?;
                last_draw = Instant::now();
            }

        let timeout = if app.scanning {
            Duration::from_millis(10)
        } else {
            tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::from_millis(10))
        };

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(modal) = app.modal.clone() {
                        match modal {
                            Modal::Help => match key.code {
                                KeyCode::Esc
                                | KeyCode::Char('?')
                                | KeyCode::Char('q')
                                | KeyCode::Enter => {
                                    app.modal = None;
                                }
                                _ => {}
                            },
                            Modal::DeleteConfirm(_) => match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                    app.execute_deletion();
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                    app.modal = None;
                                }
                                _ => {}
                            },
                            Modal::SystemProtected(_) => match key.code {
                                KeyCode::Esc
                                | KeyCode::Enter
                                | KeyCode::Char('q')
                                | KeyCode::Char(' ') => {
                                    app.modal = None;
                                }
                                _ => {}
                            },
                            Modal::Message(_) => match key.code {
                                KeyCode::Esc
                                | KeyCode::Enter
                                | KeyCode::Char('q')
                                | KeyCode::Char(' ') => {
                                    app.modal = None;
                                }
                                _ => {}
                            },
                        }
                    } else if app.is_filtering {
                        match key.code {
                            KeyCode::Esc => {
                                app.filter_query.clear();
                                app.is_filtering = false;
                                app.list_state.select(Some(0));
                            }
                            KeyCode::Enter => {
                                app.is_filtering = false;
                            }
                            KeyCode::Backspace => {
                                app.filter_query.pop();
                                app.list_state.select(Some(0));
                            }
                            KeyCode::Down => app.next_item(),
                            KeyCode::Up => app.previous_item(),
                            KeyCode::Char(c) => {
                                app.filter_query.push(c);
                                app.list_state.select(Some(0));
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.half_page_down();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.half_page_up();
                            }
                            KeyCode::Char('?') => app.modal = Some(Modal::Help),
                            KeyCode::Char('/') => {
                                app.is_filtering = true;
                            }
                            KeyCode::Char('s') => app.cycle_sort(),
                            KeyCode::Char('S') => app.reverse_sort(),
                            KeyCode::Char('t') => {
                                app.view_mode = match app.view_mode {
                                    ViewMode::Treemap => ViewMode::List,
                                    _ => ViewMode::Treemap,
                                };
                                app.clamp_selection();
                            }
                            KeyCode::Char('T') => {
                                let next_name = app.theme.name.next();
                                app.theme = Theme::from_name(next_name);
                                app.status_notification = Some((
                                    format!("Theme: {}", app.theme.name.label()),
                                    Instant::now(),
                                ));
                            }
                            KeyCode::Char('f') => {
                                app.view_mode = match app.view_mode {
                                    ViewMode::TopFiles => ViewMode::List,
                                    _ => ViewMode::TopFiles,
                                };
                                app.list_state.select(Some(0));
                            }
                            KeyCode::Char('g') | KeyCode::Home => app.jump_top(),
                            KeyCode::Char('G') | KeyCode::End => app.jump_bottom(),
                            KeyCode::PageDown => app.half_page_down(),
                            KeyCode::PageUp => app.half_page_up(),
                            KeyCode::Char('~') => {
                                if let Some(new_home_root) = app.jump_home() {
                                    let name = new_home_root
                                        .file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "~".into());
                                    app.root_path = new_home_root.clone();
                                    app.current_path = new_home_root.clone();
                                    app.root_node = DiskNode::new(name, new_home_root, true);
                                    app.marked_paths.clear();
                                    app.scanning = true;
                                    app.scanned_files_count = 0;
                                    app.scanned_bytes_count = 0;
                                    app.scan_start_time = Instant::now();
                                    let (new_tx, new_rx) = mpsc::channel();
                                    rx = new_rx;
                                    start_scanner(
                                        app.root_path.clone(),
                                        app.one_file_system,
                                        app.exclude_patterns.clone(),
                                        new_tx,
                                    );
                                }
                            }
                            KeyCode::Char(' ') => app.toggle_mark_selected(),
                            KeyCode::Char('y') => app.copy_selected_path(),
                            KeyCode::Char('o') => {
                                open_selected_external(terminal, app)?;
                            }
                            KeyCode::Char('r') => {
                                app.disks.refresh(true);
                                app.status_notification = Some((
                                    String::from("Refreshed partition metrics"),
                                    Instant::now(),
                                ));
                            }
                            KeyCode::Char('d') => {
                                app.request_delete(false);
                            }
                            KeyCode::Char('D') | KeyCode::Delete => {
                                app.request_delete(true);
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                                app.enter_dir();
                            }
                            KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                                app.exit_dir();
                            }
                            KeyCode::Esc => {
                                if !app.filter_query.is_empty() {
                                    app.filter_query.clear();
                                    app.list_state.select(Some(0));
                                } else if !app.marked_paths.is_empty() {
                                    app.marked_paths.clear();
                                } else if app.view_mode != ViewMode::List {
                                    app.view_mode = ViewMode::List;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse_event(app, mouse);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            *last_tick = Instant::now();
        }
    }
}

pub(crate) fn handle_mouse_event(app: &mut App, mouse: MouseEvent) {
    if app.modal.is_some() {
        return;
    }

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            app.next_item();
        }
        MouseEventKind::ScrollUp => {
            app.previous_item();
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let col = mouse.column;
            let row = mouse.row;

            // Check breadcrumb clicks in header (the header is hidden on the treemap page)
            if row == 1 && app.view_mode != ViewMode::Treemap {
                for (x_start, x_end, target_path) in app.breadcrumb_targets.clone() {
                    if col >= x_start && col < x_end {
                        if target_path != app.current_path {
                            app.record_current_cursor();
                            app.current_path = target_path.clone();
                            app.filter_query.clear();
                            app.restore_cursor_for(&target_path, None);
                        }
                        return;
                    }
                }
            }

            let inner = app.explorer_inner_area;
            if col >= inner.x
                && col < inner.x + inner.width
                && row >= inner.y
                && row < inner.y + inner.height
            {
                if app.view_mode == ViewMode::Treemap {
                    for &(rect, item_idx) in &app.treemap_rects {
                        if col >= rect.x
                            && col < rect.x + rect.width
                            && row >= rect.y
                            && row < rect.y + rect.height
                        {
                            let is_double = app.last_click.is_some_and(|(t, prev)| {
                                prev == item_idx && t.elapsed() < Duration::from_millis(400)
                            });
                            app.list_state.select(Some(item_idx));
                            if is_double {
                                app.enter_dir();
                                app.last_click = None;
                            } else {
                                app.last_click = Some((Instant::now(), item_idx));
                            }
                            return;
                        }
                    }
                } else {
                    let offset = app.list_state.offset();
                    let clicked_idx = offset + (row - inner.y) as usize;
                    let len = app.get_active_items().len();
                    if clicked_idx < len {
                        let is_double = app.last_click.is_some_and(|(t, prev)| {
                            prev == clicked_idx && t.elapsed() < Duration::from_millis(400)
                        });
                        app.list_state.select(Some(clicked_idx));
                        if is_double {
                            app.enter_dir();
                            app.last_click = None;
                        } else {
                            app.last_click = Some((Instant::now(), clicked_idx));
                        }
                    }
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            app.exit_dir();
        }
        _ => {}
    }
}
