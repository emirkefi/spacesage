//! Command-line arguments.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "spacesage",
    author = "Emir <emirlkf@hotmail.com>",
    version = "0.2.0",
    about = "SpaceSage — Ultra-fast, cosmic terminal disk space analyzer and heatmap for Linux"
)]
pub(crate) struct CliArgs {
    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Stay on the same filesystem as the root path (like du -x)
    #[arg(short = 'x', long = "one-file-system")]
    pub(crate) one_file_system: bool,

    /// Exclude paths or directory names matching pattern (can be specified multiple times)
    #[arg(short = 'e', long = "exclude")]
    pub(crate) exclude: Vec<String>,

    /// Color theme: arch, catppuccin, gruvbox, mono, no_color
    #[arg(long = "theme")]
    pub(crate) theme: Option<String>,

    /// Icon mode: auto, nerd, ascii, none
    #[arg(long = "icons")]
    pub(crate) icons: Option<String>,

    /// Export scan results to JSON file (use '-' for stdout) without launching TUI
    #[arg(long = "export-json")]
    pub(crate) export_json: Option<PathBuf>,

    /// Export scan results to CSV file (use '-' for stdout) without launching TUI
    #[arg(long = "export-csv")]
    pub(crate) export_csv: Option<PathBuf>,

    /// Maximum depth when exporting to JSON or CSV
    #[arg(long = "export-depth", default_value_t = 4)]
    pub(crate) export_depth: usize,
}
