//! Application state, navigation and actions (everything that is not drawing or input).

use ratatui::{layout::Rect, widgets::ListState};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use sysinfo::Disks;

use crate::model::{DiskNode, format_size};
use crate::safety::{DeletionRestriction, check_deletion_permission};
use crate::theme::{IconMode, Theme};
use crate::trash::{copy_to_clipboard, move_to_trash};
use crate::ui::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SortField {
    Size,
    Name,
    Count,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SortDirection {
    Desc,
    Asc,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewMode {
    List,
    Treemap,
    TopFiles,
}

#[derive(Clone, Debug)]
pub(crate) struct DeleteTarget {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) is_dir: bool,
    pub(crate) file_count: usize,
    pub(crate) dir_count: usize,
    pub(crate) is_system: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DeleteConfirmDialog {
    pub(crate) targets: Vec<DeleteTarget>,
    pub(crate) total_size: u64,
    pub(crate) permanent: bool,
    pub(crate) any_system: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SystemProtectedDialog {
    pub(crate) target_name: String,
    pub(crate) target_path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct MessageDialog {
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) is_error: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum Modal {
    Help,
    DeleteConfirm(DeleteConfirmDialog),
    SystemProtected(SystemProtectedDialog),
    Message(MessageDialog),
}

pub(crate) struct App {
    pub(crate) root_path: PathBuf,
    pub(crate) current_path: PathBuf,
    pub(crate) root_node: DiskNode,
    pub(crate) disks: Disks,
    pub(crate) list_state: ListState,
    pub(crate) sort_field: SortField,
    pub(crate) sort_dir: SortDirection,
    pub(crate) view_mode: ViewMode,
    pub(crate) theme: Theme,
    pub(crate) icon_mode: IconMode,
    pub(crate) one_file_system: bool,
    pub(crate) exclude_patterns: Vec<String>,
    pub(crate) scanning: bool,
    pub(crate) scanned_files_count: usize,
    pub(crate) scanned_bytes_count: u64,
    pub(crate) current_scanning_item: String,
    pub(crate) scan_start_time: Instant,
    pub(crate) scan_duration: Option<Duration>,
    pub(crate) modal: Option<Modal>,
    pub(crate) status_notification: Option<(String, Instant)>,
    pub(crate) filter_query: String,
    pub(crate) is_filtering: bool,
    pub(crate) cursor_history: HashMap<PathBuf, String>,
    pub(crate) marked_paths: HashSet<PathBuf>,
    pub(crate) last_click: Option<(Instant, usize)>,
    pub(crate) explorer_inner_area: Rect,
    pub(crate) treemap_rects: Vec<(Rect, usize)>,
    pub(crate) breadcrumb_targets: Vec<(u16, u16, PathBuf)>,
}

impl App {
    pub(crate) fn new(
        target_path: PathBuf,
        theme: Theme,
        icon_mode: IconMode,
        one_file_system: bool,
        exclude_patterns: Vec<String>,
    ) -> Self {
        let abs_path = std::fs::canonicalize(&target_path).unwrap_or(target_path);
        let name = abs_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_path.to_string_lossy().to_string());

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            root_path: abs_path.clone(),
            current_path: abs_path.clone(),
            root_node: DiskNode::new(name, abs_path, true),
            disks: Disks::new_with_refreshed_list(),
            list_state,
            sort_field: SortField::Size,
            sort_dir: SortDirection::Desc,
            view_mode: ViewMode::List,
            theme,
            icon_mode,
            one_file_system,
            exclude_patterns,
            scanning: true,
            scanned_files_count: 0,
            scanned_bytes_count: 0,
            current_scanning_item: String::from("Starting scan..."),
            scan_start_time: Instant::now(),
            scan_duration: None,
            modal: None,
            status_notification: None,
            filter_query: String::new(),
            is_filtering: false,
            cursor_history: HashMap::new(),
            marked_paths: HashSet::new(),
            last_click: None,
            explorer_inner_area: Rect::default(),
            treemap_rects: Vec::new(),
            breadcrumb_targets: Vec::new(),
        }
    }

    pub(crate) fn current_node(&self) -> Option<&DiskNode> {
        self.root_node.find_node(&self.current_path)
    }

    pub(crate) fn get_active_items(&self) -> Vec<&DiskNode> {
        if self.view_mode == ViewMode::TopFiles {
            let scope = self.current_node().unwrap_or(&self.root_node);
            let mut top = scope.collect_largest_files(200);
            if !self.filter_query.is_empty() {
                let q = self.filter_query.to_lowercase();
                top.retain(|n| n.name.to_lowercase().contains(&q));
            }
            top.truncate(100);
            return top;
        }

        let Some(node) = self.current_node() else {
            return Vec::new();
        };
        Self::filter_and_sort_node(node, &self.filter_query, self.sort_field, self.sort_dir)
    }

    /// Alias kept for unit test compatibility
    #[allow(dead_code)]
    pub(crate) fn get_filtered_sorted_children(&self) -> Vec<&DiskNode> {
        let Some(node) = self.current_node() else {
            return Vec::new();
        };
        Self::filter_and_sort_node(node, &self.filter_query, self.sort_field, self.sort_dir)
    }

    pub(crate) fn filter_and_sort_node<'a>(
        node: &'a DiskNode,
        filter_query: &str,
        sort_field: SortField,
        sort_dir: SortDirection,
    ) -> Vec<&'a DiskNode> {
        let filter_lower = if filter_query.is_empty() {
            None
        } else {
            Some(filter_query.to_lowercase())
        };

        let mut items: Vec<&DiskNode> = node
            .children
            .values()
            .filter(|child| {
                if let Some(ref query) = filter_lower {
                    child.name.to_lowercase().contains(query)
                } else {
                    true
                }
            })
            .collect();

        items.sort_by(|a, b| {
            let ord = match sort_field {
                SortField::Size => a.size.cmp(&b.size).then_with(|| a.name.cmp(&b.name)),
                SortField::Name => a
                    .name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.size.cmp(&b.size)),
                SortField::Count => (a.file_count + a.dir_count)
                    .cmp(&(b.file_count + b.dir_count))
                    .then_with(|| a.size.cmp(&b.size)),
            };
            match sort_dir {
                SortDirection::Asc => ord,
                SortDirection::Desc => ord.reverse(),
            }
        });

        items
    }

    pub(crate) fn record_current_cursor(&mut self) {
        let selected_name = {
            let items = self.get_active_items();
            self.list_state
                .selected()
                .and_then(|i| items.get(i))
                .map(|n| n.name.clone())
        };
        if let Some(name) = selected_name {
            self.cursor_history.insert(self.current_path.clone(), name);
        }
    }

    pub(crate) fn restore_cursor_for(&mut self, dir_path: &Path, fallback_name: Option<&str>) {
        let target_name = fallback_name
            .map(|s| s.to_string())
            .or_else(|| self.cursor_history.get(dir_path).cloned());

        let items = self.get_active_items();
        if items.is_empty() {
            self.list_state.select(Some(0));
            return;
        }

        if let Some(name) = target_name
            && let Some(idx) = items.iter().position(|n| n.name == name) {
                self.list_state.select(Some(idx));
                return;
            }
        self.list_state.select(Some(0));
    }

    /// Number of items the cursor may move over. On the treemap page only the
    /// first TREEMAP_MAX_BLOCKS items are drawn, so the cursor must not leave them.
    pub(crate) fn nav_len(&self) -> usize {
        let len = self.get_active_items().len();
        if self.view_mode == ViewMode::Treemap {
            len.min(TREEMAP_MAX_BLOCKS)
        } else {
            len
        }
    }

    /// Pull the selection back onto a visible block (used when entering the treemap page).
    pub(crate) fn clamp_selection(&mut self) {
        let len = self.nav_len();
        if let Some(sel) = self.list_state.selected()
            && len > 0 && sel >= len {
                self.list_state.select(Some(len - 1));
            }
    }

    pub(crate) fn next_item(&mut self) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub(crate) fn previous_item(&mut self) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub(crate) fn jump_top(&mut self) {
        if !self.get_active_items().is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub(crate) fn jump_bottom(&mut self) {
        let len = self.nav_len();
        if len > 0 {
            self.list_state.select(Some(len - 1));
        }
    }

    pub(crate) fn half_page_down(&mut self) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let step = (self.explorer_inner_area.height as usize / 2).max(5);
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((cur + step).min(len - 1)));
    }

    pub(crate) fn half_page_up(&mut self) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let step = (self.explorer_inner_area.height as usize / 2).max(5);
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(cur.saturating_sub(step)));
    }

    pub(crate) fn enter_dir(&mut self) {
        if self.view_mode == ViewMode::TopFiles {
            // In TopFiles mode, pressing Enter/l jumps to the file's parent directory and selects it
            let target_info = {
                let items = self.get_active_items();
                self.list_state
                    .selected()
                    .and_then(|idx| items.get(idx))
                    .map(|n| (n.path.parent().map(|p| p.to_path_buf()), n.name.clone()))
            };
            if let Some((Some(parent), file_name)) = target_info {
                self.view_mode = ViewMode::List;
                self.current_path = parent.clone();
                self.filter_query.clear();
                self.restore_cursor_for(&parent, Some(&file_name));
            }
            return;
        }

        let target_info = {
            let items = self.get_active_items();
            self.list_state
                .selected()
                .and_then(|selected| items.get(selected))
                .map(|t| (t.is_dir, t.path.clone(), t.access_denied))
        };

        if let Some((is_dir, target_path, access_denied)) = target_info
            && is_dir {
                if access_denied {
                    self.status_notification = Some((
                        String::from("Permission denied — run with sudo to inspect this folder"),
                        Instant::now(),
                    ));
                }
                self.record_current_cursor();
                self.current_path = target_path.clone();
                self.filter_query.clear();
                self.restore_cursor_for(&target_path, None);
            }
    }

    pub(crate) fn exit_dir(&mut self) {
        if self.view_mode == ViewMode::TopFiles {
            self.view_mode = ViewMode::List;
            return;
        }
        if self.current_path != self.root_path {
            let leaving_name = self
                .current_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
            if let Some(parent) = self.current_path.parent() {
                let parent_buf = parent.to_path_buf();
                self.record_current_cursor();
                self.current_path = parent_buf.clone();
                self.filter_query.clear();
                self.restore_cursor_for(&parent_buf, leaving_name.as_deref());
            }
        }
    }

    pub(crate) fn jump_home(&mut self) -> Option<PathBuf> {
        let Ok(home_str) = std::env::var("HOME") else {
            return None;
        };
        let home = PathBuf::from(home_str);
        if !home.exists() {
            return None;
        }
        if home.starts_with(&self.root_path) && self.root_node.find_node(&home).is_some() {
            self.record_current_cursor();
            self.current_path = home.clone();
            self.filter_query.clear();
            self.restore_cursor_for(&home, None);
            self.status_notification =
                Some((format!("Jumped to ~ ({})", home.display()), Instant::now()));
            None
        } else {
            Some(home)
        }
    }

    pub(crate) fn cycle_sort(&mut self) {
        self.sort_field = match self.sort_field {
            SortField::Size => SortField::Name,
            SortField::Name => SortField::Count,
            SortField::Count => SortField::Size,
        };
        self.sort_dir = match self.sort_field {
            SortField::Size => SortDirection::Desc,
            SortField::Name => SortDirection::Asc,
            SortField::Count => SortDirection::Desc,
        };
        self.list_state.select(Some(0));
    }

    pub(crate) fn reverse_sort(&mut self) {
        self.sort_dir = match self.sort_dir {
            SortDirection::Desc => SortDirection::Asc,
            SortDirection::Asc => SortDirection::Desc,
        };
        self.list_state.select(Some(0));
    }

    pub(crate) fn sort_badge(&self) -> String {
        let field = match self.sort_field {
            SortField::Size => "Size",
            SortField::Name => "Name",
            SortField::Count => "Count",
        };
        let arrow = match self.sort_dir {
            SortDirection::Desc => "↓",
            SortDirection::Asc => "↑",
        };
        format!("Sort: {field} {arrow}")
    }

    pub(crate) fn toggle_mark_selected(&mut self) {
        let target_path = {
            let items = self.get_active_items();
            self.list_state
                .selected()
                .and_then(|i| items.get(i))
                .map(|n| n.path.clone())
        };
        if let Some(path) = target_path
            && !self.marked_paths.remove(&path) {
                self.marked_paths.insert(path);
            }
    }

    pub(crate) fn marked_summary(&self) -> (usize, u64) {
        let mut count = 0usize;
        let mut bytes = 0u64;
        for path in &self.marked_paths {
            if let Some(node) = self.root_node.find_node(path) {
                count += 1;
                bytes += node.size;
            }
        }
        (count, bytes)
    }

    pub(crate) fn copy_selected_path(&mut self) {
        let target_path = {
            let items = self.get_active_items();
            self.list_state
                .selected()
                .and_then(|i| items.get(i))
                .map(|n| n.path.to_string_lossy().to_string())
        };
        if let Some(path_str) = target_path {
            copy_to_clipboard(&path_str);
            self.status_notification = Some((format!("Copied path: {path_str}"), Instant::now()));
        }
    }

    pub(crate) fn request_delete(&mut self, permanent: bool) {
        let candidate_nodes: Vec<DeleteTarget> = if !self.marked_paths.is_empty() {
            let mut list = Vec::new();
            for path in &self.marked_paths {
                if let Some(n) = self.root_node.find_node(path) {
                    list.push(DeleteTarget {
                        name: n.name.clone(),
                        path: n.path.clone(),
                        size: n.size,
                        is_dir: n.is_dir,
                        file_count: n.file_count,
                        dir_count: n.dir_count,
                        is_system: false,
                    });
                }
            }
            list.sort_by_key(|a| std::cmp::Reverse(a.size));
            list
        } else {
            let items = self.get_active_items();
            if let Some(selected) = self.list_state.selected() {
                if let Some(target) = items.get(selected) {
                    vec![DeleteTarget {
                        name: target.name.clone(),
                        path: target.path.clone(),
                        size: target.size,
                        is_dir: target.is_dir,
                        file_count: target.file_count,
                        dir_count: target.dir_count,
                        is_system: false,
                    }]
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };

        if candidate_nodes.is_empty() {
            return;
        }

        let mut verified = Vec::with_capacity(candidate_nodes.len());
        let mut total_size = 0u64;
        let mut any_system = false;

        for mut item in candidate_nodes {
            match check_deletion_permission(&item.path, &self.root_path) {
                Err(DeletionRestriction::RootFilesystem) => {
                    self.modal = Some(Modal::Message(MessageDialog {
                        title: "Action Forbidden".into(),
                        message:
                            "The root directory '/' cannot be deleted under any circumstances!"
                                .into(),
                        is_error: true,
                    }));
                    return;
                }
                Err(DeletionRestriction::CurrentScanRoot) => {
                    self.modal = Some(Modal::Message(MessageDialog {
                        title: "Action Forbidden".into(),
                        message: "Cannot delete the active scan root while viewing it!".into(),
                        is_error: true,
                    }));
                    return;
                }
                Err(DeletionRestriction::SystemProtected { .. }) => {
                    self.modal = Some(Modal::SystemProtected(SystemProtectedDialog {
                        target_name: item.name,
                        target_path: item.path,
                    }));
                    return;
                }
                Ok(is_sys) => {
                    item.is_system = is_sys;
                    if is_sys {
                        any_system = true;
                    }
                    total_size += item.size;
                    verified.push(item);
                }
            }
        }

        self.modal = Some(Modal::DeleteConfirm(DeleteConfirmDialog {
            targets: verified,
            total_size,
            permanent,
            any_system,
        }));
    }

    /// Kept for unit test compatibility (defaults to permanent delete in unit test or trash)
    #[allow(dead_code)]
    pub(crate) fn request_delete_selected(&mut self) {
        self.request_delete(true);
    }

    pub(crate) fn execute_deletion(&mut self) {
        if let Some(Modal::DeleteConfirm(dialog)) = self.modal.take() {
            let mut freed_bytes = 0u64;
            let mut deleted_count = 0usize;
            let mut first_name = String::new();
            let mut last_error: Option<String> = None;

            for target in &dialog.targets {
                if first_name.is_empty() {
                    first_name = target.name.clone();
                }

                let res = if dialog.permanent {
                    if target.is_dir {
                        let is_symlink = target
                            .path
                            .symlink_metadata()
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false);
                        if is_symlink {
                            std::fs::remove_file(&target.path)
                        } else {
                            std::fs::remove_dir_all(&target.path)
                        }
                    } else {
                        std::fs::remove_file(&target.path)
                    }
                } else {
                    move_to_trash(&target.path).map(|_| ())
                };

                match res {
                    Ok(()) => {
                        self.root_node.remove_node(&target.path, &self.root_path);
                        self.marked_paths.remove(&target.path);
                        freed_bytes += target.size;
                        deleted_count += 1;
                    }
                    Err(err) => {
                        last_error = Some(format!(
                            "Could not delete '{}':\n{}",
                            target.path.display(),
                            err
                        ));
                    }
                }
            }

            let remaining = self.get_active_items();
            if remaining.is_empty() {
                self.list_state.select(None);
            } else if let Some(sel) = self.list_state.selected()
                && sel >= remaining.len() {
                    self.list_state
                        .select(Some(remaining.len().saturating_sub(1)));
                }

            self.disks.refresh(true);

            if let Some(err_msg) = last_error {
                self.modal = Some(Modal::Message(MessageDialog {
                    title: "Deletion Failed".into(),
                    message: err_msg,
                    is_error: true,
                }));
            } else if deleted_count > 0 {
                let action = if dialog.permanent {
                    "Permanently deleted"
                } else {
                    "Moved to Trash"
                };
                let target_desc = if deleted_count == 1 {
                    format!("'{first_name}'")
                } else {
                    format!("{deleted_count} items")
                };
                self.status_notification = Some((
                    format!(
                        "{action} {target_desc} · Freed {}",
                        format_size(freed_bytes)
                    ),
                    Instant::now(),
                ));
            }
        }
    }

    pub(crate) fn active_disk_usage(&self) -> Option<(String, u64, u64, f64)> {
        let mut best_match: Option<(&sysinfo::Disk, usize)> = None;
        for disk in &self.disks {
            let mount = disk.mount_point();
            if self.current_path.starts_with(mount) {
                let len = mount.as_os_str().len();
                if best_match.is_none_or(|(_, best_len)| len > best_len) {
                    best_match = Some((disk, len));
                }
            }
        }
        let (disk, _) = best_match.or_else(|| self.disks.iter().next().map(|d| (d, 0)))?;
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let mount_str = disk.mount_point().to_string_lossy().to_string();
        Some((mount_str, used, total, pct))
    }
}
