//! Unit tests for app logic and rendering.

use ratatui::Terminal;
use std::path::PathBuf;

use crate::app::*;
use crate::safety::is_superuser;
use crate::theme::{IconMode, Theme};
use crate::ui::*;

fn test_app(root: PathBuf) -> App {
    App::new(
        root,
        Theme::arch_ice_blue(),
        IconMode::Ascii,
        false,
        Vec::new(),
    )
}

#[test]
fn test_app_delete_file_and_folder_flow() {
    let temp_dir = std::env::temp_dir().join("arch_disk_tui_app_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let file_a = temp_dir.join("test_file.txt");
    std::fs::write(&file_a, b"hello world").unwrap();

    let sub_dir = temp_dir.join("my_subfolder");
    std::fs::create_dir_all(&sub_dir).unwrap();
    let file_b = sub_dir.join("nested.bin");
    std::fs::write(&file_b, b"1234567890").unwrap();

    let mut app = test_app(temp_dir.clone());
    app.root_node.insert(&file_a, 11, false, &temp_dir);
    app.root_node.insert(&sub_dir, 10, true, &temp_dir);
    app.root_node.insert(&file_b, 10, false, &temp_dir);

    let items = app.get_filtered_sorted_children();
    assert_eq!(items.len(), 2);

    let file_idx = items
        .iter()
        .position(|i| i.name == "test_file.txt")
        .unwrap();
    app.list_state.select(Some(file_idx));

    app.request_delete_selected();
    assert!(
        matches!(app.modal, Some(Modal::DeleteConfirm(ref d)) if d.targets.len() == 1 && d.targets[0].name == "test_file.txt")
    );

    app.execute_deletion();
    assert!(!file_a.exists());
    assert!(app.modal.is_none());
    assert!(app.status_notification.is_some());

    let remaining = app.get_filtered_sorted_children();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].name, "my_subfolder");
    app.list_state.select(Some(0));

    app.request_delete_selected();
    assert!(
        matches!(app.modal, Some(Modal::DeleteConfirm(ref d)) if d.targets[0].name == "my_subfolder" && d.targets[0].is_dir)
    );

    app.execute_deletion();
    assert!(!sub_dir.exists());
    assert!(!file_b.exists());
    assert!(app.modal.is_none());
    assert_eq!(app.get_filtered_sorted_children().len(), 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cursor_memory_on_back_navigation() {
    let root_path = PathBuf::from("/home/user");
    let mut app = test_app(root_path.clone());
    app.root_node.insert(
        &PathBuf::from("/home/user/alpha/a.txt"),
        1000,
        false,
        &root_path,
    );
    app.root_node.insert(
        &PathBuf::from("/home/user/beta/b.txt"),
        500,
        false,
        &root_path,
    );
    app.root_node.insert(
        &PathBuf::from("/home/user/gamma/c.txt"),
        100,
        false,
        &root_path,
    );

    // Select second item ("beta")
    app.list_state.select(Some(1));
    app.enter_dir();
    assert_eq!(app.current_path, PathBuf::from("/home/user/beta"));

    // Exit back to parent -> should restore cursor on "beta" (index 1), not reset to 0!
    app.exit_dir();
    assert_eq!(app.current_path, root_path);
    assert_eq!(app.list_state.selected(), Some(1));
}

#[test]
fn test_sort_cycle_and_reverse() {
    let root_path = PathBuf::from("/home/user");
    let mut app = test_app(root_path.clone());
    app.root_node
        .insert(&PathBuf::from("/home/user/b.txt"), 100, false, &root_path);
    app.root_node
        .insert(&PathBuf::from("/home/user/a.txt"), 500, false, &root_path);

    assert_eq!(app.sort_badge(), "Sort: Size ↓");
    assert_eq!(app.get_active_items()[0].name, "a.txt");

    app.reverse_sort();
    assert_eq!(app.sort_badge(), "Sort: Size ↑");
    assert_eq!(app.get_active_items()[0].name, "b.txt");

    app.cycle_sort();
    assert_eq!(app.sort_badge(), "Sort: Name ↑");
    assert_eq!(app.get_active_items()[0].name, "a.txt");
}

#[test]
fn test_multi_select_batch_trash() {
    let _guard = crate::trash::TEST_ENV_LOCK.lock().unwrap();
    let temp_dir = std::env::temp_dir().join("spacesage_batch_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    unsafe {
        std::env::set_var("XDG_DATA_HOME", temp_dir.join("trash_xdg"));
    }

    let f1 = temp_dir.join("one.bin");
    let f2 = temp_dir.join("two.bin");
    std::fs::write(&f1, b"1111").unwrap();
    std::fs::write(&f2, b"22222222").unwrap();

    let mut app = test_app(temp_dir.clone());
    app.root_node.insert(&f1, 4, false, &temp_dir);
    app.root_node.insert(&f2, 8, false, &temp_dir);

    app.list_state.select(Some(0));
    app.toggle_mark_selected();
    app.list_state.select(Some(1));
    app.toggle_mark_selected();

    assert_eq!(app.marked_summary(), (2, 12));

    // Request trash (permanent = false)
    app.request_delete(false);
    assert!(
        matches!(app.modal, Some(Modal::DeleteConfirm(ref d)) if d.targets.len() == 2 && !d.permanent)
    );
    app.execute_deletion();
    assert!(!f1.exists());
    assert!(!f2.exists());
    assert!(app.marked_paths.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_app_system_file_deletion_blocked_for_regular_user() {
    let scan_root = PathBuf::from("/etc");
    let mut app = test_app(scan_root.clone());
    let system_file = PathBuf::from("/etc/fstab");
    app.root_node.insert(&system_file, 100, false, &scan_root);

    let items = app.get_filtered_sorted_children();
    assert_eq!(items.len(), 1);
    app.list_state.select(Some(0));

    app.request_delete_selected();

    if !is_superuser() {
        assert!(
            matches!(app.modal, Some(Modal::SystemProtected(ref d)) if d.target_name == "fstab")
        );
    } else {
        assert!(
            matches!(app.modal, Some(Modal::DeleteConfirm(ref d)) if d.targets[0].name == "fstab" && d.any_system)
        );
    }
}

fn buffer_row(buf: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    (0..buf.area.height)
        .map(|y| buffer_row(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn app_with_files(count: usize) -> App {
    let root_path = PathBuf::from("/home/user");
    let mut app = test_app(root_path.clone());
    for i in 0..count {
        let p = PathBuf::from(format!("/home/user/item{i:02}/file.bin"));
        app.root_node
            .insert(&p, 10_000 - (i as u64) * 100, false, &root_path);
    }
    app.scanning = false;
    app
}

#[test]
fn test_treemap_is_fullscreen_page() {
    let mut app = app_with_files(8);
    app.view_mode = ViewMode::Treemap;

    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| render_ui(f, &mut app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let text = buffer_text(&buf);

    // The dashboard chrome (header, drives panel, analytics) is gone...
    assert!(!text.contains("ARCH·DISK"), "header should be hidden");
    // ...the treemap owns the top row, and only the footer is left below it.
    assert!(buffer_row(&buf, 0).contains("Treemap"));
    assert!(buffer_row(&buf, 39).contains("list view"));

    // Border (1 cell) around the page: inner area is the full terminal minus
    // the border and the 1-line footer.
    let inner = app.explorer_inner_area;
    assert_eq!(
        (inner.x, inner.y, inner.width, inner.height),
        (1, 1, 118, 37)
    );

    // The blocks tile the entire inner area, with no gaps or overlaps.
    assert_eq!(app.treemap_rects.len(), 8);
    let covered: u32 = app
        .treemap_rects
        .iter()
        .map(|(r, _)| r.width as u32 * r.height as u32)
        .sum();
    assert_eq!(covered, 118 * 37);
    for (r, _) in &app.treemap_rects {
        assert!(r.x >= inner.x && r.x + r.width <= inner.x + inner.width);
        assert!(r.y >= inner.y && r.y + r.height <= inner.y + inner.height);
    }
}

#[test]
fn test_treemap_navigation_stays_on_drawn_blocks() {
    let mut app = app_with_files(60);
    app.view_mode = ViewMode::Treemap;

    app.jump_bottom();
    assert_eq!(app.list_state.selected(), Some(TREEMAP_MAX_BLOCKS - 1));
    app.next_item();
    assert_eq!(app.list_state.selected(), Some(0));
    app.previous_item();
    assert_eq!(app.list_state.selected(), Some(TREEMAP_MAX_BLOCKS - 1));

    // A cursor left deep in the list view is pulled onto a drawn block.
    app.view_mode = ViewMode::List;
    app.list_state.select(Some(55));
    app.view_mode = ViewMode::Treemap;
    app.clamp_selection();
    assert_eq!(app.list_state.selected(), Some(TREEMAP_MAX_BLOCKS - 1));

    // The list view still reaches every item.
    app.view_mode = ViewMode::List;
    app.jump_bottom();
    assert_eq!(app.list_state.selected(), Some(59));
}

#[test]
fn test_list_view_keeps_dashboard_layout() {
    let mut app = app_with_files(8);
    app.view_mode = ViewMode::List;

    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| render_ui(f, &mut app)).unwrap();
    let text = buffer_text(terminal.backend().buffer());

    assert!(
        text.contains("ARCH·DISK"),
        "header should be visible in list view"
    );
    assert!(app.treemap_rects.is_empty());
}
