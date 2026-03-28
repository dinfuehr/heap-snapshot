use super::children::{compute_edges, compute_retainers, shifted_window_start};
use super::*;
use crate::types::{Distance, RawHeapSnapshot, SnapshotHeader, SnapshotMeta};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs::File;
use std::rc::Rc;

fn standard_snapshot_meta() -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let node_fields = ["type", "name", "id", "self_size", "edge_count"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let node_type_enum = [
        "hidden",
        "array",
        "string",
        "object",
        "code",
        "closure",
        "regexp",
        "number",
        "native",
        "synthetic",
        "concatenated string",
        "sliced string",
        "symbol",
        "bigint",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let edge_fields = ["type", "name_or_index", "to_node"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let edge_type_enum = [
        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    (node_fields, node_type_enum, edge_fields, edge_type_enum)
}

fn build_snapshot(strings: Vec<String>, nodes: Vec<u32>, edges: Vec<u32>) -> HeapSnapshot {
    let (node_fields, node_type_enum, edge_fields, edge_type_enum) = standard_snapshot_meta();
    let nfc = node_fields.len();
    let efc = edge_fields.len();
    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields,
                node_type_enum,
                edge_fields,
                edge_type_enum,
                location_fields: vec![],
                sample_fields: vec![],
                trace_function_info_fields: vec![],
                trace_node_fields: vec![],
            },
            node_count: nodes.len() / nfc,
            edge_count: edges.len() / efc,
            trace_function_count: 0,
            root_index: Some(0),
            extra_native_bytes: None,
        },
        nodes,
        edges,
        strings,
        locations: vec![],
    };

    HeapSnapshot::new(raw)
}

fn load_test_snapshot(path: &str) -> HeapSnapshot {
    let file = File::open(path).unwrap();
    let raw = crate::parser::parse(file).unwrap();
    HeapSnapshot::new(raw)
}

fn find_row_index_by_ordinal(app: &App, ordinal: NodeOrdinal) -> usize {
    app.cached_rows
        .iter()
        .position(|row| row.node_ordinal() == Some(ordinal))
        .unwrap()
}

fn row_identity(app: &App, ordinal: NodeOrdinal) -> (NodeId, Option<ChildrenKey>) {
    let row = app
        .cached_rows
        .iter()
        .find(|row| row.node_ordinal() == Some(ordinal))
        .unwrap();
    (row.nav.id, row.nav.children_key.clone())
}

fn collect_reachable_work(work_rx: &mpsc::Receiver<WorkItem>) -> Vec<NodeOrdinal> {
    let mut queued = Vec::new();
    while let Ok(item) = work_rx.try_recv() {
        match item {
            WorkItem::ReachableSize(ordinal) => queued.push(ordinal),
            WorkItem::RetainerPlan(_) => panic!("unexpected retainer plan work item"),
            WorkItem::ExtensionName(_) => {} // ignore extension lookups in tests
        }
    }
    queued
}

fn make_js_global_snapshot() -> HeapSnapshot {
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "Window (global*)",
        "Window (global)",
        "value_a",
        "value_b",
        "value_c",
        "value_d",
        "value_e",
        "value_f",
        "gobj1",
        "gobj2",
        "gproxy1",
        "gproxy2",
        "shared_a",
        "specific_obj_a",
        "shared_b",
        "specific_obj_b",
        "proxy_shared_a",
        "specific_proxy_a",
        "proxy_shared_b",
        "specific_proxy_b",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, 9, 1, 2, 0, 4, 3, 2, 3, 10, 3, 3, 2, 5, 10, 3, 3, 3, 7, 10, 3, 3, 3, 9, 10,
        3, 2, 4, 11, 1, 0, 2, 5, 13, 1, 0, 2, 6, 15, 1, 0, 2, 7, 17, 1, 0, 2, 8, 19, 1, 0, 2, 9,
        21, 1, 0,
    ];

    let node_index = |ordinal: u32| ordinal * 5;
    let edges: Vec<u32> = vec![
        1,
        0,
        node_index(1),
        2,
        10,
        node_index(2),
        2,
        11,
        node_index(3),
        2,
        12,
        node_index(4),
        2,
        13,
        node_index(5),
        2,
        14,
        node_index(6),
        2,
        15,
        node_index(7),
        2,
        16,
        node_index(8),
        2,
        14,
        node_index(9),
        2,
        17,
        node_index(10),
        2,
        16,
        node_index(11),
        2,
        18,
        node_index(6),
        2,
        19,
        node_index(7),
        2,
        20,
        node_index(8),
        2,
        18,
        node_index(9),
        2,
        21,
        node_index(10),
        2,
        20,
        node_index(11),
    ];

    build_snapshot(strings, nodes, edges)
}

fn make_many_retainers_snapshot(retainer_count: usize) -> HeapSnapshot {
    let mut strings: Vec<String> = vec![
        "".to_string(),
        "(GC roots)".to_string(),
        "target".to_string(),
        "Holder".to_string(),
        "ref".to_string(),
    ];
    for i in 0..retainer_count {
        strings.push(format!("holder_{i}"));
    }

    let mut nodes: Vec<u32> = vec![
        9,
        0,
        1,
        0,
        1, // synthetic root
        9,
        1,
        2,
        0,
        retainer_count as u32, // GC roots
        2,
        2,
        3,
        1,
        0, // target string
    ];
    for i in 0..retainer_count {
        nodes.extend_from_slice(&[3, 3, (4 + i) as u32, 1, 1]);
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut edges: Vec<u32> = vec![1, 0, node_index(1)];
    for i in 0..retainer_count {
        edges.extend_from_slice(&[2, (5 + i) as u32, node_index(3 + i)]);
    }
    for _ in 0..retainer_count {
        edges.extend_from_slice(&[2, 4, node_index(2)]);
    }

    build_snapshot(strings, nodes, edges)
}

fn make_nested_retainers_snapshot(extra_retainers: usize) -> HeapSnapshot {
    let mut strings = vec![
        "".to_string(),
        "(GC roots)".to_string(),
        "target".to_string(),
        "Holder".to_string(),
        "RootHolder".to_string(),
        "DetachedHolder".to_string(),
        "gc_root".to_string(),
        "ref".to_string(),
        "off_path".to_string(),
    ];

    let mut nodes = vec![
        9,
        0,
        1,
        0,
        (2 + extra_retainers) as u32, // synthetic root
        9,
        1,
        2,
        0,
        1, // (GC roots)
        2,
        2,
        3,
        1,
        0, // target
        3,
        3,
        4,
        1,
        1, // holder_main
        3,
        4,
        5,
        1,
        1, // root_holder
        3,
        5,
        6,
        1,
        1, // detached_holder
    ];
    for i in 0..extra_retainers {
        nodes.extend_from_slice(&[3, 5, (7 + i) as u32, 1, 1]);
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut root_edges = vec![
        1,
        0,
        node_index(1), // root -> (GC roots)
        2,
        8,
        node_index(5), // root -> detached_holder
    ];
    let mut edges = vec![
        2,
        6,
        node_index(4), // (GC roots) -> root_holder
        2,
        7,
        node_index(2), // holder_main -> target
        2,
        6,
        node_index(3), // root_holder -> holder_main
        2,
        8,
        node_index(3), // detached_holder -> holder_main
    ];
    for i in 0..extra_retainers {
        let root_name_idx = strings.len() as u32;
        strings.push(format!("synthetic_extra_{i}"));
        root_edges.extend_from_slice(&[2, root_name_idx, node_index(6 + i)]);

        let edge_name_idx = strings.len() as u32;
        strings.push(format!("extra_retainer_{}", edge_name_idx));
        edges.extend_from_slice(&[2, edge_name_idx, node_index(3)]);
    }
    root_edges.append(&mut edges);

    build_snapshot(strings, nodes, root_edges)
}

fn make_many_edges_snapshot(edge_count: usize) -> HeapSnapshot {
    let mut strings = vec!["".to_string(), "(GC roots)".to_string(), "Leaf".to_string()];
    let mut nodes = vec![
        9,
        0,
        1,
        0,
        (edge_count + 1) as u32, // synthetic root
        9,
        1,
        2,
        0,
        0, // (GC roots)
    ];
    for i in 0..edge_count {
        nodes.extend_from_slice(&[3, 2, (3 + i) as u32, 1, 0]);
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut edges = vec![1, 0, node_index(1)];
    for i in 0..edge_count {
        let edge_name_idx = strings.len() as u32;
        let edge_name = if i % 2 == 0 {
            format!("match_{i}")
        } else {
            format!("skip_{i}")
        };
        strings.push(edge_name);
        edges.extend_from_slice(&[2, edge_name_idx, node_index(2 + i)]);
    }

    build_snapshot(strings, nodes, edges)
}

fn make_duplicate_children_snapshot() -> HeapSnapshot {
    let strings = vec![
        "".to_string(),
        "(GC roots)".to_string(),
        "Container".to_string(),
        "ChildA".to_string(),
        "ChildB".to_string(),
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
        "container".to_string(),
    ];
    let nodes = vec![
        9, 0, 1, 0, 2, // synthetic root
        9, 1, 2, 0, 0, // (GC roots)
        3, 2, 3, 1, 3, // container
        3, 3, 4, 1, 0, // child_a
        3, 4, 5, 1, 0, // child_b
    ];
    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let edges = vec![
        1,
        0,
        node_index(1),
        2,
        8,
        node_index(2),
        2,
        5,
        node_index(3),
        2,
        6,
        node_index(3),
        2,
        7,
        node_index(4),
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_shift_forward_to_partial_last_page() {
    // 16 items, showing 1-10, press n → show 11-16
    assert_eq!(shifted_window_start(0, 10, 16, 10), 10);
}

#[test]
fn test_shift_forward_no_effect_on_last_page() {
    // 16 items, showing 11-16, press n → stay at 11-16
    assert_eq!(shifted_window_start(10, 10, 16, 10), 10);
}

#[test]
fn test_shift_backward_from_last_page() {
    // 16 items, showing 11-16, press p → show 1-10
    assert_eq!(shifted_window_start(10, 10, 16, -10), 0);
}

#[test]
fn test_shift_backward_no_effect_at_start() {
    // Already at start, press p → stay at 0
    assert_eq!(shifted_window_start(0, 10, 16, -10), 0);
}

#[test]
fn test_shift_forward_exact_fit() {
    // 20 items, count=10 — two exact pages
    assert_eq!(shifted_window_start(0, 10, 20, 10), 10);
    // Second page is full, press n → no change
    assert_eq!(shifted_window_start(10, 10, 20, 10), 10);
}

#[test]
fn test_shift_all_items_visible() {
    // 5 items, count=10 — everything visible, n has no effect
    assert_eq!(shifted_window_start(0, 10, 5, 10), 0);
}

#[test]
fn test_shift_three_pages() {
    // 30 items, count=10 — three exact pages
    assert_eq!(shifted_window_start(0, 10, 30, 10), 10);
    assert_eq!(shifted_window_start(10, 10, 30, 10), 20);
    assert_eq!(shifted_window_start(20, 10, 30, 10), 20); // last page, no change
}

#[test]
fn test_shift_empty() {
    // 0 items — no effect
    assert_eq!(shifted_window_start(0, 10, 0, 10), 0);
    assert_eq!(shifted_window_start(0, 10, 0, -10), 0);
}

#[test]
fn test_reachable_result_invalidates_rows_and_updates_render_data() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, _work_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Dominators;
    app.rebuild_rows(&snap);
    assert!(!app.rows_dirty);

    let root_ord = snap.gc_roots_ordinal();
    let root_row = app.cached_rows.first().unwrap();
    match &root_row.render.kind {
        FlatRowKind::HeapNode { reachable_size, .. } => assert_eq!(*reachable_size, None),
        _ => panic!("expected heap node row"),
    }

    result_tx
        .send(WorkResult::ReachableSize {
            ordinal: root_ord,
            size: 1234.0,
        })
        .unwrap();
    assert!(app.drain_results(&snap));
    // Reachable size is now patched in-place without marking rows dirty.
    assert!(!app.rows_dirty);

    let root_row = app.cached_rows.first().unwrap();
    match &root_row.render.kind {
        FlatRowKind::HeapNode { reachable_size, .. } => {
            assert_eq!(*reachable_size, Some(1234.0))
        }
        _ => panic!("expected heap node row"),
    }
}

#[test]
fn test_ctrl_f_and_ctrl_b_scroll_help_like_page_down_and_page_up() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Help;
    app.help_state.scroll_offset = 25;
    app.help_state.page_height = 7;

    app.handle_normal_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        &snap,
    );
    assert_eq!(app.help_state.scroll_offset, 19);

    app.handle_normal_key(
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        &snap,
    );
    assert_eq!(app.help_state.scroll_offset, 25);
}

#[test]
fn test_page_down_and_up_use_viewport_height_with_one_row_overlap() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Summary;
    app.rebuild_rows(&snap);
    assert!(app.cached_rows.len() > 3);

    app.summary_state.cursor = 1;
    app.summary_state.scroll_offset = 0;
    app.summary_state.page_height = 3;

    app.handle_normal_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &snap);
    assert_eq!(app.summary_state.scroll_offset, 2);
    assert_eq!(app.summary_state.cursor, 3);

    app.handle_normal_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &snap);
    assert_eq!(app.summary_state.scroll_offset, 0);
    assert_eq!(app.summary_state.cursor, 1);
}

#[test]
fn test_ctrl_f_does_not_open_edge_filter() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), &snap);
    assert!(matches!(app.input_mode, InputMode::EdgeFilter));

    app.input_mode = InputMode::Normal;
    app.handle_normal_key(
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        &snap,
    );
    assert!(matches!(app.input_mode, InputMode::Normal));
}

#[test]
fn test_compute_edges_lists_specific_js_global_fields_before_common_fields() {
    let snap = make_js_global_snapshot();
    let next_id = Cell::new(1);

    let object_children = compute_edges(&snap, NodeOrdinal(2), EdgeWindow::default(), "", &next_id);
    assert!(object_children[0].label.starts_with("specific_obj_a :: "));
    assert!(object_children[1].label.starts_with("shared_a :: "));
    assert!(object_children[2].label.starts_with("shared_b :: "));

    let proxy_children = compute_edges(&snap, NodeOrdinal(4), EdgeWindow::default(), "", &next_id);
    assert!(proxy_children[0].label.starts_with("specific_proxy_a :: "));
    assert!(proxy_children[1].label.starts_with("proxy_shared_a :: "));
    assert!(proxy_children[2].label.starts_with("proxy_shared_b :: "));
}

#[test]
fn test_opening_contexts_queues_reachable_for_all_native_contexts() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE), &snap);

    let mut queued = Vec::new();
    while let Ok(item) = work_rx.try_recv() {
        match item {
            WorkItem::ReachableSize(ordinal) => queued.push(ordinal),
            WorkItem::RetainerPlan(_) => panic!("unexpected retainer plan work item"),
            WorkItem::ExtensionName(_) => {} // ignore extension lookups in tests
        }
    }

    let expected: Vec<NodeOrdinal> = snap
        .native_contexts()
        .iter()
        .copied()
        .map(NodeOrdinal)
        .collect();
    assert!(matches!(app.current_view, ViewType::Contexts));
    assert_eq!(queued, expected);
}

#[test]
fn test_opening_retainers_only_caches_first_page() {
    let snap = make_many_retainers_snapshot(25);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);

    let children = app
        .retainers
        .tree_state
        .children_map
        .iter()
        .find(|(k, _)| matches!(k, ChildrenKey::Retainers(_, ord) if *ord == NodeOrdinal(2)))
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!(children.len(), EDGE_PAGE_SIZE + 1);
    assert!(children[0].label.starts_with("ref in Holder @"));
    assert_eq!(
        children.last().unwrap().label,
        Rc::<str>::from("1–20 of 25 retainers  (n/p: page, a: all)")
    );
}

#[test]
fn test_pressing_a_on_retainers_status_line_shows_all() {
    let snap = make_many_retainers_snapshot(25);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    app.rebuild_rows(&snap);

    // Move cursor to the status line (last row).
    let status_idx = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("of 25 retainers"))
        .expect("status line should exist");
    app.retainers.tree_state.cursor = status_idx;

    // Press 'a' to show all retainers.
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    // All 25 retainers should now be visible (plus a status line showing the full range).
    let retainer_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.node_ordinal().is_some())
        .collect();
    assert_eq!(retainer_rows.len(), 25, "all 25 retainers should be visible");

    // No "(n/p: page, a: all)" hint should remain.
    let has_paging_hint = app
        .cached_rows
        .iter()
        .any(|r| r.render.label.contains("n/p: page"));
    assert!(
        !has_paging_hint,
        "paging hint should be gone after showing all"
    );
}

#[test]
fn test_pressing_a_on_selected_of_status_line_shows_all_retainers() {
    // After auto-expansion, an intermediate node shows "1 selected of 2 retainers".
    // Pressing 'a' on that status line should show all retainers for that node.
    let snap = make_nested_retainers_snapshot(0);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 8,
            max_nodes: 64,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // Find the "selected of" status line and move cursor there.
    let status_idx = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("1 selected of 2 retainers"))
        .expect("'selected of' status line should exist");
    app.retainers.tree_state.cursor = status_idx;

    // Press 'a' to show all retainers for the intermediate node.
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    // The "selected of" status line should be gone.
    let has_selected = app
        .cached_rows
        .iter()
        .any(|r| r.render.label.contains("selected of"));
    assert!(
        !has_selected,
        "'selected of' status line should be gone after pressing 'a'"
    );
}

#[test]
fn test_plan_tree_uses_retainer_path_keys() {
    // With the tree-based retainer plan, auto-expanded nodes use
    // RetainerPath(NodeId) keys so each occurrence gets unique children.
    // Only GC-root-reachable paths appear in the plan tree.
    let snap = make_nested_retainers_snapshot(0);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 8,
            max_nodes: 64,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // Node 3 (Holder) should have a RetainerPath key (not Retainers)
    // because it was auto-expanded from the plan tree.
    let (_, holder_key) = row_identity(&app, NodeOrdinal(3));
    assert!(
        matches!(&holder_key, Some(ChildrenKey::Retainers(..))),
        "auto-expanded plan node should use Retainers key"
    );

    // Only RootHolder is on the GC-root path (DetachedHolder is off-path,
    // connected only via the synthetic root, not (GC roots)).
    // A status line is appended because only 1 of 2 retainers is selected.
    let holder_children = app
        .retainers
        .tree_state
        .children_map
        .get(holder_key.as_ref().unwrap())
        .unwrap();
    assert_eq!(holder_children.len(), 2);
    assert!(holder_children[0].label.contains("RootHolder"));
    assert!(
        holder_children[1]
            .label
            .contains("1 selected of 2 retainers"),
        "expected status line, got: {:?}",
        holder_children[1].label,
    );

    // Manual expand on a Retainers-keyed node at the plan tree leaf
    // should show unfiltered retainers (bypass gc_root_path_edges).
    let leaf_info = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("RootHolder"))
        .map(|r| (r.nav.id, r.nav.children_key.clone()));
    if let Some((id, Some(ChildrenKey::Retainers(nid, ord)))) = leaf_info {
        app.expand(id, Some(ChildrenKey::Retainers(nid, ord)), &snap);
        assert!(app.retainers.unfiltered_nodes.contains(&nid));
    }
}

#[test]
fn test_nested_retainer_paging_updates_child_window_only() {
    let snap = make_nested_retainers_snapshot(24);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    app.rebuild_rows(&snap);
    let (holder_id, holder_key) = row_identity(&app, NodeOrdinal(3));
    let holder_key_id = match &holder_key {
        Some(ChildrenKey::Retainers(id, _)) => *id,
        _ => panic!("expected Retainers key"),
    };
    app.expand(holder_id, holder_key, &snap);
    app.rebuild_rows(&snap);

    app.retainers.tree_state.cursor = find_row_index_by_ordinal(&app, NodeOrdinal(3));
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);

    let holder_window = app
        .retainers
        .tree_state
        .edge_windows
        .get(&holder_key_id)
        .copied()
        .unwrap();
    assert_eq!(holder_window.start, EDGE_PAGE_SIZE);
    // The target node's root_id should NOT have a paging window
    assert!(!app.retainers.root_id.map_or(true, |rid| {
        app.retainers.tree_state.edge_windows.contains_key(&rid)
    }));

    let children = app
        .retainers
        .tree_state
        .children_map
        .iter()
        .find(|(k, _)| matches!(k, ChildrenKey::Retainers(_, ord) if *ord == NodeOrdinal(3)))
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!(
        children.last().unwrap().label,
        Rc::<str>::from("21–26 of 26 retainers  (n/p: page, a: all)")
    );
}

#[test]
fn test_nested_retainer_page_shift_clamps_cursor_on_partial_last_page() {
    let snap = make_nested_retainers_snapshot(24);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    app.rebuild_rows(&snap);
    let (holder_id, holder_key) = row_identity(&app, NodeOrdinal(3));
    app.expand(holder_id, holder_key, &snap);
    app.rebuild_rows(&snap);

    app.retainers.tree_state.cursor = find_row_index_by_ordinal(&app, NodeOrdinal(3));
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    let holder_row = find_row_index_by_ordinal(&app, NodeOrdinal(3));
    let last_child_before = app.subtree_end_index(holder_row);
    app.retainers.tree_state.cursor = last_child_before;

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);

    let holder_row = find_row_index_by_ordinal(&app, NodeOrdinal(3));
    let last_child_after = app.subtree_end_index(holder_row);
    assert_eq!(app.retainers.tree_state.cursor, last_child_after);

    let children = app
        .retainers
        .tree_state
        .children_map
        .iter()
        .find(|(k, _)| matches!(k, ChildrenKey::Retainers(_, ord) if *ord == NodeOrdinal(3)))
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!(
        children.last().unwrap().label,
        Rc::<str>::from("21–26 of 26 retainers  (n/p: page, a: all)")
    );
}

#[test]
fn test_reopening_contexts_does_not_requeue_cached_or_pending_reachable_work() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/globals.heapsnapshot"
    ));
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_view(ViewType::Contexts, &snap);
    let expected = collect_reachable_work(&work_rx);
    assert!(!expected.is_empty());

    let cached = expected[0];
    app.reachable_pending.remove(&cached);
    app.reachable_sizes.insert(cached, 42.0);

    app.set_view(ViewType::Summary, &snap);
    app.set_view(ViewType::Contexts, &snap);
    assert!(collect_reachable_work(&work_rx).is_empty());
}

#[test]
fn test_search_id_opening_retainers_keeps_retainer_cache_paged() {
    let snap = make_many_retainers_snapshot(25);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Search from containment view so @id opens retainers (summary view shows in summary instead)
    app.current_view = ViewType::Containment;
    app.input_mode = InputMode::Search;
    app.search_input = "@3".to_string();
    app.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &snap);

    assert!(matches!(app.current_view, ViewType::Retainers));
    let children = app
        .retainers
        .tree_state
        .children_map
        .iter()
        .find(|(k, _)| matches!(k, ChildrenKey::Retainers(_, ord) if *ord == NodeOrdinal(2)))
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!(children.len(), EDGE_PAGE_SIZE + 1);
    assert_eq!(
        children.last().unwrap().label,
        Rc::<str>::from("1–20 of 25 retainers  (n/p: page, a: all)")
    );
}

#[test]
fn test_edge_filter_resets_edge_paging_and_is_unavailable_in_retainers() {
    let snap = make_many_edges_snapshot(EDGE_PAGE_SIZE + 4);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Containment;
    let root_id = app.containment_root_id;
    app.containment_state.edge_windows.insert(
        root_id,
        EdgeWindow {
            start: EDGE_PAGE_SIZE,
            count: EDGE_PAGE_SIZE,
        },
    );
    app.recompute_paged_children(
        PagedChildrenParent::Edges {
            id: root_id,
            ordinal: snap.synthetic_root_ordinal(),
            is_compare: false,
        },
        &snap,
    );
    app.rebuild_rows(&snap);
    app.containment_state.cursor = 1;

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), &snap);
    assert!(matches!(app.input_mode, InputMode::EdgeFilter));
    app.edge_filter_input = "match".to_string();
    app.handle_edge_filter_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &snap);

    assert!(!app.containment_state.edge_windows.contains_key(&root_id));
    let children = app
        .containment_state
        .children_map
        .get(&ChildrenKey::Edges(
            app.containment_root_id,
            snap.synthetic_root_ordinal(),
        ))
        .unwrap();
    assert!(
        children
            .last()
            .unwrap()
            .label
            .contains("matching \"match\"")
    );

    let retainers_snap = make_many_retainers_snapshot(25);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut retainers_app = App::new(&retainers_snap, Vec::new(), work_tx, result_rx);
    retainers_app.set_retainers_target(NodeOrdinal(2), &retainers_snap);
    retainers_app.rebuild_rows(&retainers_snap);
    retainers_app.handle_normal_key(
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
        &retainers_snap,
    );
    assert!(matches!(retainers_app.input_mode, InputMode::Normal));
}

#[test]
fn test_reachable_all_deduplicates_duplicate_and_known_children() {
    let snap = make_duplicate_children_snapshot();
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);
    app.reachable_pending.insert(NodeOrdinal(4));
    app.containment_state.cursor = find_row_index_by_ordinal(&app, NodeOrdinal(2));

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE), &snap);

    assert_eq!(
        collect_reachable_work(&work_rx),
        vec![NodeOrdinal(2), NodeOrdinal(3)]
    );
}

/// Snapshot with 3 code nodes (grouped under "(compiled code)") and 1 object.
/// Code nodes have raw names: "SharedFunctionInfo", "SharedFunctionInfo", "BytecodeArray".
fn make_summary_filter_snapshot() -> HeapSnapshot {
    let strings: Vec<String> = [
        "",                   // 0
        "(GC roots)",         // 1
        "SharedFunctionInfo", // 2
        "BytecodeArray",      // 3
        "MyObj",              // 4
        "a",                  // 5
        "b",                  // 6
        "c",                  // 7
        "d",                  // 8
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    // Node types: code=4, object=3, synthetic=9
    let nodes = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 4, // node 1: (GC roots), 4 edges
        4, 2, 10, 100, 0, // node 2: code "SharedFunctionInfo", size=100
        4, 2, 11, 200, 0, // node 3: code "SharedFunctionInfo", size=200
        4, 3, 12, 150, 0, // node 4: code "BytecodeArray", size=150
        3, 4, 13, 300, 0, // node 5: object "MyObj", size=300
    ];
    let edges = vec![
        1,
        0,
        node_index(1), // root → GC roots
        2,
        5,
        node_index(2), // GC roots → SFI_1
        2,
        6,
        node_index(3), // GC roots → SFI_2
        2,
        7,
        node_index(4), // GC roots → BytecodeArray
        2,
        8,
        node_index(5), // GC roots → MyObj
    ];
    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_summary_filter_by_group_name_shows_all_members() {
    let snap = make_summary_filter_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Filter by group name "compiled code"
    app.summary_filter = "compiled code".to_string();
    app.rebuild_rows(&snap);

    // Only (compiled code) group should be visible (MyObj filtered out)
    let group_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .collect();
    assert_eq!(group_rows.len(), 1);
    assert!(group_rows[0].render.label.contains("(compiled code)"));
    // Full count: 3 code nodes
    assert!(group_rows[0].render.label.contains("\u{00d7}3"));

    // Expand the group
    let id = group_rows[0].nav.id;
    let ck = group_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // All 3 members should be visible (no member filtering)
    let member_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::HeapNode { .. }) && r.nav.depth == 1)
        .collect();
    assert_eq!(member_rows.len(), 3);
}

#[test]
fn test_summary_filter_by_member_name_filters_members() {
    let snap = make_summary_filter_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Filter by member name "sharedfunctioninfo" (lowercase)
    app.summary_filter = "sharedfunctioninfo".to_string();
    app.rebuild_rows(&snap);

    // (compiled code) group should appear (matched by member)
    let group_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .collect();
    assert_eq!(group_rows.len(), 1);
    assert!(group_rows[0].render.label.contains("(compiled code)"));
    // Filtered count: only 2 SharedFunctionInfo nodes, not 3
    assert!(
        group_rows[0].render.label.contains("\u{00d7}2"),
        "expected x2 in label: {}",
        group_rows[0].render.label
    );

    // Expand the group
    let id = group_rows[0].nav.id;
    let ck = group_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Only the 2 SharedFunctionInfo members should be shown
    let member_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| {
            matches!(
                r.render.kind,
                FlatRowKind::HeapNode {
                    node_ordinal: Some(_),
                    ..
                }
            ) && r.nav.depth == 1
        })
        .collect();
    assert_eq!(member_rows.len(), 2);
    for row in &member_rows {
        assert!(
            row.render.label.contains("SharedFunctionInfo"),
            "unexpected member: {}",
            row.render.label
        );
    }
}

#[test]
fn test_summary_filter_no_match_hides_group() {
    let snap = make_summary_filter_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.summary_filter = "nonexistent".to_string();
    app.rebuild_rows(&snap);

    assert!(app.cached_rows.is_empty());
}

#[test]
fn test_summary_filter_member_match_paged() {
    // Build a snapshot with many code nodes so filtered results get paged
    let count = (EDGE_PAGE_SIZE + 5) * 2;
    let mut strings: Vec<String> = vec![
        "".to_string(),
        "(GC roots)".to_string(),
        "TargetFunc".to_string(),
        "OtherCode".to_string(),
    ];
    for i in 0..count {
        strings.push(format!("e{i}"));
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut nodes = vec![
        9u32,
        0,
        1,
        0,
        1, // node 0: synthetic root
        9,
        1,
        2,
        0,
        count as u32, // node 1: (GC roots)
    ];
    for i in 0..count {
        // All code type (4), alternate names between TargetFunc(2) and OtherCode(3)
        let name_idx = if i % 2 == 0 { 2u32 } else { 3 };
        nodes.extend_from_slice(&[4, name_idx, (100 + i) as u32, 10, 0]);
    }

    let mut edges = vec![1u32, 0, node_index(1)];
    for i in 0..count {
        edges.extend_from_slice(&[2, (4 + i) as u32, node_index(2 + i)]);
    }

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Filter by "targetfunc" — should match ~half the code nodes
    app.summary_filter = "targetfunc".to_string();
    app.rebuild_rows(&snap);

    let group_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .collect();
    assert_eq!(group_rows.len(), 1);

    // Expand
    let id = group_rows[0].nav.id;
    let ck = group_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Should show at most EDGE_PAGE_SIZE members + 1 status line
    let depth1_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 1)
        .collect();
    // EDGE_PAGE_SIZE matching members + 1 "N of M matching" status row
    assert_eq!(depth1_rows.len(), EDGE_PAGE_SIZE + 1);
    // Last row is the status line
    assert!(
        depth1_rows
            .last()
            .unwrap()
            .render
            .label
            .contains("matching"),
        "expected paging status, got: {}",
        depth1_rows.last().unwrap().render.label
    );

    // Status line should show "1–EDGE_PAGE_SIZE of ..."
    let status = &depth1_rows.last().unwrap().render.label;
    assert!(
        status.starts_with(&format!("1\u{2013}{EDGE_PAGE_SIZE}")),
        "expected first page range, got: {status}"
    );

    // Move cursor to the group row so n/p can find the paged parent
    let group_idx = app
        .cached_rows
        .iter()
        .position(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .unwrap();
    app.summary_state.cursor = group_idx;

    // Press 'n' to advance to the next page
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    let depth1_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 1)
        .collect();
    // Second page: remaining matching members + status line
    let status = &depth1_rows.last().unwrap().render.label;
    assert!(
        status.starts_with(&format!("{}\u{2013}", EDGE_PAGE_SIZE + 1)),
        "expected second page range, got: {status}"
    );

    // Press 'p' to go back to the first page
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    let depth1_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 1)
        .collect();
    let status = &depth1_rows.last().unwrap().render.label;
    assert!(
        status.starts_with(&format!("1\u{2013}{EDGE_PAGE_SIZE}")),
        "expected back on first page, got: {status}"
    );
}

#[test]
fn test_summary_unfiltered_class_members_paged() {
    // Build a snapshot with more objects than EDGE_PAGE_SIZE in one class
    let count = EDGE_PAGE_SIZE + 10;
    let mut strings: Vec<String> =
        vec!["".to_string(), "(GC roots)".to_string(), "Foo".to_string()];
    for i in 0..count {
        strings.push(format!("e{i}"));
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut nodes = vec![
        9u32,
        0,
        1,
        0,
        1, // node 0: synthetic root
        9,
        1,
        2,
        0,
        count as u32, // node 1: (GC roots)
    ];
    for i in 0..count {
        // All object type (3), name "Foo" (idx 2)
        nodes.extend_from_slice(&[3, 2, (100 + i) as u32, 10, 0]);
    }

    let mut edges = vec![1u32, 0, node_index(1)];
    for i in 0..count {
        edges.extend_from_slice(&[2, (3 + i) as u32, node_index(2 + i)]);
    }

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // No filter — expand the Foo group
    app.rebuild_rows(&snap);
    let group_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .collect();
    assert!(group_rows.len() >= 1);

    let id = group_rows[0].nav.id;
    let ck = group_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Should show EDGE_PAGE_SIZE members + 1 status line
    let depth1_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 1)
        .collect();
    assert_eq!(
        depth1_rows.len(),
        EDGE_PAGE_SIZE + 1,
        "expected {} members + status, got {}",
        EDGE_PAGE_SIZE,
        depth1_rows.len()
    );
    let status = &depth1_rows.last().unwrap().render.label;
    assert!(
        status.contains("objects"),
        "expected paging status with 'objects', got: {status}"
    );
    assert!(
        status.starts_with(&format!("1\u{2013}{EDGE_PAGE_SIZE}")),
        "expected first page range, got: {status}"
    );

    // Move cursor to group and press 'n' to advance
    let group_idx = app
        .cached_rows
        .iter()
        .position(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .unwrap();
    app.summary_state.cursor = group_idx;

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);

    let depth1_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 1)
        .collect();
    let status = &depth1_rows.last().unwrap().render.label;
    assert!(
        status.starts_with(&format!("{}\u{2013}", EDGE_PAGE_SIZE + 1)),
        "expected second page, got: {status}"
    );
}

/// Helper: build a snapshot with `count` objects in a single "Foo" class,
/// create an App, expand the first summary group, and move the cursor to
/// the paging status row.  Returns (snap, app, work_rx).
fn make_paged_summary_app(count: usize) -> (HeapSnapshot, App, mpsc::Receiver<WorkItem>) {
    let mut strings: Vec<String> =
        vec!["".to_string(), "(GC roots)".to_string(), "Foo".to_string()];
    for i in 0..count {
        strings.push(format!("e{i}"));
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut nodes = vec![9u32, 0, 1, 0, 1, 9, 1, 2, 0, count as u32];
    for i in 0..count {
        nodes.extend_from_slice(&[3, 2, (100 + i) as u32, 10, 0]);
    }

    let mut edges = vec![1u32, 0, node_index(1)];
    for i in 0..count {
        edges.extend_from_slice(&[2, (3 + i) as u32, node_index(2 + i)]);
    }

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.rebuild_rows(&snap);
    // Expand the first (Foo) group
    let id = app.cached_rows[0].nav.id;
    let ck = app.cached_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Move cursor to the paging status row (last depth-1 row)
    let status_idx = app
        .cached_rows
        .iter()
        .rposition(|r| r.nav.depth == 1)
        .unwrap();
    app.summary_state.cursor = status_idx;
    assert!(
        app.cached_rows[status_idx].render.label.contains("objects"),
        "cursor should be on status row"
    );
    assert_eq!(app.cached_rows[status_idx].node_ordinal(), None);

    (snap, app, work_rx)
}

#[test]
fn test_status_row_node_ordinal_is_none() {
    let (snap, app, _work_rx) = make_paged_summary_app(EDGE_PAGE_SIZE + 5);

    // Status row has no node ordinal
    let status_row = app.current_row().unwrap();
    assert_eq!(status_row.node_ordinal(), None);
    // Real member rows have Some
    let member_row = &app.cached_rows[1];
    assert!(member_row.node_ordinal().is_some());

    // Also verify containment status rows
    let (work_tx, _work_rx2) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app2 = App::new(&snap, Vec::new(), work_tx, result_rx);
    app2.set_view(ViewType::Containment, &snap);
    app2.rebuild_rows(&snap);
    // Expand root to get paged children
    if !app2.cached_rows.is_empty() {
        let id = app2.cached_rows[0].nav.id;
        let ck = app2.cached_rows[0].nav.children_key.clone();
        app2.expand(id, ck, &snap);
        app2.rebuild_rows(&snap);
        // Last row should be the status row
        if let Some(last) = app2.cached_rows.last() {
            if last.render.label.contains("of") && !last.nav.has_children {
                assert_eq!(last.node_ordinal(), None);
            }
        }
    }
}

#[test]
fn test_status_row_distance_is_none() {
    let (_snap, app, _work_rx) = make_paged_summary_app(EDGE_PAGE_SIZE + 5);

    // The last row on the first page is a status/paging row (e.g. "1–10 of 15 objects")
    let status_row = app.current_row().unwrap();
    assert!(
        status_row.node_ordinal().is_none(),
        "expected a status row (node_ordinal=None)"
    );
    // Status rows should have distance=None, not a numeric value
    match &status_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert_eq!(
                *distance, None,
                "status/paging rows should have distance=None"
            );
        }
        _ => panic!("expected HeapNode variant for summary member status row"),
    }

    // Verify a real member row has Some(distance)
    let member_row = &app.cached_rows[1];
    assert!(member_row.node_ordinal().is_some());
    match &member_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert!(
                distance.is_some(),
                "real member rows should have Some(distance)"
            );
        }
        _ => panic!("expected HeapNode variant"),
    }
}

#[test]
fn test_keys_on_status_row_are_noop() {
    let (snap, mut app, work_rx) = make_paged_summary_app(EDGE_PAGE_SIZE + 5);

    let view_before = app.current_view;
    let rows_before = app.cached_rows.len();

    // 'r' — should not switch to retainers view
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), &snap);
    assert_eq!(app.current_view, view_before);
    assert!(app.retainers.target.is_none());

    // 's' — should not switch to summary (already there, but also shouldn't crash)
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), &snap);

    // 'R' — should not queue any reachable work
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), &snap);
    assert!(
        collect_reachable_work(&work_rx).is_empty(),
        "'R' on status row should not queue reachable work"
    );

    // 'A' — should not queue any reachable work
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE), &snap);
    assert!(
        collect_reachable_work(&work_rx).is_empty(),
        "'A' on status row should not queue reachable work"
    );

    // Enter — should not expand (status row has no children)
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &snap);
    app.rebuild_rows(&snap);
    assert_eq!(
        app.cached_rows.len(),
        rows_before,
        "Enter on status row should not change row count"
    );
}

#[test]
fn test_filtered_status_row_node_ordinal_is_none() {
    let count = (EDGE_PAGE_SIZE + 5) * 2;
    let mut strings: Vec<String> = vec![
        "".to_string(),
        "(GC roots)".to_string(),
        "TargetFunc".to_string(),
        "OtherCode".to_string(),
    ];
    for i in 0..count {
        strings.push(format!("e{i}"));
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    let mut nodes = vec![9u32, 0, 1, 0, 1, 9, 1, 2, 0, count as u32];
    for i in 0..count {
        let name_idx = if i % 2 == 0 { 2u32 } else { 3 };
        nodes.extend_from_slice(&[4, name_idx, (100 + i) as u32, 10, 0]);
    }

    let mut edges = vec![1u32, 0, node_index(1)];
    for i in 0..count {
        edges.extend_from_slice(&[2, (4 + i) as u32, node_index(2 + i)]);
    }

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Apply member-level filter and expand
    app.summary_filter = "targetfunc".to_string();
    app.rebuild_rows(&snap);
    let id = app.cached_rows[0].nav.id;
    let ck = app.cached_rows[0].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Find the "matching" status row
    let status_idx = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("matching"))
        .expect("should have a filtered paging status row");

    // Status row should have None ordinal
    assert_eq!(app.cached_rows[status_idx].node_ordinal(), None);

    // Move cursor there and press 'R' — no work should be queued
    app.summary_state.cursor = status_idx;
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), &snap);
    assert!(
        collect_reachable_work(&work_rx).is_empty(),
        "'R' on filtered status row should not queue reachable work"
    );
}

#[test]
fn test_shift_edge_window_respects_enlarged_page_size() {
    // (GC roots) with 5 * EDGE_PAGE_SIZE edges so we can grow the window then page.
    let edge_count = EDGE_PAGE_SIZE * 5;
    let mut strings: Vec<String> = vec![
        "".to_string(),           // 0
        "(GC roots)".to_string(), // 1
        "child".to_string(),      // 2
    ];
    for i in 0..edge_count {
        strings.push(format!("e{i}"));
    }

    let node_index = |ordinal: usize| (ordinal * 5) as u32;
    // node 0: synthetic root, node 1: (GC roots)
    let mut nodes = vec![9u32, 0, 1, 0, 1, 9, 1, 2, 0, edge_count as u32];
    for i in 0..edge_count {
        nodes.extend_from_slice(&[3, 2, (200 + i) as u32, 10, 0]);
    }

    let mut edges = vec![
        1u32,
        0,
        node_index(1), // root -> (GC roots)
    ];
    for i in 0..edge_count {
        edges.extend_from_slice(&[2, (3 + i) as u32, node_index(2 + i)]);
    }

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Switch to containment — (GC roots) is the first visible row
    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // Find and expand (GC roots)
    let gcr_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .unwrap();
    let gcr_id = app.cached_rows[gcr_idx].nav.id;
    let gcr_ck = app.cached_rows[gcr_idx].nav.children_key.clone();
    app.expand(gcr_id, gcr_ck, &snap);
    app.rebuild_rows(&snap);

    // Default page: 1–EDGE_PAGE_SIZE
    let ck = ChildrenKey::Edges(gcr_id, NodeOrdinal(1));
    let children = app.containment_state.children_map.get(&ck).unwrap();
    let status = &children.last().unwrap().label;
    assert!(
        status.starts_with(&format!("1\u{2013}{EDGE_PAGE_SIZE} of {edge_count}")),
        "expected default page, got: {status}"
    );

    // Move cursor to (GC roots) row and press '+' to grow window to 2*EDGE_PAGE_SIZE
    app.containment_state.cursor = gcr_idx;
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE), &snap);

    let doubled = EDGE_PAGE_SIZE * 2;
    let children = app.containment_state.children_map.get(&ck).unwrap();
    let status = &children.last().unwrap().label;
    assert!(
        status.starts_with(&format!("1\u{2013}{doubled} of {edge_count}")),
        "expected doubled page, got: {status}"
    );

    // Press 'n' — should shift by the current window size (2*EDGE_PAGE_SIZE),
    // NOT by the default EDGE_PAGE_SIZE.
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &snap);

    let children = app.containment_state.children_map.get(&ck).unwrap();
    let status = &children.last().unwrap().label;
    assert!(
        status.starts_with(&format!(
            "{}\u{2013}{} of {edge_count}",
            doubled + 1,
            doubled + doubled
        )),
        "expected shift by doubled window size, got: {status}"
    );

    // Press 'p' — should shift back by the same window size
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), &snap);

    let children = app.containment_state.children_map.get(&ck).unwrap();
    let status = &children.last().unwrap().label;
    assert!(
        status.starts_with(&format!("1\u{2013}{doubled} of {edge_count}")),
        "expected back to first page, got: {status}"
    );
}

/// Build a snapshot where node 3 is only reachable via a weak edge (unreachable).
/// Verify that the TUI displays distance as -5 (rendered as "–") for such nodes.
fn make_unreachable_node_snapshot() -> HeapSnapshot {
    // Strings: 0: "", 1: "(GC roots)", 2: "Reachable", 3: "Unreachable",
    //          4: "Child", 5: "ref", 6: "weak_ref", 7: "child"
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "Reachable".into(),
        "Unreachable".into(),
        "Child".into(),
        "ref".into(),
        "weak_ref".into(),
        "child".into(),
    ];

    let n = |ordinal: u32| ordinal * 5; // node_field_count = 5

    //              type name id  size edges
    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 1, // node 2: Reachable, size=100, 1 weak edge
        3, 3, 5, 300, 1, // node 3: Unreachable, size=300, 1 edge
        3, 4, 7, 150, 0, // node 4: Child, size=150, 0 edges
    ];

    // edge type indices: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element[0]--> (GC roots)
        2,
        5,
        n(2), // (GC roots) --property "ref"--> Reachable
        6,
        6,
        n(3), // Reachable --weak "weak_ref"--> Unreachable
        2,
        7,
        n(4), // Unreachable --property "child"--> Child
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_unreachable_node_distance_displayed_as_dash() {
    let snap = make_unreachable_node_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Use containment view and expand (GC roots) → Reachable → Unreachable
    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // Expand (GC roots)
    let gcr_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .unwrap();
    let gcr_id = app.cached_rows[gcr_idx].nav.id;
    let gcr_ck = app.cached_rows[gcr_idx].nav.children_key.clone();
    app.expand(gcr_id, gcr_ck, &snap);
    app.rebuild_rows(&snap);

    // Expand Reachable (node 2) to reveal the weak edge to Unreachable
    let reachable_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    let reachable_id = app.cached_rows[reachable_idx].nav.id;
    let reachable_ck = app.cached_rows[reachable_idx].nav.children_key.clone();
    app.expand(reachable_id, reachable_ck, &snap);
    app.rebuild_rows(&snap);

    // Find Unreachable (node 3) in flattened rows
    let unreachable_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(3)))
        .expect("Unreachable node should appear in containment tree via weak edge");

    match &unreachable_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert_eq!(
                *distance,
                Some(Distance::UNREACHABLE_BASE),
                "unreachable node should have UNREACHABLE_BASE"
            );
            // UNREACHABLE_BASE is rendered as "U" by the TUI
            let dist_str = crate::print::format_distance(distance.unwrap());
            assert_eq!(dist_str, "U");
        }
        _ => panic!("expected HeapNode"),
    }

    // Also verify the reachable node has distance 1
    let reachable_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    match &reachable_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert_eq!(
                *distance,
                Some(Distance(1)),
                "Reachable node should have distance 1"
            );
        }
        _ => panic!("expected HeapNode"),
    }

    // Expand Unreachable to see Child (node 4, also unreachable)
    let unr_id = unreachable_row.nav.id;
    let unr_ck = unreachable_row.nav.children_key.clone();
    app.expand(unr_id, unr_ck, &snap);
    app.rebuild_rows(&snap);

    let child_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(4)))
        .expect("Child of unreachable node should appear after expanding");

    match &child_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert_eq!(
                *distance,
                Some(Distance(Distance::UNREACHABLE_BASE.0 + 1)),
                "child of unreachable node should have UNREACHABLE_BASE + 1"
            );
        }
        _ => panic!("expected HeapNode"),
    }
}

#[test]
fn test_retainers_of_unreachable_node() {
    let snap = make_unreachable_node_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Open retainers for node 4 (Child, unreachable).
    // Its only retainer is node 3 (Unreachable, distance=-5).
    app.set_retainers_target(NodeOrdinal(4), &snap);
    app.rebuild_rows(&snap);

    // The retainer (node 3) should appear
    let retainer_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(3)));

    let retainer_row = retainer_row
        .expect("node 3 should appear as retainer of node 4 (it's a strong property edge)");

    match &retainer_row.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            // The retainer (node 3) is unreachable, so distance should be UNREACHABLE_BASE
            assert_eq!(
                *distance,
                Some(Distance::UNREACHABLE_BASE),
                "unreachable retainer should have UNREACHABLE_BASE"
            );
        }
        _ => panic!("expected HeapNode"),
    }

    // Because distance >= UNREACHABLE_BASE, compute_retainers marks it as non-expandable.
    // This means you can't walk further up the retainer chain from an
    // unreachable node — the retainer tree dead-ends here.
    assert!(
        !retainer_row.nav.has_children,
        "unreachable retainer should not be expandable (distance >= UNREACHABLE_BASE)"
    );

    // Now open retainers for node 3 (Unreachable).
    // Its retainer is node 2 via a weak edge — weak edges now show up
    // with a [weak] prefix in the label.
    app.set_retainers_target(NodeOrdinal(3), &snap);
    app.rebuild_rows(&snap);

    let weak_retainer = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("weak-edge retainer (node 2) should now appear");

    // The row should be marked as weak (rendered dimmed in the TUI)
    assert!(
        weak_retainer.render.is_weak,
        "weak retainer should have is_weak=true",
    );

    match &weak_retainer.render.kind {
        FlatRowKind::HeapNode { distance, .. } => {
            assert_eq!(
                *distance,
                Some(Distance(1)),
                "node 2 is reachable, distance should be 1"
            );
        }
        _ => panic!("expected HeapNode"),
    }

    // Node 2 is reachable (distance=1), so it should be expandable
    assert!(
        weak_retainer.nav.has_children,
        "reachable weak retainer should still be expandable"
    );
}

#[test]
fn test_weak_edge_in_containment_view() {
    let snap = make_unreachable_node_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // Expand (GC roots) → Reachable
    let gcr_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .unwrap();
    let gcr_id = app.cached_rows[gcr_idx].nav.id;
    let gcr_ck = app.cached_rows[gcr_idx].nav.children_key.clone();
    app.expand(gcr_id, gcr_ck, &snap);
    app.rebuild_rows(&snap);

    // Expand Reachable — its only edge is weak → Unreachable
    let reachable_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    let reachable_id = app.cached_rows[reachable_idx].nav.id;
    let reachable_ck = app.cached_rows[reachable_idx].nav.children_key.clone();
    app.expand(reachable_id, reachable_ck, &snap);
    app.rebuild_rows(&snap);

    // The weak edge child (node 3) should have is_weak=true
    let weak_child = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(3)))
        .expect("Unreachable node should appear via weak edge");
    assert!(
        weak_child.render.is_weak,
        "weak edge child should have is_weak=true"
    );

    // The strong edge child from (GC roots) → Reachable should NOT be weak
    let strong_child = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    assert!(
        !strong_child.render.is_weak,
        "strong edge child should have is_weak=false"
    );
}

#[test]
fn test_weak_edge_in_summary_view() {
    let snap = make_unreachable_node_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Summary view — expand a class group, then expand a member to see edges
    app.current_view = ViewType::Summary;
    app.rebuild_rows(&snap);

    // Find the "Reachable" group (node 2 is type "object" named "Reachable")
    // and expand it to show class members
    let group_idx = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("Reachable"))
        .expect("Should find Reachable group in summary");
    let group_id = app.cached_rows[group_idx].nav.id;
    let group_ck = app.cached_rows[group_idx].nav.children_key.clone();
    app.expand(group_id, group_ck, &snap);
    app.rebuild_rows(&snap);

    // Find the Reachable member row (node ordinal 2)
    let member_idx = app
        .cached_rows
        .iter()
        .position(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("Should find Reachable member");
    let member_id = app.cached_rows[member_idx].nav.id;
    let member_ck = app.cached_rows[member_idx].nav.children_key.clone();
    app.expand(member_id, member_ck, &snap);
    app.rebuild_rows(&snap);

    // The weak edge child (node 3) should have is_weak=true
    let weak_child = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(3)))
        .expect("Unreachable node should appear via weak edge in summary");
    assert!(
        weak_child.render.is_weak,
        "weak edge in summary should have is_weak=true"
    );
}

#[test]
fn test_strong_edges_not_marked_weak() {
    let snap = make_unreachable_node_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Dominators view — all edges here are strong dominator relationships
    app.current_view = ViewType::Dominators;
    app.rebuild_rows(&snap);

    for row in &app.cached_rows {
        assert!(!row.render.is_weak, "dominator rows should never be weak");
    }

    // Expand the root to see dominated children
    let root_id = app.cached_rows[0].nav.id;
    let root_ck = app.cached_rows[0].nav.children_key.clone();
    app.expand(root_id, root_ck, &snap);
    app.rebuild_rows(&snap);

    for row in &app.cached_rows {
        assert!(
            !row.render.is_weak,
            "dominated children should not be weak, got weak row: {}",
            row.render.label
        );
    }
}

#[test]
fn test_weak_retainer_filtered_by_path_edges() {
    // When path_edges is active, compute_retainers should only include
    // edges in the path set — including weak edges not in the set should
    // be excluded.
    let snap = make_unreachable_node_snapshot();
    let next_id = std::cell::Cell::new(1u64);

    // Without path_edges: all retainers of node 3 are returned (including
    // the weak edge from node 2).
    let all = super::children::compute_retainers(
        &snap,
        NodeOrdinal(3),
        EdgeWindow::default(),
        None,
        &next_id,
    );
    let weak_count = all.iter().filter(|c| c.is_weak).count();
    assert!(
        weak_count > 0,
        "node 3 should have at least one weak retainer"
    );

    // With an empty path_edges set: no edges pass the filter.
    let empty_set = FxHashSet::default();
    let filtered = super::children::compute_retainers(
        &snap,
        NodeOrdinal(3),
        EdgeWindow::default(),
        Some(&empty_set),
        &next_id,
    );
    let filtered_weak = filtered.iter().filter(|c| c.is_weak).count();
    assert_eq!(
        filtered_weak, 0,
        "off-path weak retainers should be excluded when path_edges is active"
    );
}

#[test]
fn test_filtered_retainers_sorted_before_paging() {
    // Build a snapshot where the target (node 2) has 3 retainers at
    // different distances.  Raw snapshot order: far (dist 3), mid (dist 2),
    // near (dist 1).  With path_edges filtering + page size 2, page 1
    // should contain the two nearest retainers (dist 1, 2), not the two
    // that happen to appear first in raw order (dist 3, 2).
    //
    // Graph:
    //   node 0 (synthetic root) -> node 1 (GC roots) -> node 3 (near, dist 1) -> target
    //   node 0 -> node 1 -> node 4 (bridge) -> node 5 (mid, dist 3) -> target
    //   node 0 -> node 6 (far, dist 3, NOT on GC path) -> target
    //
    // Retainer iteration order for target: far (node 6, edge_idx=e3),
    // mid (node 5, edge_idx=e2), near (node 3, edge_idx=e1).
    // path_edges filter includes e1 and e2 (but not e3), giving 2 filtered
    // retainers in raw order: mid, near.  After sorting by distance,
    // page 1 (size=1) should show near (dist 2), not mid (dist 3).
    let strings = vec![
        "".to_string(),           // 0
        "(GC roots)".to_string(), // 1
        "Target".to_string(),     // 2
        "Near".to_string(),       // 3
        "Bridge".to_string(),     // 4
        "Mid".to_string(),        // 5
        "Far".to_string(),        // 6
        "ref".to_string(),        // 7
        "gc".to_string(),         // 8
        "link".to_string(),       // 9
    ];

    let ni = |ord: usize| (ord * 5) as u32;
    let nodes = vec![
        // node 0: synthetic root (type=9, 3 edges)
        9, 0, 1, 0, 3, // node 1: (GC roots) (type=9, 2 edges)
        9, 1, 2, 0, 2, // node 2: Target (type=3 object, 0 edges)
        3, 2, 3, 100, 0, // node 3: Near (type=3, 1 edge -> target)
        3, 3, 4, 10, 1, // node 4: Bridge (type=3, 1 edge -> Mid)
        3, 4, 5, 10, 1, // node 5: Mid (type=3, 1 edge -> target)
        3, 5, 6, 10, 1, // node 6: Far (type=3, 1 edge -> target, unreachable from GC roots)
        3, 6, 7, 10, 1,
    ];

    // Edges laid out so retainer iteration for Target sees Far before Mid
    // before Near (raw order is by edge_idx, which follows declaration order).
    let edges = vec![
        // node 0 edges: -> GC roots, -> Bridge, -> Far
        1,
        0,
        ni(1), // e0: root -> GC roots
        2,
        9,
        ni(4), // e1: root -> Bridge
        2,
        8,
        ni(6), // e2: root -> Far
        // node 1 edges: -> Near, -> bridge?  No: GC roots -> Near
        2,
        8,
        ni(3), // e3: GC roots -> Near
        2,
        9,
        ni(4), // e4: GC roots -> Bridge  (so Bridge is at dist 2)
        // node 3 edges: Near -> Target
        2,
        7,
        ni(2), // e5: Near -> Target
        // node 4 edges: Bridge -> Mid
        2,
        9,
        ni(5), // e6: Bridge -> Mid
        // node 5 edges: Mid -> Target
        2,
        7,
        ni(2), // e7: Mid -> Target
        // node 6 edges: Far -> Target
        2,
        7,
        ni(2), // e8: Far -> Target
    ];

    let snap = build_snapshot(strings, nodes, edges);

    // Verify distances make sense.
    let near_dist = snap.node_distance(NodeOrdinal(3));
    let mid_dist = snap.node_distance(NodeOrdinal(5));
    assert!(
        near_dist < mid_dist,
        "Near (dist {near_dist}) should be closer than Mid (dist {mid_dist})"
    );

    // Collect retainer edge indices for Target (node 2).
    let mut retainer_edges = Vec::new();
    snap.for_each_retainer(NodeOrdinal(2), |edge_idx, _| {
        retainer_edges.push(edge_idx);
    });
    // path_edges includes all retainer edges except Far's.
    let far_edge = retainer_edges.iter().find(|&&ei| {
        let mut found_ord = NodeOrdinal(0);
        snap.for_each_retainer(NodeOrdinal(2), |ei2, ord| {
            if ei2 == ei {
                found_ord = ord;
            }
        });
        found_ord == NodeOrdinal(6)
    });
    let path_edges: FxHashSet<usize> = retainer_edges
        .iter()
        .copied()
        .filter(|ei| Some(ei) != far_edge)
        .collect();

    let next_id = std::cell::Cell::new(1u64);

    // Page size 1 — first page should show the nearest retainer.
    let w = EdgeWindow { start: 0, count: 1 };
    let page1 = compute_retainers(&snap, NodeOrdinal(2), w, Some(&path_edges), &next_id);
    let first_data: Vec<_> = page1.iter().filter(|c| c.node_ordinal.is_some()).collect();
    assert_eq!(first_data.len(), 1);
    assert_eq!(
        first_data[0].distance,
        Some(near_dist),
        "page 1 should contain nearest retainer (dist {near_dist}), got dist {:?}",
        first_data[0].distance
    );

    // Page 2 should show the farther retainer.
    let w2 = EdgeWindow { start: 1, count: 1 };
    let page2 = compute_retainers(&snap, NodeOrdinal(2), w2, Some(&path_edges), &next_id);
    let second_data: Vec<_> = page2.iter().filter(|c| c.node_ordinal.is_some()).collect();
    assert_eq!(second_data.len(), 1);
    assert_eq!(
        second_data[0].distance,
        Some(mid_dist),
        "page 2 should contain farther retainer (dist {mid_dist}), got dist {:?}",
        second_data[0].distance
    );
}

#[test]
fn test_plan_stops_at_gc_root_successor() {
    // The plan tree should stop at the child of (GC roots) (e.g. a root
    // category like "(Strong roots)") and never include (GC roots) itself.
    //
    // make_nested_retainers_snapshot graph (0 extras):
    //   node 0 (synthetic root) -> node 1 (GC roots) -> node 4 (RootHolder)
    //                                                      -> node 3 (Holder) -> node 2 (target)
    //   node 0 -> node 5 (DetachedHolder) -> node 3 (Holder)
    //
    // Path: target(2) <- Holder(3) <- RootHolder(4) <- (GC roots)(1)
    // RootHolder is the successor of (GC roots), so the plan should stop there.
    let snap = make_nested_retainers_snapshot(0);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );

    assert!(plan.reached_gc_roots, "plan should reach GC roots");

    // Collect all ordinals in the plan tree.
    fn collect_ordinals(edges: &[crate::print::retainers::RetainerPathEdge]) -> Vec<NodeOrdinal> {
        let mut result = Vec::new();
        for e in edges {
            result.push(e.retainer);
            result.extend(collect_ordinals(&e.children));
        }
        result
    }
    let plan_ordinals = collect_ordinals(&plan.tree);

    // (GC roots) = node 1, should NOT be in the plan tree.
    assert!(
        !plan_ordinals.contains(&NodeOrdinal(1)),
        "(GC roots) should not appear in the plan tree; tree contains: {plan_ordinals:?}"
    );

    // RootHolder = node 4, SHOULD be the deepest node in the tree.
    assert!(
        plan_ordinals.contains(&NodeOrdinal(4)),
        "RootHolder (the GC root successor) should be in the plan tree"
    );

    // Holder = node 3, should also be present (intermediate).
    assert!(
        plan_ordinals.contains(&NodeOrdinal(3)),
        "Holder (intermediate node) should be in the plan tree"
    );
}

#[test]
fn test_plan_gc_root_successor_is_leaf_in_tui() {
    // When the plan is applied in the TUI, the GC root successor should
    // appear as a non-auto-expanded leaf (expandable but collapsed).
    let snap = make_nested_retainers_snapshot(0);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // RootHolder (node 4) should be visible.
    let root_holder_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("RootHolder"))
        .expect("RootHolder should be visible in the flattened rows");

    // It should be expandable (has_children) but NOT auto-expanded.
    assert!(
        root_holder_row.nav.has_children,
        "RootHolder should be expandable"
    );
    assert!(
        !root_holder_row.nav.is_expanded,
        "RootHolder should not be auto-expanded (it's the terminal GC root successor)"
    );

    // (GC roots) should NOT appear anywhere in the rows before manual expand.
    let gc_roots_present = app
        .cached_rows
        .iter()
        .any(|r| r.render.label.contains("(GC roots)"));
    assert!(
        !gc_roots_present,
        "(GC roots) should not appear in the retainers view before manual expand"
    );

    // Manually expanding RootHolder should reveal (GC roots) as a retainer.
    let rh_id = root_holder_row.nav.id;
    let rh_ck = root_holder_row.nav.children_key.clone();
    app.expand(rh_id, rh_ck, &snap);
    app.rebuild_rows(&snap);

    let gc_roots_after = app
        .cached_rows
        .iter()
        .any(|r| r.render.label.contains("(GC roots)"));
    assert!(
        gc_roots_after,
        "(GC roots) should be visible after manually expanding RootHolder"
    );
}

#[test]
fn test_plan_includes_weak_retainer_paths() {
    // Build a snapshot where the target has a strong path AND a weak path
    // to GC roots.  Both should appear in the plan tree, with the weak
    // retainer marked is_weak in the TUI.
    //
    // Graph:
    //   node 0 (synthetic root) -> node 1 (GC roots)
    //   node 1 -> node 2 (StrongHolder)  --strong--> node 4 (Target)
    //   node 1 -> node 3 (WeakHolder)    --weak--->  node 4 (Target)
    let strings = vec![
        "".to_string(),             // 0
        "(GC roots)".to_string(),   // 1
        "StrongHolder".to_string(), // 2
        "WeakHolder".to_string(),   // 3
        "Target".to_string(),       // 4
        "ref".to_string(),          // 5
        "weak_ref".to_string(),     // 6
        "gc".to_string(),           // 7
    ];

    let ni = |ord: usize| (ord * 5) as u32;
    let nodes = vec![
        // node 0: synthetic root (2 edges: -> GC roots)
        9, 0, 1, 0, 1, // node 1: (GC roots) (2 edges: -> StrongHolder, -> WeakHolder)
        9, 1, 2, 0, 2, // node 2: StrongHolder (1 strong edge -> Target)
        3, 2, 3, 10, 1, // node 3: WeakHolder (1 weak edge -> Target)
        3, 3, 4, 10, 1, // node 4: Target (0 edges)
        3, 4, 5, 100, 0,
    ];

    let edges = vec![
        // node 0 edges
        1,
        0,
        ni(1), // e0: root -> (GC roots)
        // node 1 edges
        2,
        7,
        ni(2), // e1: (GC roots) -> StrongHolder
        2,
        7,
        ni(3), // e2: (GC roots) -> WeakHolder
        // node 2 edges
        2,
        5,
        ni(4), // e3: StrongHolder -> Target (strong)
        // node 3 edges
        6,
        6,
        ni(4), // e4: WeakHolder -> Target (weak, type=6)
    ];

    let snap = build_snapshot(strings, nodes, edges);

    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(4),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    assert!(plan.reached_gc_roots);

    // Both StrongHolder and WeakHolder should be in the plan tree.
    let top_retainers: Vec<NodeOrdinal> = plan.tree.iter().map(|e| e.retainer).collect();
    assert!(
        top_retainers.contains(&NodeOrdinal(2)),
        "StrongHolder should be in the plan tree"
    );
    assert!(
        top_retainers.contains(&NodeOrdinal(3)),
        "WeakHolder should be in the plan tree (weak path to GC roots)"
    );

    // Apply in TUI and check is_weak rendering.
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(4), &snap);
    app.apply_retainers_plan(NodeOrdinal(4), plan, &snap);
    app.rebuild_rows(&snap);

    let strong_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("StrongHolder"));
    let weak_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("WeakHolder"));

    assert!(strong_row.is_some(), "StrongHolder should be visible");
    assert!(weak_row.is_some(), "WeakHolder should be visible");
    assert!(
        !strong_row.unwrap().render.is_weak,
        "StrongHolder should not be marked weak"
    );
    assert!(
        weak_row.unwrap().render.is_weak,
        "WeakHolder should be marked weak"
    );
}

/// Snapshot where target (node 2) has two direct retainers:
///   - OnPathHolder (node 3): on a GC-root path via RootCategory (node 4)
///   - OffPathHolder (node 5): NOT on any GC-root path
///
/// Graph:
///   node 0 (synthetic root) -> node 1 (GC roots) -> node 4 (RootCategory)
///                                                        -> node 3 (OnPathHolder) -> node 2 (target)
///   node 0 -> node 5 (OffPathHolder) -> node 2 (target)
fn make_mixed_retainers_snapshot() -> HeapSnapshot {
    let strings = vec![
        "".to_string(),              // 0
        "(GC roots)".to_string(),    // 1
        "Target".to_string(),        // 2
        "OnPathHolder".to_string(),  // 3
        "RootCategory".to_string(),  // 4
        "OffPathHolder".to_string(), // 5
        "ref".to_string(),           // 6
        "gc".to_string(),            // 7
        "off".to_string(),           // 8
    ];

    let ni = |ord: usize| (ord * 5) as u32;
    let nodes = vec![
        // node 0: synthetic root (2 edges)
        9, 0, 1, 0, 2, // node 1: (GC roots) (1 edge -> RootCategory)
        9, 1, 2, 0, 1, // node 2: Target (0 edges)
        3, 2, 3, 100, 0, // node 3: OnPathHolder (1 edge -> Target)
        3, 3, 4, 10, 1, // node 4: RootCategory (1 edge -> OnPathHolder)
        3, 4, 5, 10, 1, // node 5: OffPathHolder (1 edge -> Target)
        3, 5, 6, 10, 1,
    ];

    let edges = vec![
        // node 0 edges
        1,
        0,
        ni(1), // e0: root -> (GC roots)
        2,
        8,
        ni(5), // e1: root -> OffPathHolder
        // node 1 edges
        2,
        7,
        ni(4), // e2: (GC roots) -> RootCategory
        // node 3 edges
        2,
        6,
        ni(2), // e3: OnPathHolder -> Target
        // node 4 edges
        2,
        6,
        ni(3), // e4: RootCategory -> OnPathHolder
        // node 5 edges
        2,
        8,
        ni(2), // e5: OffPathHolder -> Target
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_apply_plan_shows_all_direct_retainers() {
    let snap = make_mixed_retainers_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // Both direct retainers should be visible in the flattened rows.
    let on_path = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("OnPathHolder"));
    let off_path = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("OffPathHolder"));

    assert!(on_path.is_some(), "OnPathHolder should be visible");
    assert!(off_path.is_some(), "OffPathHolder should be visible");

    // OnPathHolder should be auto-expanded (it's on the plan tree).
    assert!(
        on_path.unwrap().nav.is_expanded,
        "OnPathHolder should be auto-expanded (on GC root path)"
    );

    // OffPathHolder should NOT be auto-expanded.
    assert!(
        !off_path.unwrap().nav.is_expanded,
        "OffPathHolder should not be auto-expanded (not on GC root path)"
    );

    // OnPathHolder's subtree (RootCategory) should be visible
    // because it's auto-expanded.
    let root_cat = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("RootCategory"));
    assert!(
        root_cat.is_some(),
        "RootCategory should be visible (child of auto-expanded OnPathHolder)"
    );
}

#[test]
fn test_apply_plan_sorts_on_path_retainers_first() {
    let snap = make_mixed_retainers_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // Find positions of the two direct retainers (depth-1 rows).
    let on_path_pos = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("OnPathHolder"))
        .expect("OnPathHolder should exist");
    let off_path_pos = app
        .cached_rows
        .iter()
        .position(|r| r.render.label.contains("OffPathHolder"))
        .expect("OffPathHolder should exist");

    assert!(
        on_path_pos < off_path_pos,
        "On-path retainers should appear before off-path retainers \
             (on_path at {on_path_pos}, off_path at {off_path_pos})"
    );
}

#[test]
fn test_apply_plan_no_selected_status_line() {
    // After applying a plan, we should NOT see "X selected of Y retainers"
    // since all retainers are shown.
    let snap = make_mixed_retainers_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    let has_selected_line = app
        .cached_rows
        .iter()
        .any(|r| r.render.label.contains("selected of"));
    assert!(
        !has_selected_line,
        "Should not have a 'selected of' status line — all retainers are shown"
    );
}

#[test]
fn test_apply_plan_intermediate_status_line() {
    // make_nested_retainers_snapshot(0) creates:
    //   root(0) -> (GC roots)(1) -> root_holder(4) -> holder(3) -> target(2)
    //   root(0) -> detached_holder(5) -> holder(3)
    //
    // The plan for target(2) selects: holder(3) -> root_holder(4) -> (GC roots).
    // holder(3) has 2 retainers (root_holder and detached_holder) but only
    // root_holder is on the plan.  The status line "1 selected of 2 retainers"
    // should appear under holder(3) in the flattened rows.
    let snap = make_nested_retainers_snapshot(0);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 8,
            max_nodes: 64,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    // Find the status line under the auto-expanded holder node.
    let status_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("selected of"));
    assert!(
        status_row.is_some(),
        "expected 'selected of' status line under auto-expanded intermediate node"
    );
    assert!(
        status_row
            .unwrap()
            .render
            .label
            .contains("1 selected of 2 retainers"),
        "status line should show '1 selected of 2 retainers', got: {:?}",
        status_row.unwrap().render.label,
    );
}

#[test]
fn test_apply_plan_intermediate_status_line_with_extra_retainers() {
    // With extra_retainers=5, holder(3) has 7 retainers total
    // (root_holder + detached_holder + 5 extras), but the plan
    // only selects root_holder.
    let snap = make_nested_retainers_snapshot(5);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(2),
        RetainerAutoExpandLimits {
            max_depth: 8,
            max_nodes: 64,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(2), plan, &snap);
    app.rebuild_rows(&snap);

    let status_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("selected of"));
    assert!(status_row.is_some(), "expected 'selected of' status line");
    assert!(
        status_row
            .unwrap()
            .render
            .label
            .contains("1 selected of 7 retainers"),
        "status line should show '1 selected of 7 retainers', got: {:?}",
        status_row.unwrap().render.label,
    );
}

#[test]
fn test_apply_plan_root_level_status_line_when_paged() {
    // make_nested_retainers_snapshot with extra_retainers creates many
    // retainers of holder(3).  At the ROOT level (target's retainers),
    // there's only 1 retainer (holder), so no root-level status line.
    // But we can test the root level with make_many_retainers_snapshot
    // which has many retainers for the target.
    let snap = make_many_retainers_snapshot(25);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(2), &snap);

    // Before the plan is applied, the initial paged view should have a
    // status line like "1-20 of 25 retainers".
    app.rebuild_rows(&snap);
    let paging_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("of 25 retainers"));
    assert!(
        paging_row.is_some(),
        "expected paging status line for 25 retainers"
    );
}

/// Snapshot with a diamond/reconvergence in the retainer graph:
///
///   (GC roots)(1) -> RootCat(5) -> Shared(4) -> HolderA(2) -> Target(6)
///                                            \-> HolderB(3) -> Target(6)
///
/// Both HolderA and HolderB reach GC roots via Shared, but when the DFS
/// visits Shared the second time (through HolderB), it's already memoized.
fn make_diamond_retainers_snapshot() -> HeapSnapshot {
    let strings = vec![
        "".to_string(),           // 0
        "(GC roots)".to_string(), // 1
        "HolderA".to_string(),    // 2
        "HolderB".to_string(),    // 3
        "Shared".to_string(),     // 4
        "RootCat".to_string(),    // 5
        "Target".to_string(),     // 6
        "ref".to_string(),        // 7
        "gc".to_string(),         // 8
    ];

    let ni = |ord: usize| (ord * 5) as u32;
    let nodes = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root (1 edge)
        9, 1, 2, 0, 1, // node 1: (GC roots) (1 edge -> RootCat)
        3, 2, 3, 10, 1, // node 2: HolderA (1 edge -> Target)
        3, 3, 4, 10, 1, // node 3: HolderB (1 edge -> Target)
        3, 4, 5, 10, 2, // node 4: Shared (2 edges -> HolderA, HolderB)
        3, 5, 6, 10, 1, // node 5: RootCat (1 edge -> Shared)
        3, 6, 7, 100, 0, // node 6: Target (0 edges)
    ];

    let edges = vec![
        1,
        0,
        ni(1), // e0: root -> (GC roots)
        2,
        8,
        ni(5), // e1: (GC roots) -> RootCat
        2,
        7,
        ni(6), // e2: HolderA -> Target
        2,
        7,
        ni(6), // e3: HolderB -> Target
        2,
        7,
        ni(2), // e4: Shared -> HolderA
        2,
        7,
        ni(3), // e5: Shared -> HolderB
        2,
        7,
        ni(4), // e6: RootCat -> Shared
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_plan_excludes_shared_subgraph_dead_ends() {
    // In a diamond retainer graph, the DFS visits Shared once through
    // HolderA (building the subtree) and again through HolderB (memo hit).
    // The plan tree should NOT include the pruned occurrence of Shared
    // under HolderB, since it would appear as a dead-end branch.
    let snap = make_diamond_retainers_snapshot();
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(6), // Target
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );

    assert!(plan.reached_gc_roots);

    // Collect the plan tree structure.
    fn tree_labels(
        snap: &HeapSnapshot,
        edges: &[crate::print::retainers::RetainerPathEdge],
    ) -> Vec<(String, Vec<String>)> {
        edges
            .iter()
            .map(|e| {
                let name = snap.node_display_name(e.retainer).to_string();
                let child_names: Vec<String> = tree_labels(snap, &e.children)
                    .into_iter()
                    .map(|(n, _)| n)
                    .collect();
                (name, child_names)
            })
            .collect()
    }
    let top = tree_labels(&snap, &plan.tree);
    let top_names: Vec<&str> = top.iter().map(|(n, _)| n.as_str()).collect();

    // One of HolderA/HolderB should have Shared as a child in the tree
    // (first visit, full subtree). The other should be excluded entirely
    // from the plan tree (pruned duplicate with no visible path to root).
    let holders_in_tree: Vec<_> = top.iter().filter(|(n, _)| n.contains("Holder")).collect();

    assert_eq!(
        holders_in_tree.len(),
        1,
        "Only one Holder should be in the plan tree (the one with the full \
             Shared subtree); got: {top_names:?}"
    );

    // The included Holder should have Shared as a child.
    let (holder_name, holder_children) = holders_in_tree[0];
    assert!(
        holder_children.iter().any(|c| c.contains("Shared")),
        "{holder_name} should have Shared as a child in the plan tree"
    );
}

#[test]
fn test_plan_diamond_gc_root_path_edges_complete() {
    // Even though the plan tree prunes the shared-subgraph duplicate,
    // gc_root_path_edges should still contain edges for BOTH paths
    // (through HolderA and HolderB).
    let snap = make_diamond_retainers_snapshot();
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(6), // Target
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );

    // Both retainer edges into Target (from HolderA and HolderB) should
    // be in gc_root_path_edges.
    let mut target_retainer_edges = Vec::new();
    snap.for_each_retainer(NodeOrdinal(6), |edge_idx, _| {
        target_retainer_edges.push(edge_idx);
    });

    for edge_idx in &target_retainer_edges {
        assert!(
            plan.gc_root_path_edges.contains(edge_idx),
            "gc_root_path_edges should contain edge {edge_idx} \
                 (retainer edge into Target)"
        );
    }
}

#[test]
fn test_plan_diamond_both_holders_visible_in_tui() {
    // After applying the plan, both HolderA and HolderB should be
    // visible in the TUI (all direct retainers are shown), but only
    // the one with the full subtree should be auto-expanded.
    let snap = make_diamond_retainers_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(NodeOrdinal(6), &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(6),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );
    app.apply_retainers_plan(NodeOrdinal(6), plan, &snap);
    app.rebuild_rows(&snap);

    let holder_a = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("HolderA"));
    let holder_b = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("HolderB"));

    assert!(holder_a.is_some(), "HolderA should be visible");
    assert!(holder_b.is_some(), "HolderB should be visible");

    // Exactly one should be auto-expanded (the one on the plan tree).
    let expanded_count = [&holder_a, &holder_b]
        .iter()
        .filter(|h| h.unwrap().nav.is_expanded)
        .count();
    assert_eq!(
        expanded_count, 1,
        "Exactly one Holder should be auto-expanded"
    );

    // Shared should be visible (child of the auto-expanded Holder).
    let shared = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("Shared"));
    assert!(
        shared.is_some(),
        "Shared should be visible under the auto-expanded Holder"
    );
}

#[test]
fn test_plan_reached_gc_roots_for_root_holder_start() {
    // When the start node is directly retained by (GC roots), the plan
    // should report reached_gc_roots = true even though the tree is empty
    // (there are no deeper retainer paths to show).
    //
    // Uses make_mixed_retainers_snapshot: RootCategory (node 4) is
    // directly retained by (GC roots) (node 1).
    let snap = make_mixed_retainers_snapshot();
    assert!(
        snap.is_root_holder(NodeOrdinal(4)),
        "RootCategory should be directly retained by (GC roots)"
    );

    let plan = plan_gc_root_retainer_paths(
        &snap,
        NodeOrdinal(4),
        RetainerAutoExpandLimits {
            max_depth: 10,
            max_nodes: 100,
        },
    );

    assert!(
        plan.reached_gc_roots,
        "Plan should report reached_gc_roots for a node directly retained by (GC roots)"
    );
}

fn setup_weakrefs_7207_tui() -> (HeapSnapshot, App) {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/weakrefs.heapsnapshot"
    ));
    let ordinal = snap
        .node_for_snapshot_object_id(crate::types::NodeId(7207))
        .expect("@7207 should exist");

    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_retainers_target(ordinal, &snap);
    let plan = plan_gc_root_retainer_paths(
        &snap,
        ordinal,
        RetainerAutoExpandLimits {
            max_depth: 20,
            max_nodes: 2000,
        },
    );
    app.apply_retainers_plan(ordinal, plan, &snap);
    app.rebuild_rows(&snap);
    (snap, app)
}

#[test]
fn test_weakrefs_7207_tui_direct_retainers() {
    let (_snap, app) = setup_weakrefs_7207_tui();

    // Collect depth-0 rows (direct retainers of @7207).
    let root_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| r.nav.depth == 0)
        .collect();

    // Expected direct retainers at root level (labels contain these):
    let expected_labels = [
        "(Stack roots) @43",
        "global [JSGlobalObject] @7571",
        "system / Context @7217",
        "@2327",
        "system / WeakCell @21325",
        "system / PropertyCell @21255",
        "WeakRef @21195",
    ];

    for expected in &expected_labels {
        assert!(
            root_rows.iter().any(|r| r.render.label.contains(expected)),
            "Expected direct retainer containing {expected:?} but not found. \
                 Root labels: {:?}",
            root_rows
                .iter()
                .map(|r| r.render.label.as_ref())
                .collect::<Vec<_>>()
        );
    }

    assert_eq!(
        root_rows.len(),
        expected_labels.len(),
        "Expected {} direct retainers, got {}. Labels: {:?}",
        expected_labels.len(),
        root_rows.len(),
        root_rows
            .iter()
            .map(|r| r.render.label.as_ref())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_weakrefs_7207_tui_root_holders_marked() {
    let (_snap, app) = setup_weakrefs_7207_tui();

    // (Stack roots) @43 is a root holder — it should be marked.
    let stack_roots = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("(Stack roots) @43"))
        .expect("(Stack roots) @43 should be visible");
    assert!(
        stack_roots.render.is_root_holder,
        "(Stack roots) should be marked as root_holder"
    );

    // Non-root-holders should not be marked.
    let weakref = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("WeakRef @21195"))
        .expect("WeakRef @21195 should be visible");
    assert!(
        !weakref.render.is_root_holder,
        "WeakRef should not be marked as root_holder"
    );
}

#[test]
fn test_weakrefs_7207_tui_auto_expansion() {
    let (_snap, app) = setup_weakrefs_7207_tui();

    // On-plan retainers should be auto-expanded.
    let global = app
        .cached_rows
        .iter()
        .find(|r| r.nav.depth == 0 && r.render.label.contains("global [JSGlobalObject] @7571"))
        .expect("global @7571 should be at depth 0");
    assert!(
        global.nav.is_expanded,
        "global @7571 (on-plan) should be auto-expanded"
    );

    // Off-plan retainer should not be expanded.
    let weakref = app
        .cached_rows
        .iter()
        .find(|r| r.nav.depth == 0 && r.render.label.contains("WeakRef @21195"))
        .expect("WeakRef @21195 should be at depth 0");
    assert!(
        !weakref.nav.is_expanded,
        "WeakRef @21195 (off-plan) should not be auto-expanded"
    );

    // Auto-expanded tree should reach a GC root: look for (Handle scope)
    // or (Stack roots) at some depth > 0.
    let gc_root_leaf = app.cached_rows.iter().any(|r| {
        r.nav.depth > 0
            && (r.render.label.contains("(Handle scope)")
                || r.render.label.contains("(Stack roots)")
                || r.render.label.contains("(Global handles)")
                || r.render.label.contains("(Strong root list)"))
    });
    assert!(
        gc_root_leaf,
        "Auto-expanded tree should contain a GC root holder at depth > 0"
    );
}

// ── Unreachable-only summary filter tests ──────────────────────────

/// Snapshot with a mix of reachable and unreachable nodes in the same group.
/// - Nodes 2,3: object "MyObj" (reachable via GC roots)
/// - Nodes 4,5: object "MyObj" (unreachable — only via weak edge)
/// - Node 6: object "Other" (reachable)
fn make_mixed_reachable_snapshot() -> HeapSnapshot {
    let strings = vec![
        "".into(),           // 0
        "(GC roots)".into(), // 1
        "MyObj".into(),      // 2
        "Other".into(),      // 3
        "a".into(),          // 4
        "b".into(),          // 5
        "c".into(),          // 6
        "d".into(),          // 7
        "weak".into(),       // 8
    ];

    let n = |ordinal: u32| ordinal * 5;

    //              type name id  size edges
    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 4, // node 1: (GC roots), 4 edges (a,b,c + weak→4)
        3, 2, 10, 100, 0, // node 2: MyObj reachable, size=100
        3, 2, 11, 200, 0, // node 3: MyObj reachable, size=200
        3, 2, 12, 300, 1, // node 4: MyObj unreachable, size=300, 1 edge→5
        3, 2, 13, 400, 0, // node 5: MyObj unreachable, size=400
        3, 3, 14, 500, 0, // node 6: Other reachable, size=500
    ];

    let edges = vec![
        1u32,
        0,
        n(1), // root → (GC roots)
        2,
        4,
        n(2), // (GC roots) --property "a"--> MyObj @10
        2,
        5,
        n(3), // (GC roots) --property "b"--> MyObj @11
        2,
        6,
        n(6), // (GC roots) --property "c"--> Other @14
        6,
        8,
        n(4), // (GC roots) --weak "weak"--> MyObj @12
        2,
        7,
        n(5), // MyObj @12 --property "d"--> MyObj @13
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_unreachable_filter_hides_reachable_groups() {
    let snap = make_mixed_reachable_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Without filter: both MyObj and Other groups visible
    app.rebuild_rows(&snap);
    let groups: Vec<&str> = app
        .cached_rows
        .iter()
        .filter_map(|r| match r.render.kind {
            FlatRowKind::SummaryGroup { .. } => Some(r.render.label.as_ref()),
            _ => None,
        })
        .collect();
    assert!(
        groups.len() >= 2,
        "should have at least MyObj and Other groups"
    );

    // Enable unreachable filter
    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    let groups: Vec<&str> = app
        .cached_rows
        .iter()
        .filter_map(|r| match r.render.kind {
            FlatRowKind::SummaryGroup { .. } => Some(r.render.label.as_ref()),
            _ => None,
        })
        .collect();

    // Other group is fully reachable → hidden
    assert!(
        !groups.iter().any(|l| l.contains("Other")),
        "Other group should be hidden: {groups:?}"
    );
    // MyObj group has unreachable members → visible
    assert!(
        groups.iter().any(|l| l.contains("MyObj")),
        "MyObj group should be visible: {groups:?}"
    );
}

#[test]
fn test_unreachable_filter_shows_correct_count() {
    let snap = make_mixed_reachable_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    let myobj_group = app
        .cached_rows
        .iter()
        .find(|r| {
            matches!(r.render.kind, FlatRowKind::SummaryGroup { .. })
                && r.render.label.contains("MyObj")
        })
        .expect("MyObj group should be visible");

    // Only 2 unreachable MyObj nodes (nodes 4 and 5), not 4 total
    assert!(
        myobj_group.render.label.contains("\u{00d7}2"),
        "expected ×2 in label: {}",
        myobj_group.render.label
    );
}

#[test]
fn test_unreachable_filter_expanded_members() {
    let snap = make_mixed_reachable_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    // Find and expand the MyObj group
    let myobj_idx = app
        .cached_rows
        .iter()
        .position(|r| {
            matches!(r.render.kind, FlatRowKind::SummaryGroup { .. })
                && r.render.label.contains("MyObj")
        })
        .expect("MyObj group should exist");
    let id = app.cached_rows[myobj_idx].nav.id;
    let ck = app.cached_rows[myobj_idx].nav.children_key.clone();
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    // Collect member rows under the group
    let members: Vec<_> = app
        .cached_rows
        .iter()
        .filter(|r| {
            matches!(
                r.render.kind,
                FlatRowKind::HeapNode {
                    node_ordinal: Some(_),
                    ..
                }
            ) && r.nav.depth == 1
        })
        .collect();

    assert_eq!(members.len(), 2, "should show only 2 unreachable members");

    // All shown members should be unreachable (distance >= UNREACHABLE_BASE)
    for m in &members {
        match &m.render.kind {
            FlatRowKind::HeapNode { distance, .. } => {
                assert!(
                    distance.is_some_and(|d| d.is_unreachable()),
                    "member should be unreachable: {}",
                    m.render.label
                );
            }
            _ => panic!("expected HeapNode"),
        }
    }
}

#[test]
fn test_unreachable_filter_toggle_restores_all() {
    let snap = make_mixed_reachable_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.rebuild_rows(&snap);
    let count_before = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .count();

    // Toggle on
    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);
    let count_filtered = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .count();
    assert!(count_filtered < count_before, "filter should reduce groups");

    // Toggle off
    app.summary_unreachable_filter = UnreachableFilter::Off;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);
    let count_after = app
        .cached_rows
        .iter()
        .filter(|r| matches!(r.render.kind, FlatRowKind::SummaryGroup { .. }))
        .count();
    assert_eq!(
        count_before, count_after,
        "toggling off should restore all groups"
    );
}

#[test]
fn test_unreachable_plus_text_filter_hides_group_matching_only_reachable() {
    // Regression: when `u` + text filter are both active, a group should
    // not appear if the text filter only matches reachable members.
    //
    // Snapshot: (string) group with members "hello" (reachable) and
    // "world" (unreachable).  Filter text "hello" + unreachable-only
    // should hide the group because "hello" is reachable.
    let strings = vec![
        "".into(),           // 0
        "(GC roots)".into(), // 1
        "hello".into(),      // 2 — node name for reachable string
        "world".into(),      // 3 — node name for unreachable string
        "ref".into(),        // 4
        "weak".into(),       // 5
    ];

    let n = |ordinal: u32| ordinal * 5;
    // node types: synthetic=9, string=2
    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 2, // node 1: (GC roots), 2 edges
        2, 2, 10, 100, 0, // node 2: string "hello", reachable
        2, 3, 11, 200, 0, // node 3: string "world", unreachable
    ];
    let edges = vec![
        1u32,
        0,
        n(1), // root → (GC roots)
        2,
        4,
        n(2), // (GC roots) --property "ref"--> "hello"
        6,
        5,
        n(3), // (GC roots) --weak "weak"--> "world"
    ];
    let snap = build_snapshot(strings, nodes, edges);

    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Enable unreachable-only + text filter "hello"
    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_filter = "hello".to_string();
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    let groups: Vec<&str> = app
        .cached_rows
        .iter()
        .filter_map(|r| match r.render.kind {
            FlatRowKind::SummaryGroup { .. } => Some(r.render.label.as_ref()),
            _ => None,
        })
        .collect();

    // "hello" is reachable → no unreachable member matches → group hidden
    assert!(
        groups.is_empty(),
        "no group should be visible when text matches only reachable members: {groups:?}"
    );

    // Now filter "world" — that one IS unreachable, group should appear
    app.summary_filter = "world".to_string();
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    let groups: Vec<&str> = app
        .cached_rows
        .iter()
        .filter_map(|r| match r.render.kind {
            FlatRowKind::SummaryGroup { .. } => Some(r.render.label.as_ref()),
            _ => None,
        })
        .collect();

    assert_eq!(
        groups.len(),
        1,
        "group should appear for unreachable match: {groups:?}"
    );
    assert!(
        groups[0].contains("\u{00d7}1"),
        "count should be 1: {}",
        groups[0]
    );
}

#[test]
fn test_unreachable_filter_recomputes_group_sizes() {
    let snap = make_mixed_reachable_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Without filter: MyObj group has full sizes (100+200+300+400 = 1000 shallow)
    app.rebuild_rows(&snap);
    let myobj_full = app
        .cached_rows
        .iter()
        .find(|r| {
            matches!(r.render.kind, FlatRowKind::SummaryGroup { .. })
                && r.render.label.contains("MyObj")
        })
        .expect("MyObj group should exist");
    let full_shallow = match myobj_full.render.kind {
        FlatRowKind::SummaryGroup { shallow_size, .. } => shallow_size,
        _ => unreachable!(),
    };
    assert_eq!(full_shallow, 1000.0, "full group shallow = 100+200+300+400");

    // With unreachable filter: only nodes 4 (300) and 5 (400) → shallow = 700
    app.summary_unreachable_filter = UnreachableFilter::All;
    app.summary_state = TreeState::new();
    app.mark_rows_dirty();
    app.rebuild_rows(&snap);

    let myobj_filtered = app
        .cached_rows
        .iter()
        .find(|r| {
            matches!(r.render.kind, FlatRowKind::SummaryGroup { .. })
                && r.render.label.contains("MyObj")
        })
        .expect("MyObj group should exist in unreachable mode");
    let (filtered_shallow, _filtered_retained) = match myobj_filtered.render.kind {
        FlatRowKind::SummaryGroup {
            shallow_size,
            retained_size,
            ..
        } => (shallow_size, retained_size),
        _ => unreachable!(),
    };
    assert_eq!(filtered_shallow, 700.0, "unreachable shallow = 300+400");
    assert!(
        filtered_shallow < full_shallow,
        "filtered shallow ({filtered_shallow}) should be less than full ({full_shallow})"
    );
}

#[test]
fn test_weakrefs_7207_plan_tree_structure() {
    let snap = load_test_snapshot(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/weakrefs.heapsnapshot"
    ));
    let ordinal = snap
        .node_for_snapshot_object_id(crate::types::NodeId(7207))
        .expect("@7207 should exist");

    let plan = plan_gc_root_retainer_paths(
        &snap,
        ordinal,
        RetainerAutoExpandLimits {
            max_depth: 20,
            max_nodes: 2000,
        },
    );

    assert!(plan.reached_gc_roots, "Plan should reach GC roots");
    assert!(!plan.tree.is_empty(), "Plan tree should not be empty");

    // Collect top-level plan tree entries (direct retainers on GC-root paths).
    let top_labels: Vec<String> = plan
        .tree
        .iter()
        .map(|pe| {
            let name = snap.node_display_name(pe.retainer);
            let id = snap.node_id(pe.retainer);
            format!("{name} @{id}")
        })
        .collect();

    // global @7571 should be in the plan tree (it's on a GC-root path).
    assert!(
        top_labels.iter().any(|l| l.contains("@7571")),
        "global @7571 should be in plan tree. Top entries: {top_labels:?}"
    );

    // Every leaf of the plan tree should be a root holder.
    fn assert_leaves_are_root_holders(
        snap: &HeapSnapshot,
        edges: &[crate::print::retainers::RetainerPathEdge],
    ) {
        for pe in edges {
            if pe.children.is_empty() {
                assert!(
                    snap.is_root_holder(pe.retainer),
                    "Leaf {} @{} should be a root holder",
                    snap.node_display_name(pe.retainer),
                    snap.node_id(pe.retainer)
                );
            } else {
                assert_leaves_are_root_holders(snap, &pe.children);
            }
        }
    }
    assert_leaves_are_root_holders(&snap, &plan.tree);

    // WeakRef @21195 should NOT be in the plan tree (it doesn't reach GC roots
    // through strong paths, only weakly).
    assert!(
        !top_labels.iter().any(|l| l.contains("@21195")),
        "WeakRef @21195 should not be in plan tree (weak path)"
    );
}

#[test]
fn test_show_in_summary_from_other_view() {
    // show_in_summary from retainers view should switch to summary and
    // place the cursor on the target node.
    let count = EDGE_PAGE_SIZE + 10;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // Pick a node that is on the first page (ordinal index 0 in the aggregate).
    let target = app.sorted_aggregates[0].node_ordinals[0];

    // Switch away from Summary.
    app.set_view(ViewType::Retainers, &snap);
    assert_eq!(app.current_view, ViewType::Retainers);

    // Call show_in_summary.
    app.show_in_summary(target, &snap);
    assert_eq!(app.current_view, ViewType::Summary);

    // Cursor should be on the target node.
    let cursor_row = &app.cached_rows[app.summary_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on the target node"
    );
}

#[test]
fn test_show_in_summary_adjusts_window_for_distant_node() {
    // When the target ordinal is beyond the default paging window,
    // show_in_summary should adjust the window so the node is visible.
    let count = EDGE_PAGE_SIZE * 3;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // Pick a node near the end — well outside the default first page.
    let last_idx = count - 1;
    let target = app.sorted_aggregates[0].node_ordinals[last_idx];

    // Collapse the group first so show_in_summary starts fresh.
    let group_id = app.summary_ids[0];
    app.summary_state.expanded.remove(&group_id);
    app.summary_state
        .children_map
        .remove(&ChildrenKey::ClassMembers(0));

    app.show_in_summary(target, &snap);

    // Cursor must be on the target node.
    let cursor_row = &app.cached_rows[app.summary_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on the distant target node"
    );

    // The class member window should have been adjusted to include the target.
    let w = app
        .summary_state
        .class_member_windows
        .get(&0)
        .expect("window should be set");
    assert!(
        last_idx >= w.start && last_idx < w.start + w.count,
        "window {}-{} should contain index {last_idx}",
        w.start,
        w.start + w.count,
    );
}

#[test]
fn test_show_in_summary_within_summary_recenters_window() {
    // Pressing 's' while already in the summary view on a node that is
    // nested under edges (and whose class group has a different paging
    // window) should re-center the class member window and place the
    // cursor on the target.
    let count = EDGE_PAGE_SIZE * 3;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // Pick a node in the last page.
    let target_member_idx = count - 2;
    let target = app.sorted_aggregates[0].node_ordinals[target_member_idx];

    // Simulate pressing 's' on that node while in Summary view.
    // (In practice the user would be on an edge-child row pointing at this node.)
    app.show_in_summary(target, &snap);
    assert_eq!(app.current_view, ViewType::Summary);

    let cursor_row = &app.cached_rows[app.summary_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on the target within summary"
    );
}

#[test]
fn test_show_in_summary_already_visible_does_not_change_window() {
    // If the target is already within the current window, the window
    // should not change.
    let count = EDGE_PAGE_SIZE + 10;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // Target on the first page — index 5.
    let target = app.sorted_aggregates[0].node_ordinals[5];

    // Record the current window.
    let w_before = app
        .summary_state
        .class_member_windows
        .get(&0)
        .copied()
        .unwrap_or_default();

    app.show_in_summary(target, &snap);

    let w_after = app
        .summary_state
        .class_member_windows
        .get(&0)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        w_before.start, w_after.start,
        "window start should not change"
    );
    assert_eq!(
        w_before.count, w_after.count,
        "window count should not change"
    );

    let cursor_row = &app.cached_rows[app.summary_state.cursor];
    assert_eq!(cursor_row.node_ordinal(), Some(target));
}

// ── show_in_dominators tests ────────────────────────────────────────────

/// Build a chain snapshot:
///   node 0: synthetic root → node 1
///   node 1: (GC roots) → node 2
///   node 2: A → node 3
///   node 3: B → node 4
///   node 4: C (leaf)
///
/// Dominator tree: gc_roots → A → B → C
fn make_chain_snapshot() -> HeapSnapshot {
    let strings: Vec<String> = [
        "",           // 0
        "(GC roots)", // 1
        "A",          // 2
        "B",          // 3
        "C",          // 4
        "edge",       // 5
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let ni = |ordinal: u32| ordinal * 5;
    let nodes: Vec<u32> = vec![
        // type, name, id, self_size, edge_count
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 1, // 2: A
        3, 3, 4, 20, 1, // 3: B
        3, 4, 5, 30, 0, // 4: C (leaf)
    ];

    let edges: Vec<u32> = vec![
        // type, name_or_index, to_node
        1,
        0,
        ni(1), // root → (GC roots)
        2,
        5,
        ni(2), // (GC roots) → A
        2,
        5,
        ni(3), // A → B
        2,
        5,
        ni(4), // B → C
    ];

    build_snapshot(strings, nodes, edges)
}

fn make_chain_app() -> (HeapSnapshot, App, mpsc::Receiver<WorkItem>) {
    let snap = make_chain_snapshot();
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let app = App::new(&snap, Vec::new(), work_tx, result_rx);
    (snap, app, work_rx)
}

#[test]
fn test_show_in_dominators_expands_path_to_leaf() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(4); // C

    app.show_in_dominators(target, &snap);

    assert_eq!(app.current_view, ViewType::Dominators);
    let cursor_row = &app.cached_rows[app.dominators_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on C"
    );

    // The path gc_roots → A → B → C should all be visible.
    assert!(
        app.cached_rows
            .iter()
            .any(|r| r.node_ordinal() == Some(NodeOrdinal(1))),
        "(GC roots) should be visible"
    );
    assert!(
        app.cached_rows
            .iter()
            .any(|r| r.node_ordinal() == Some(NodeOrdinal(2))),
        "A should be visible"
    );
    assert!(
        app.cached_rows
            .iter()
            .any(|r| r.node_ordinal() == Some(NodeOrdinal(3))),
        "B should be visible"
    );
}

#[test]
fn test_show_in_dominators_intermediate_node() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(2); // A

    app.show_in_dominators(target, &snap);

    assert_eq!(app.current_view, ViewType::Dominators);
    let cursor_row = &app.cached_rows[app.dominators_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on A"
    );
}

#[test]
fn test_show_in_dominators_from_other_view() {
    let (snap, mut app, _work_rx) = make_chain_app();
    app.set_view(ViewType::Summary, &snap);
    app.rebuild_rows(&snap);

    let target = NodeOrdinal(3); // B
    app.show_in_dominators(target, &snap);

    assert_eq!(app.current_view, ViewType::Dominators);
    let cursor_row = &app.cached_rows[app.dominators_state.cursor];
    assert_eq!(cursor_row.node_ordinal(), Some(target));
}

#[test]
fn test_show_in_dominators_pushes_history() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(4);
    assert!(app.history.is_empty());

    app.show_in_dominators(target, &snap);

    assert_eq!(app.history.last(), Some(&target));
}

// ── show_in_containment tests ───────────────────────────────────────────

#[test]
fn test_show_in_containment_expands_path_to_leaf() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(4); // C

    app.show_in_containment(target, &snap);

    assert_eq!(app.current_view, ViewType::Containment);
    let cursor_row = &app.cached_rows[app.containment_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on C"
    );
}

#[test]
fn test_show_in_containment_intermediate_node() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(2); // A

    app.show_in_containment(target, &snap);

    assert_eq!(app.current_view, ViewType::Containment);
    let cursor_row = &app.cached_rows[app.containment_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on A"
    );
}

#[test]
fn test_show_in_containment_pushes_history() {
    let (snap, mut app, _work_rx) = make_chain_app();
    let target = NodeOrdinal(3);
    assert!(app.history.is_empty());

    app.show_in_containment(target, &snap);

    assert_eq!(app.history.last(), Some(&target));
}

#[test]
fn test_show_in_containment_respects_paging() {
    // When the target is beyond the default page, the edge window
    // should be adjusted to include it with the default page size,
    // not blown up to usize::MAX.
    let count = EDGE_PAGE_SIZE * 3;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // The paged_summary_app has:
    //   node 0: synthetic root → node 1
    //   node 1: (GC roots) → nodes 2..2+count (Foo objects)
    //
    // In containment, the path is: synthetic_root → (GC roots) → Foo.
    // Pick a target on the last page.
    let target = NodeOrdinal(2 + count - 1);

    app.show_in_containment(target, &snap);

    assert_eq!(app.current_view, ViewType::Containment);
    let cursor_row = &app.cached_rows[app.containment_state.cursor];
    assert_eq!(
        cursor_row.node_ordinal(),
        Some(target),
        "cursor should land on the distant target"
    );

    // The (GC roots) node is the parent whose edge window matters.
    // Find its NodeId by looking at its children_key.
    let gc_roots_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .expect("(GC roots) should be visible");
    let gc_roots_id = gc_roots_row.nav.id;

    let w = app
        .containment_state
        .edge_windows
        .get(&gc_roots_id)
        .expect("edge window should exist for (GC roots)");
    assert!(
        w.count <= EDGE_PAGE_SIZE * 2,
        "page size should stay reasonable, got count={}",
        w.count,
    );
    // The target should be within the window.
    let target_edge_pos = count - 1; // last Foo
    assert!(
        target_edge_pos >= w.start && target_edge_pos < w.start + w.count,
        "window {}-{} should contain edge position {target_edge_pos}",
        w.start,
        w.start + w.count,
    );
}

#[test]
fn test_show_in_containment_first_page_keeps_default_window() {
    // When the target is on the first page, the default edge window
    // should not be changed.
    let count = EDGE_PAGE_SIZE * 2;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    // Target on the first page (ordinal 2 = first Foo object, edge index 0).
    let target = NodeOrdinal(2);

    app.show_in_containment(target, &snap);

    assert_eq!(app.current_view, ViewType::Containment);
    let cursor_row = &app.cached_rows[app.containment_state.cursor];
    assert_eq!(cursor_row.node_ordinal(), Some(target));

    // The (GC roots) parent should still have the default window.
    let gc_roots_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .expect("(GC roots) should be visible");
    let gc_roots_id = gc_roots_row.nav.id;

    let w = app
        .containment_state
        .edge_windows
        .get(&gc_roots_id)
        .copied()
        .unwrap_or_default();
    assert_eq!(w.start, 0, "window start should still be 0");
    assert_eq!(
        w.count, EDGE_PAGE_SIZE,
        "window count should be default page size"
    );
}

#[test]
fn test_show_in_containment_status_line_present_when_paged() {
    // After auto-expanding to a target beyond the first page,
    // a paging status line should still be present.
    let count = EDGE_PAGE_SIZE * 3;
    let (snap, mut app, _work_rx) = make_paged_summary_app(count);

    let target = NodeOrdinal(2 + count - 1);
    app.show_in_containment(target, &snap);

    // There should be a status row with "X of Y refs" paging info.
    let has_status = app.cached_rows.iter().any(|r| {
        r.node_ordinal().is_none()
            && r.render.label.contains("of")
            && r.render.label.contains("refs")
            && !r.nav.has_children
    });
    assert!(has_status, "paging status line should be present");
}

#[test]
fn test_show_in_containment_preserves_expansion_on_same_page() {
    // Build a snapshot where the (GC roots) node has two children A and B,
    // and A has a child D.  The user expands A→D, then presses 'c' on B.
    // Because B is on the same page as A, the cached children (and thus
    // A's NodeId and expansion state) should be reused, so A stays expanded.
    //
    //   node 0: synthetic root → node 1
    //   node 1: (GC roots) → node 2 (A), node 3 (B)
    //   node 2: A → node 4 (D)
    //   node 3: B (leaf)
    //   node 4: D (leaf)
    let strings: Vec<String> = [
        "",           // 0
        "(GC roots)", // 1
        "A",          // 2
        "B",          // 3
        "D",          // 4
        "edge",       // 5
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let ni = |ordinal: u32| ordinal * 5;
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 2, // 1: (GC roots) — 2 edges
        3, 2, 3, 10, 1, // 2: A — 1 edge
        3, 3, 4, 20, 0, // 3: B — leaf
        3, 4, 5, 30, 0, // 4: D — leaf
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        ni(1), // root → (GC roots)
        2,
        5,
        ni(2), // (GC roots) → A
        2,
        5,
        ni(3), // (GC roots) → B
        2,
        5,
        ni(4), // A → D
    ];

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Navigate to containment and expand (GC roots) → A → D.
    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // Expand (GC roots).
    let gc_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(1)))
        .unwrap();
    let gc_id = gc_row.nav.id;
    let gc_ck = gc_row.nav.children_key.clone();
    app.expand(gc_id, gc_ck, &snap);
    app.rebuild_rows(&snap);

    // Expand A.
    let a_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    let a_id = a_row.nav.id;
    let a_ck = a_row.nav.children_key.clone();
    app.expand(a_id, a_ck, &snap);
    app.rebuild_rows(&snap);

    // D should now be visible.
    assert!(
        app.cached_rows
            .iter()
            .any(|r| r.node_ordinal() == Some(NodeOrdinal(4))),
        "D should be visible after expanding A"
    );

    // Record A's NodeId — it should be preserved after show_in_containment.
    let a_id_before = a_id;

    // Now press 'c' on B — it's on the same page as A.
    app.show_in_containment(NodeOrdinal(3), &snap);

    // Cursor should land on B.
    let cursor_row = &app.cached_rows[app.containment_state.cursor];
    assert_eq!(cursor_row.node_ordinal(), Some(NodeOrdinal(3)));

    // A should still have the same NodeId (children were reused, not recomputed).
    let a_row_after = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .unwrap();
    assert_eq!(
        a_row_after.nav.id, a_id_before,
        "A's NodeId should be preserved (children reused)"
    );

    // A should still be expanded and D visible.
    assert!(a_row_after.nav.is_expanded, "A should still be expanded");
    assert!(
        app.cached_rows
            .iter()
            .any(|r| r.node_ordinal() == Some(NodeOrdinal(4))),
        "D should still be visible (expansion preserved)"
    );
}

/// Build a snapshot where (GC roots) has a **weak** edge to a node ("Target"),
/// so Target's retainer row is both `is_root_holder` and `is_weak`.
/// The render priority must give cyan (weak) rather than red (root holder).
fn make_weak_root_snapshot() -> HeapSnapshot {
    // Strings: 0="", 1="(GC roots)", 2="Holder", 3="Target", 4="ref", 5="weak_ref"
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "Holder".into(),
        "Target".into(),
        "ref".into(),
        "weak_ref".into(),
    ];

    let n = |ordinal: u32| ordinal * 5; // node_field_count = 5

    //              type name id  size edges
    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge → Holder (strong)
        3, 2, 3, 100, 1, // node 2: Holder, 1 weak edge → Target
        3, 3, 5, 200, 0, // node 3: Target, 0 edges
    ];

    // edge type indices: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element[0]--> (GC roots)
        2,
        4,
        n(2), // (GC roots) --property "ref"--> Holder
        6,
        5,
        n(3), // Holder --weak "weak_ref"--> Target
    ];

    build_snapshot(strings, nodes, edges)
}

#[test]
fn test_weak_root_holder_displays_cyan() {
    let snap = make_weak_root_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Open retainers for Target (node 3, id=5).
    let target_ord = snap
        .node_for_snapshot_object_id(crate::types::NodeId(5))
        .expect("Target @5 should exist");
    app.set_retainers_target(target_ord, &snap);
    app.rebuild_rows(&snap);

    // Holder (node 2) retains Target via a weak edge and is itself held by (GC roots).
    let holder_row = app
        .cached_rows
        .iter()
        .find(|r| r.render.label.contains("Holder"))
        .expect("Holder should appear as retainer of Target");

    assert!(holder_row.render.is_weak, "edge to Target is weak");
    assert!(
        holder_row.render.is_root_holder,
        "Holder is directly held by (GC roots)"
    );
}

#[test]
fn test_extension_name_replaces_url_in_contexts_label() {
    // Strings: 0="", 1="(GC roots)", 2="system / NativeContext / chrome-extension://testid123/page.html"
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "system / NativeContext / chrome-extension://testid123/page.html",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Node 0: synthetic root (type=9, name=0, id=1, size=0, edges=1)
    // Node 1: GC roots       (type=9, name=1, id=3, size=0, edges=1)
    // Node 2: NativeContext   (type=0, name=2, id=5, size=100, edges=0)
    let nodes: Vec<u32> = vec![9, 0, 1, 0, 1, 9, 1, 3, 0, 1, 0, 2, 5, 100, 0];

    let node_index = |ordinal: u32| ordinal * 5;
    // Edge from root -> GC roots, edge from GC roots -> NativeContext
    let edges: Vec<u32> = vec![1, 0, node_index(1), 1, 0, node_index(2)];

    let snap = build_snapshot(strings, nodes, edges);
    assert_eq!(
        snap.native_contexts().len(),
        1,
        "should find one native context"
    );

    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Pre-fill the extension name (no network request)
    app.extension_names
        .insert("testid123".to_string(), "My Test Extension".to_string());

    // Switch to contexts view
    app.set_view(ViewType::Contexts, &snap);
    app.rebuild_rows(&snap);

    let ctx_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("should find the native context row");

    assert!(
        ctx_row.render.label.contains("My Test Extension"),
        "expected resolved extension name in label, got: {}",
        ctx_row.render.label
    );
    assert!(
        ctx_row.render.label.contains("testid123"),
        "expected extension ID in label, got: {}",
        ctx_row.render.label
    );
    assert!(
        !ctx_row.render.label.contains("chrome-extension://"),
        "URL should be replaced by extension name, got: {}",
        ctx_row.render.label
    );
}

#[test]
fn test_extension_url_unchanged_when_name_not_resolved() {
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "system / NativeContext / chrome-extension://unknownext/index.html",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let nodes: Vec<u32> = vec![9, 0, 1, 0, 1, 9, 1, 3, 0, 1, 0, 2, 5, 100, 0];
    let node_index = |ordinal: u32| ordinal * 5;
    let edges: Vec<u32> = vec![1, 0, node_index(1), 1, 0, node_index(2)];

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    // Do NOT pre-fill extension_names
    app.set_view(ViewType::Contexts, &snap);
    app.rebuild_rows(&snap);

    let ctx_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("should find the native context row");

    assert!(
        ctx_row
            .render
            .label
            .contains("chrome-extension://unknownext/index.html"),
        "URL should remain when no extension name is available, got: {}",
        ctx_row.render.label
    );
}

#[test]
fn test_multiple_contexts_same_extension_id_resolved() {
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "system / NativeContext / chrome-extension://sameid/page1.html",
        "system / NativeContext / chrome-extension://sameid/page2.html",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, 9, 1, 3, 0, 2, 0, 2, 5, 100, 0, 0, 3, 7, 200, 0,
    ];
    let node_index = |ordinal: u32| ordinal * 5;
    let edges: Vec<u32> = vec![
        1,
        0,
        node_index(1),
        1,
        0,
        node_index(2),
        1,
        0,
        node_index(3),
    ];

    let snap = build_snapshot(strings, nodes, edges);
    assert_eq!(snap.native_contexts().len(), 2);

    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_view(ViewType::Contexts, &snap);

    // Only one ExtensionName work item should be queued (deduped)
    let mut ext_lookups = Vec::new();
    while let Ok(item) = work_rx.try_recv() {
        if let WorkItem::ExtensionName(id) = item {
            ext_lookups.push(id);
        }
    }
    assert_eq!(
        ext_lookups,
        vec!["sameid"],
        "should queue exactly one lookup for the shared extension ID"
    );

    // Now pre-fill the resolved name and rebuild
    app.extension_names
        .insert("sameid".to_string(), "Shared Extension".to_string());
    app.rebuild_rows(&snap);

    // Both context rows should show the resolved name
    let ctx_rows: Vec<_> = app
        .cached_rows
        .iter()
        .filter(
            |r| matches!(r.node_ordinal(), Some(o) if o == NodeOrdinal(2) || o == NodeOrdinal(3)),
        )
        .collect();
    assert_eq!(ctx_rows.len(), 2);
    for row in &ctx_rows {
        assert!(
            row.render.label.contains("Shared Extension"),
            "expected resolved name in both context rows, got: {}",
            row.render.label
        );
        assert!(
            !row.render.label.contains("chrome-extension://"),
            "URL should be replaced in both rows, got: {}",
            row.render.label
        );
    }
}

#[test]
fn test_non_extension_url_not_affected_by_extension_names() {
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "system / NativeContext / https://example.com/app",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let nodes: Vec<u32> = vec![9, 0, 1, 0, 1, 9, 1, 3, 0, 1, 0, 2, 5, 100, 0];
    let node_index = |ordinal: u32| ordinal * 5;
    let edges: Vec<u32> = vec![1, 0, node_index(1), 1, 0, node_index(2)];

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_view(ViewType::Contexts, &snap);
    app.rebuild_rows(&snap);

    // No extension lookups should be queued
    let ext_lookups: Vec<_> = std::iter::from_fn(|| work_rx.try_recv().ok())
        .filter(|item| matches!(item, WorkItem::ExtensionName(_)))
        .collect();
    assert!(
        ext_lookups.is_empty(),
        "should not queue lookups for non-extension URLs"
    );

    let ctx_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("should find the native context row");

    assert!(
        ctx_row.render.label.contains("https://example.com/app"),
        "non-extension URL should remain unchanged, got: {}",
        ctx_row.render.label
    );
}

#[test]
fn test_extension_name_via_drain_results_updates_label() {
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "system / NativeContext / chrome-extension://asyncext/bg.html",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let nodes: Vec<u32> = vec![9, 0, 1, 0, 1, 9, 1, 3, 0, 1, 0, 2, 5, 100, 0];
    let node_index = |ordinal: u32| ordinal * 5;
    let edges: Vec<u32> = vec![1, 0, node_index(1), 1, 0, node_index(2)];

    let snap = build_snapshot(strings, nodes, edges);
    let (work_tx, _work_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);

    app.set_view(ViewType::Contexts, &snap);
    app.rebuild_rows(&snap);

    // Before resolution: URL should be present
    let ctx_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("should find context row");
    assert!(
        ctx_row
            .render
            .label
            .contains("chrome-extension://asyncext/bg.html"),
        "URL should be present before resolution, got: {}",
        ctx_row.render.label
    );

    // Simulate background worker completing
    result_tx
        .send(WorkResult::ExtensionName {
            extension_id: "asyncext".to_string(),
            name: Some("Async Extension".to_string()),
        })
        .unwrap();

    let changed = app.drain_results(&snap);
    assert!(changed, "drain_results should report a change");

    // Rows should be dirty, rebuild them
    app.rebuild_rows(&snap);

    let ctx_row = app
        .cached_rows
        .iter()
        .find(|r| r.node_ordinal() == Some(NodeOrdinal(2)))
        .expect("should find context row after resolution");
    assert!(
        ctx_row.render.label.contains("Async Extension"),
        "label should contain resolved name after drain_results, got: {}",
        ctx_row.render.label
    );
    assert!(
        !ctx_row.render.label.contains("chrome-extension://"),
        "URL should be replaced after drain_results, got: {}",
        ctx_row.render.label
    );
}

/// Build a three-level chain: root → (GC roots) → Parent → Child → Grandchild
fn make_chain_for_collapse_snapshot() -> HeapSnapshot {
    let strings: Vec<String> = [
        "",           // 0
        "(GC roots)", // 1
        "Parent",     // 2
        "Child",      // 3
        "Grandchild", // 4
        "child",      // 5  (edge name)
        "grandchild", // 6  (edge name)
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let node_index = |ordinal: u32| ordinal * 5;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // [0] synthetic root
        9, 1, 2, 0, 1, // [1] (GC roots)
        3, 2, 3, 100, 1, // [2] Parent  (1 edge → Child)
        3, 3, 4, 50, 1, // [3] Child   (1 edge → Grandchild)
        3, 4, 5, 10, 0, // [4] Grandchild (leaf)
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        node_index(1), // root → (GC roots)
        1,
        0,
        node_index(2), // (GC roots) → Parent
        2,
        5,
        node_index(3), // Parent → Child
        2,
        6,
        node_index(4), // Child → Grandchild
    ];

    build_snapshot(strings, nodes, edges)
}

/// Collapsing a node must evict children_map and edge_windows entries for
/// the collapsed node and all its descendants.
#[test]
fn collapse_evicts_children_map_and_edge_windows() {
    let snap = make_chain_for_collapse_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);
    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // Expand (GC roots) → Parent becomes visible
    let gcr_idx = find_row_index_by_ordinal(&app, NodeOrdinal(1));
    let gcr_id = app.cached_rows[gcr_idx].nav.id;
    let gcr_ck = app.cached_rows[gcr_idx].nav.children_key.clone();
    app.expand(gcr_id, gcr_ck.clone(), &snap);
    app.rebuild_rows(&snap);

    // Expand Parent → Child becomes visible
    let parent_idx = find_row_index_by_ordinal(&app, NodeOrdinal(2));
    let parent_id = app.cached_rows[parent_idx].nav.id;
    let parent_ck = app.cached_rows[parent_idx].nav.children_key.clone();
    app.expand(parent_id, parent_ck.clone(), &snap);
    app.rebuild_rows(&snap);

    // Expand Child → Grandchild becomes visible
    let child_idx = find_row_index_by_ordinal(&app, NodeOrdinal(3));
    let child_id = app.cached_rows[child_idx].nav.id;
    let child_ck = app.cached_rows[child_idx].nav.children_key.clone();
    app.expand(child_id, child_ck.clone(), &snap);
    app.rebuild_rows(&snap);

    // Insert a custom edge_window for Child to verify it gets cleaned up.
    app.containment_state
        .edge_windows
        .insert(child_id, EdgeWindow { start: 0, count: 5 });

    // Verify caches are populated.
    let parent_ck = parent_ck.unwrap();
    let child_ck = child_ck.unwrap();
    assert!(
        app.containment_state.children_map.contains_key(&parent_ck),
        "parent children should be cached before collapse"
    );
    assert!(
        app.containment_state.children_map.contains_key(&child_ck),
        "child children should be cached before collapse"
    );
    assert!(
        app.containment_state.edge_windows.contains_key(&child_id),
        "child edge_window should exist before collapse"
    );

    // Collapse (GC roots) — should evict everything under it.
    app.collapse(gcr_id);
    app.rebuild_rows(&snap);

    // children_map entries for Parent and Child should be gone.
    assert!(
        !app.containment_state.children_map.contains_key(&parent_ck),
        "parent children_map entry should be evicted after collapse"
    );
    assert!(
        !app.containment_state.children_map.contains_key(&child_ck),
        "child children_map entry should be evicted after collapse"
    );
    // The (GC roots) own children entry should also be evicted.
    let gcr_ck = gcr_ck.unwrap();
    assert!(
        !app.containment_state.children_map.contains_key(&gcr_ck),
        "collapsed node's own children_map entry should be evicted"
    );

    // edge_windows for descendant rows should be evicted too.
    assert!(
        !app.containment_state.edge_windows.contains_key(&child_id),
        "descendant edge_window should be evicted after collapse"
    );
}

/// Build a snapshot where a node has an edge pointing back to itself.
///
///   [0] synthetic root  --element-->  [1] (GC roots)  --element-->  [2] SelfRef
///                                                                        |
///                                                                        +--property "self"--> [2] SelfRef  (self-cycle)
///                                                                        +--property "leaf"--> [3] Leaf
fn make_self_cycle_snapshot() -> HeapSnapshot {
    let strings: Vec<String> = [
        "",           // 0
        "(GC roots)", // 1
        "SelfRef",    // 2
        "Leaf",       // 3
        "self",       // 4
        "leaf",       // 5
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let node_index = |ordinal: u32| ordinal * 5;

    // Nodes: [type, name, id, self_size, edge_count]
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // [0] synthetic root (1 edge to GC roots)
        9, 1, 2, 0, 1, // [1] (GC roots) (1 edge to SelfRef)
        3, 2, 3, 100, 2, // [2] SelfRef (2 edges: self + leaf)
        3, 3, 4, 50, 0, // [3] Leaf (no edges)
    ];

    // Edges: [type, name_or_index, to_node]
    let edges: Vec<u32> = vec![
        1,
        0,
        node_index(1), // root -> (GC roots)
        1,
        0,
        node_index(2), // (GC roots) -> SelfRef
        2,
        4,
        node_index(2), // SelfRef -> SelfRef  (SELF-CYCLE)
        2,
        5,
        node_index(3), // SelfRef -> Leaf
    ];

    build_snapshot(strings, nodes, edges)
}

/// Expanding a self-referencing node must not cause unbounded row growth.
///
/// Before the fix, `ChildrenKey::Edges` was keyed by `NodeOrdinal` alone, so
/// the parent and self-child shared the same cache entry. Expanding the child
/// re-entered the same cached children vector, producing infinite recursion
/// (or OOM). Now `Edges` is keyed by `(NodeId, NodeOrdinal)`, so each row
/// gets its own cache slot.
#[test]
fn self_cycle_does_not_cause_infinite_expansion() {
    let snap = make_self_cycle_snapshot();
    let (work_tx, _work_rx) = mpsc::channel();
    let (_result_tx, result_rx) = mpsc::channel();
    let mut app = App::new(&snap, Vec::new(), work_tx, result_rx);
    app.current_view = ViewType::Containment;
    app.rebuild_rows(&snap);

    // First expand (GC roots) so SelfRef becomes visible.
    let gcr_ord = NodeOrdinal(1);
    let gcr_idx = find_row_index_by_ordinal(&app, gcr_ord);
    let (gcr_id, gcr_ck) = (
        app.cached_rows[gcr_idx].nav.id,
        app.cached_rows[gcr_idx].nav.children_key.clone(),
    );
    app.expand(gcr_id, gcr_ck, &snap);
    app.rebuild_rows(&snap);

    // Find the first occurrence of SelfRef (ordinal 2).
    let self_ref_ord = NodeOrdinal(2);
    let idx = find_row_index_by_ordinal(&app, self_ref_ord);
    let (id, ck) = (
        app.cached_rows[idx].nav.id,
        app.cached_rows[idx].nav.children_key.clone(),
    );

    // Expand it — this computes children for SelfRef, which includes itself.
    app.expand(id, ck, &snap);
    app.rebuild_rows(&snap);

    let rows_after_first = app.cached_rows.len();

    // Find the self-child (second occurrence of ordinal 2, nested under the first).
    let child_idx = app
        .cached_rows
        .iter()
        .enumerate()
        .find(|&(i, row)| i != idx && row.node_ordinal() == Some(self_ref_ord))
        .expect("self-child row should exist after expanding parent")
        .0;
    let child_id = app.cached_rows[child_idx].nav.id;
    let child_ck = app.cached_rows[child_idx].nav.children_key.clone();

    // The child must have a *different* ChildrenKey from the parent so it
    // gets its own cache entry.
    let parent_ck = app.cached_rows[idx].nav.children_key.clone();
    assert!(
        parent_ck != child_ck,
        "parent and self-child should have different ChildrenKey"
    );

    // Expand the self-child.
    app.expand(child_id, child_ck, &snap);
    app.rebuild_rows(&snap);

    let rows_after_second = app.cached_rows.len();

    // The second expansion should add a bounded number of new rows (the
    // children of the self-child: another SelfRef + Leaf + possibly a
    // status row), NOT an unbounded explosion.
    let new_rows = rows_after_second - rows_after_first;
    assert!(
        new_rows <= 10,
        "expanding self-child should add a small number of rows, got {new_rows}"
    );
}
