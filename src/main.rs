//! SpaceSage entry point: parse arguments, set up the terminal, run the app.

use clap::Parser;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    error::Error,
    io::{self},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

mod app;
mod cli;
mod events;
mod model;
mod safety;
mod scanner;
mod theme;
mod trash;
mod ui;

#[cfg(test)]
mod tests;

use crate::app::*;
use crate::cli::*;
use crate::events::*;
use crate::scanner::*;
use crate::theme::{AppConfig, IconMode, Theme, ThemeName};

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();
    let mut cfg = AppConfig::load();

    if let Some(ref t) = args.theme {
        cfg.theme = ThemeName::from_str(t);
    }
    if let Some(ref ic) = args.icons {
        cfg.icons = IconMode::from_str(ic);
    }
    if args.one_file_system {
        cfg.one_file_system = true;
    }
    if !args.exclude.is_empty() {
        cfg.exclude.extend(args.exclude.clone());
    }

    if args.export_json.is_some() || args.export_csv.is_some() {
        return run_headless_export(
            args.path,
            cfg.one_file_system,
            cfg.exclude,
            args.export_json,
            args.export_csv,
            args.export_depth,
        );
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        cursor::Hide
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::from_name(cfg.theme);
    let mut app = App::new(
        args.path,
        theme,
        cfg.icons,
        cfg.one_file_system,
        cfg.exclude.clone(),
    );
    let (tx, rx): (mpsc::Sender<ScanMessage>, Receiver<ScanMessage>) = mpsc::channel();
    start_scanner(
        app.root_path.clone(),
        app.one_file_system,
        app.exclude_patterns.clone(),
        tx,
    );

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(50);

    let res = run_app(&mut terminal, &mut app, rx, tick_rate, &mut last_tick);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Application Error: {err:?}");
    }

    Ok(())
}
