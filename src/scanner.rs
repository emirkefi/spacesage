//! Background filesystem scanner and headless JSON/CSV export.

use rustc_hash::FxHashSet;
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::mpsc::{self},
    thread,
    time::{Duration, Instant},
};

use crate::model::DiskNode;

pub(crate) struct ScanItem {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) is_dir: bool,
    pub(crate) is_symlink: bool,
    pub(crate) access_denied: bool,
}

pub(crate) enum ScanMessage {
    Batch(Vec<ScanItem>),
    Progress {
        files: usize,
        bytes: u64,
        current_path: String,
    },
    Finished,
}

pub(crate) fn matches_exclude(path: &Path, name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let mut path_str_cached: Option<String> = None;
    for pat in patterns {
        if pat.is_empty() {
            continue;
        }
        if name == pat {
            return true;
        }
        if let Some(stripped) = pat.strip_prefix("*.")
            && name.ends_with(&format!(".{stripped}"))
        {
            return true;
        }
        if pat.contains('/') {
            let path_str =
                path_str_cached.get_or_insert_with(|| path.to_string_lossy().to_string());
            if path_str.contains(pat.as_str()) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn start_scanner(
    root: PathBuf,
    one_file_system: bool,
    exclude_patterns: Vec<String>,
    tx: mpsc::Sender<ScanMessage>,
) {
    thread::spawn(move || {
        let is_root_fs = root == *"/";
        #[cfg(unix)]
        let root_dev = {
            use std::os::unix::fs::MetadataExt;
            std::fs::metadata(&root).map(|m| m.dev()).ok()
        };

        let exclude_for_closure = exclude_patterns.clone();
        let has_excludes = !exclude_for_closure.is_empty();
        let walker = jwalk::WalkDir::new(&root)
            .follow_links(false)
            .skip_hidden(false)
            .process_read_dir(move |depth, _path, _state, children| {
                children.retain(|dir_entry_result| {
                    let Ok(dir_entry) = dir_entry_result else {
                        return false;
                    };

                    if is_root_fs && depth.unwrap_or(0) <= 1 {
                        let name = dir_entry.file_name.to_str().unwrap_or("");
                        if matches!(name, "proc" | "sys" | "dev" | "run") {
                            return false;
                        }
                    }

                    if has_excludes {
                        let file_name = dir_entry.file_name.to_string_lossy();
                        let path = dir_entry.path();
                        if matches_exclude(&path, &file_name, &exclude_for_closure) {
                            return false;
                        }
                    }

                    #[cfg(unix)]
                    if one_file_system
                        && dir_entry.file_type.is_dir()
                        && let Some(rdev) = root_dev
                    {
                        use std::os::unix::fs::MetadataExt;
                        if let Ok(meta) = dir_entry.metadata()
                            && meta.dev() != rdev
                        {
                            return false;
                        }
                    }

                    true
                });
            });

        let mut count = 0usize;
        let mut total_bytes = 0u64;
        let mut batch = Vec::with_capacity(1024);
        let mut last_progress = Instant::now();
        #[cfg(unix)]
        let mut seen_inodes: FxHashSet<(u64, u64)> = FxHashSet::default();

        for entry in walker {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();
            if path == root {
                continue;
            }

            if is_root_fs && entry.depth == 1 {
                let name = entry.file_name.to_str().unwrap_or("");
                if matches!(name, "proc" | "sys" | "dev" | "run") {
                    continue;
                }
            }

            let is_symlink = entry.file_type.is_symlink();
            let is_dir = entry.file_type.is_dir();
            let access_denied = if is_dir && !is_symlink {
                entry
                    .read_children
                    .as_ref()
                    .is_some_and(|r| r.error().is_some())
            } else {
                false
            };

            let mut size = 0u64;
            if !is_dir
                && !is_symlink
                && let Ok(meta) = entry.metadata()
            {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if meta.nlink() > 1 {
                        let key = (meta.dev(), meta.ino());
                        if seen_inodes.insert(key) {
                            size = meta.len();
                        }
                    } else {
                        size = meta.len();
                    }
                }
                #[cfg(not(unix))]
                {
                    size = meta.len();
                }
            }

            count += 1;
            total_bytes += size;
            batch.push(ScanItem {
                path,
                size,
                is_dir,
                is_symlink,
                access_denied,
            });

            if batch.len() >= 1024 {
                let to_send = std::mem::replace(&mut batch, Vec::with_capacity(1024));
                if tx.send(ScanMessage::Batch(to_send)).is_err() {
                    return;
                }
            }

            if count.is_multiple_of(1000) && last_progress.elapsed() >= Duration::from_millis(80) {
                last_progress = Instant::now();
                let current_path = entry.path().to_string_lossy().to_string();
                if tx
                    .send(ScanMessage::Progress {
                        files: count,
                        bytes: total_bytes,
                        current_path,
                    })
                    .is_err()
                {
                    return;
                }
            }
        }

        if !batch.is_empty() {
            let _ = tx.send(ScanMessage::Batch(batch));
        }

        let _ = tx.send(ScanMessage::Progress {
            files: count,
            bytes: total_bytes,
            current_path: String::new(),
        });

        let _ = tx.send(ScanMessage::Finished);
    });
}

pub(crate) fn run_headless_export(
    root_path: PathBuf,
    one_file_system: bool,
    exclude: Vec<String>,
    json_out: Option<PathBuf>,
    csv_out: Option<PathBuf>,
    max_depth: usize,
) -> Result<(), Box<dyn Error>> {
    let abs_path = std::fs::canonicalize(&root_path).unwrap_or(root_path);
    let name = abs_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| abs_path.to_string_lossy().to_string());
    let mut root_node = DiskNode::new(name, abs_path.clone(), true);

    let (tx, rx) = mpsc::channel();
    start_scanner(abs_path.clone(), one_file_system, exclude, tx);

    while let Ok(msg) = rx.recv() {
        match msg {
            ScanMessage::Batch(batch) => {
                for item in batch {
                    root_node.insert_ext(
                        &item.path,
                        item.size,
                        item.is_dir,
                        item.is_symlink,
                        item.access_denied,
                        &abs_path,
                    );
                }
            }
            ScanMessage::Progress { .. } => {}
            ScanMessage::Finished => break,
        }
    }

    if let Some(json_target) = json_out {
        let payload = root_node.to_json(max_depth);
        if json_target.as_os_str() == "-" {
            print!("{payload}");
        } else {
            std::fs::write(&json_target, payload)?;
            eprintln!("Exported JSON scan results to {}", json_target.display());
        }
    }

    if let Some(csv_target) = csv_out {
        let payload = root_node.to_csv(max_depth);
        if csv_target.as_os_str() == "-" {
            print!("{payload}");
        } else {
            std::fs::write(&csv_target, payload)?;
            eprintln!("Exported CSV scan results to {}", csv_target.display());
        }
    }

    Ok(())
}
