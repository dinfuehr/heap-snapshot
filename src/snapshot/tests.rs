use super::*;
use crate::types::{RawHeapSnapshot, SnapshotHeader, SnapshotMeta};

/// Builds a minimal snapshot with 5 nodes and 4 edges:
///
/// ```text
/// Node 0 (synthetic root): synthetic, "", id=1, size=0, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", id=2, size=0, 2 edges
/// Node 2: object, "Object", id=3, size=100, 1 edge
/// Node 3: string, "hello", id=5, size=50, 0 edges
/// Node 4: array, "Array", id=7, size=200, 0 edges
///
/// Edges:
///   root --element[0]--> (GC roots)
///   (GC roots) --"global"--> node2
///   (GC roots) --"arr"----> node4
///   node2 --"str"---> node3
/// ```
fn make_test_snapshot() -> HeapSnapshot {
    // node_fields: type(0), name(1), id(2), self_size(3), edge_count(4)
    let node_fields: Vec<String> = ["type", "name", "id", "self_size", "edge_count"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let nfc = node_fields.len(); // 5

    // Standard V8 node types
    let node_type_enum: Vec<String> = [
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

    // Edge fields: type(0), name_or_index(1), to_node(2)
    let edge_fields: Vec<String> = ["type", "name_or_index", "to_node"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let efc = edge_fields.len(); // 3

    // Standard V8 edge types (code adds "invisible" at the end)
    let edge_type_enum: Vec<String> = [
        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Strings table
    // 0: "", 1: "(GC roots)", 2: "Object", 3: "hello", 4: "Array",
    // 5: "global", 6: "arr", 7: "str"
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "Object",
        "hello",
        "Array",
        "global",
        "arr",
        "str",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Type indices: array=1, string=2, object=3, synthetic=9
    // Edge types: element=1, property=2

    // Nodes: 5 nodes * 5 fields = 25 values
    //              type name id  size edges
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic, "", id=1, size=0, 1 edge
        9, 1, 2, 0, 2, // node 1: synthetic, "(GC roots)", id=2, size=0, 2 edges
        3, 2, 3, 100, 1, // node 2: object, "Object", id=3, size=100, 1 edge
        2, 3, 5, 50, 0, // node 3: string, "hello", id=5, size=50, 0 edges
        1, 4, 7, 200, 0, // node 4: array, "Array", id=7, size=200, 0 edges
    ];

    // Edges: 4 edges * 3 fields = 12 values
    // to_node stores node_index = ordinal * nfc
    let edges: Vec<u32> = vec![
        1,
        0,
        1 * nfc as u32, // edge 0: element, 0,        -> node 1 (GC roots)
        2,
        5,
        2 * nfc as u32, // edge 1: property, "global", -> node 2 (Object)
        2,
        6,
        4 * nfc as u32, // edge 2: property, "arr",    -> node 4 (Array)
        2,
        7,
        3 * nfc as u32, // edge 3: property, "str",    -> node 3 (hello)
    ];

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

fn make_js_global_snapshot() -> HeapSnapshot {
    let node_fields: Vec<String> = ["type", "name", "id", "self_size", "edge_count"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let nfc = node_fields.len();

    let node_type_enum: Vec<String> = [
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

    let edge_fields: Vec<String> = ["type", "name_or_index", "to_node"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let efc = edge_fields.len();

    let edge_type_enum: Vec<String> = [
        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

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

    let nodes_u32 = |ordinal: u32| ordinal * nfc as u32;
    let edges: Vec<u32> = vec![
        1,
        0,
        nodes_u32(1),
        2,
        10,
        nodes_u32(2),
        2,
        11,
        nodes_u32(3),
        2,
        12,
        nodes_u32(4),
        2,
        13,
        nodes_u32(5),
        2,
        14,
        nodes_u32(6),
        2,
        15,
        nodes_u32(7),
        2,
        16,
        nodes_u32(8),
        2,
        14,
        nodes_u32(9),
        2,
        17,
        nodes_u32(10),
        2,
        16,
        nodes_u32(11),
        2,
        18,
        nodes_u32(6),
        2,
        19,
        nodes_u32(7),
        2,
        20,
        nodes_u32(8),
        2,
        18,
        nodes_u32(9),
        2,
        21,
        nodes_u32(10),
        2,
        20,
        nodes_u32(11),
    ];

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

#[test]
fn test_gc_roots_ordinal() {
    let snap = make_test_snapshot();
    // root_node_ordinal returns (GC roots) which is ordinal 1
    assert_eq!(snap.gc_roots_ordinal(), NodeOrdinal(1));
}

#[test]
fn test_node_id() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_id(NodeOrdinal(0)), NodeId(1)); // synthetic root
    assert_eq!(snap.node_id(NodeOrdinal(1)), NodeId(2)); // (GC roots)
    assert_eq!(snap.node_id(NodeOrdinal(2)), NodeId(3)); // Object
    assert_eq!(snap.node_id(NodeOrdinal(3)), NodeId(5)); // hello
    assert_eq!(snap.node_id(NodeOrdinal(4)), NodeId(7)); // Array
}

#[test]
fn test_node_self_size() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_self_size(NodeOrdinal(0)), 0); // synthetic root
    assert_eq!(snap.node_self_size(NodeOrdinal(1)), 0); // (GC roots)
    assert_eq!(snap.node_self_size(NodeOrdinal(2)), 100); // Object
    assert_eq!(snap.node_self_size(NodeOrdinal(3)), 50); // hello
    assert_eq!(snap.node_self_size(NodeOrdinal(4)), 200); // Array
}

#[test]
fn test_node_distance() {
    let snap = make_test_snapshot();
    // BFS from (GC roots) ordinal 1
    assert_eq!(snap.node_distance(NodeOrdinal(0)), 0); // synthetic root (fallback BFS)
    assert_eq!(snap.node_distance(NodeOrdinal(1)), 0); // (GC roots)
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1); // Object
    assert_eq!(snap.node_distance(NodeOrdinal(3)), 2); // hello
    assert_eq!(snap.node_distance(NodeOrdinal(4)), 1); // Array
}

#[test]
fn test_node_retained_size() {
    let snap = make_test_snapshot();
    // Dominator tree rooted at (GC roots) ordinal 1
    assert_eq!(snap.node_retained_size(NodeOrdinal(1)), 350.0); // (GC roots): 0+100+50+200
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 150.0); // Object: 100+50
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 50.0); // hello
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 200.0); // Array
}

#[test]
fn test_node_for_snapshot_object_id() {
    let snap = make_test_snapshot();
    assert_eq!(
        snap.node_for_snapshot_object_id(NodeId(1)),
        Some(NodeOrdinal(0))
    );
    assert_eq!(
        snap.node_for_snapshot_object_id(NodeId(2)),
        Some(NodeOrdinal(1))
    );
    assert_eq!(
        snap.node_for_snapshot_object_id(NodeId(3)),
        Some(NodeOrdinal(2))
    );
    assert_eq!(
        snap.node_for_snapshot_object_id(NodeId(5)),
        Some(NodeOrdinal(3))
    );
    assert_eq!(
        snap.node_for_snapshot_object_id(NodeId(7)),
        Some(NodeOrdinal(4))
    );
    assert_eq!(snap.node_for_snapshot_object_id(NodeId(999)), None);
}

#[test]
fn test_is_root() {
    let snap = make_test_snapshot();
    // is_root checks against gc_roots_ordinal = 1
    assert!(!snap.is_root(NodeOrdinal(0)));
    assert!(snap.is_root(NodeOrdinal(1)));
    assert!(!snap.is_root(NodeOrdinal(2)));
    assert!(!snap.is_root(NodeOrdinal(3)));
    assert!(!snap.is_root(NodeOrdinal(4)));
}

#[test]
fn test_node_edge_count() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_edge_count(NodeOrdinal(0)), 1); // synthetic root -> (GC roots)
    assert_eq!(snap.node_edge_count(NodeOrdinal(1)), 2); // (GC roots) -> Object, Array
    assert_eq!(snap.node_edge_count(NodeOrdinal(2)), 1); // Object -> hello
    assert_eq!(snap.node_edge_count(NodeOrdinal(3)), 0);
    assert_eq!(snap.node_edge_count(NodeOrdinal(4)), 0);
}

#[test]
fn test_get_edges() {
    let snap = make_test_snapshot();

    // Synthetic root -> (GC roots)
    let root_edges = snap.get_edges(NodeOrdinal(0));
    assert_eq!(root_edges.len(), 1);
    assert_eq!(root_edges[0].1, NodeOrdinal(1)); // -> (GC roots)

    // (GC roots) -> Object, Array
    let gc_edges = snap.get_edges(NodeOrdinal(1));
    assert_eq!(gc_edges.len(), 2);
    assert_eq!(gc_edges[0].1, NodeOrdinal(2)); // -> Object
    assert_eq!(gc_edges[1].1, NodeOrdinal(4)); // -> Array

    // Object -> hello
    let n2_edges = snap.get_edges(NodeOrdinal(2));
    assert_eq!(n2_edges.len(), 1);
    assert_eq!(n2_edges[0].1, NodeOrdinal(3)); // -> hello

    assert!(snap.get_edges(NodeOrdinal(3)).is_empty());
    assert!(snap.get_edges(NodeOrdinal(4)).is_empty());
}

#[test]
fn test_get_retainers() {
    let snap = make_test_snapshot();

    // Synthetic root has no retainers
    assert!(snap.get_retainers(NodeOrdinal(0)).is_empty());

    // (GC roots) retained by synthetic root (ordinal 0)
    let r1 = snap.get_retainers(NodeOrdinal(1));
    assert_eq!(r1.len(), 1);
    assert_eq!(r1[0].1, NodeOrdinal(0));

    // Object retained by (GC roots) (ordinal 1)
    let r2 = snap.get_retainers(NodeOrdinal(2));
    assert_eq!(r2.len(), 1);
    assert_eq!(r2[0].1, NodeOrdinal(1));

    // hello retained by Object (ordinal 2)
    let r3 = snap.get_retainers(NodeOrdinal(3));
    assert_eq!(r3.len(), 1);
    assert_eq!(r3[0].1, NodeOrdinal(2));

    // Array retained by (GC roots) (ordinal 1)
    let r4 = snap.get_retainers(NodeOrdinal(4));
    assert_eq!(r4.len(), 1);
    assert_eq!(r4[0].1, NodeOrdinal(1));
}

#[test]
fn test_edge_name() {
    let snap = make_test_snapshot();

    // (GC roots) edges
    let gc_edges = snap.get_edges(NodeOrdinal(1));
    assert_eq!(snap.edge_name(gc_edges[0].0), "global");
    assert_eq!(snap.edge_name(gc_edges[1].0), "arr");

    // Object edges
    let n2_edges = snap.get_edges(NodeOrdinal(2));
    assert_eq!(snap.edge_name(n2_edges[0].0), "str");
}

#[test]
fn test_edge_type_name() {
    let snap = make_test_snapshot();
    // Synthetic root -> (GC roots) is element type
    let root_edges = snap.get_edges(NodeOrdinal(0));
    assert_eq!(snap.edge_type_name(root_edges[0].0), "element");
    // (GC roots) -> Object, Array are property type
    let gc_edges = snap.get_edges(NodeOrdinal(1));
    assert_eq!(snap.edge_type_name(gc_edges[0].0), "property");
    assert_eq!(snap.edge_type_name(gc_edges[1].0), "property");
}

#[test]
fn test_is_invisible_edge() {
    let snap = make_test_snapshot();
    let gc_edges = snap.get_edges(NodeOrdinal(1));
    assert!(!snap.is_invisible_edge(gc_edges[0].0));
    assert!(!snap.is_invisible_edge(gc_edges[1].0));
}

#[test]
fn test_node_type_name() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_type_name(NodeOrdinal(0)), "synthetic");
    assert_eq!(snap.node_type_name(NodeOrdinal(1)), "synthetic");
    assert_eq!(snap.node_type_name(NodeOrdinal(2)), "object");
    assert_eq!(snap.node_type_name(NodeOrdinal(3)), "string");
    assert_eq!(snap.node_type_name(NodeOrdinal(4)), "array");
}

#[test]
fn test_node_class_name() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_class_name(NodeOrdinal(0)), "(synthetic)");
    assert_eq!(snap.node_class_name(NodeOrdinal(1)), "(synthetic)");
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "Object");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "(string)");
    assert_eq!(snap.node_class_name(NodeOrdinal(4)), "(array)");
}

#[test]
fn test_node_display_name() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_display_name(NodeOrdinal(0)), "");
    assert_eq!(snap.node_display_name(NodeOrdinal(1)), "(GC roots)");
    assert_eq!(snap.node_display_name(NodeOrdinal(2)), "{str}");
    assert_eq!(snap.node_display_name(NodeOrdinal(3)), "hello");
    assert_eq!(snap.node_display_name(NodeOrdinal(4)), "Array");
}

#[test]
fn test_normalize_constructor_type_for_js_globals() {
    assert_eq!(
        HeapSnapshot::normalize_constructor_type("Window (global*)"),
        Some("[JSGlobalObject]")
    );
    assert_eq!(
        HeapSnapshot::normalize_constructor_type("Window (global*) / https://example.test"),
        Some("[JSGlobalObject]")
    );
    assert_eq!(
        HeapSnapshot::normalize_constructor_type("Window (global)"),
        Some("[JSGlobalProxy]")
    );
    assert_eq!(
        HeapSnapshot::normalize_constructor_type("Window (global) / <detached>"),
        Some("[JSGlobalProxy]")
    );
    assert_eq!(HeapSnapshot::normalize_constructor_type("Window"), None);
}

#[test]
fn test_normalize_display_name_for_js_globals() {
    assert_eq!(
        HeapSnapshot::normalize_display_name("Window (global*) / https://example.test"),
        "Window [JSGlobalObject] / https://example.test"
    );
    assert_eq!(
        HeapSnapshot::normalize_display_name("Window (global) / <detached>"),
        "Window [JSGlobalProxy] / <detached>"
    );
    assert_eq!(HeapSnapshot::normalize_display_name("Window"), "Window");
}

#[test]
fn test_find_js_globals_and_common_fields() {
    let snap = make_js_global_snapshot();

    assert_eq!(snap.js_global_objects(), &[2, 3]);
    assert_eq!(snap.js_global_proxies(), &[4, 5]);

    assert!(snap.is_js_global_object(NodeOrdinal(2)));
    assert!(snap.is_js_global_proxy(NodeOrdinal(4)));

    assert!(snap.is_common_js_global_field(NodeOrdinal(2), "shared_a"));
    assert!(snap.is_common_js_global_field(NodeOrdinal(2), "shared_b"));
    assert!(!snap.is_common_js_global_field(NodeOrdinal(2), "specific_obj_a"));

    assert!(snap.is_common_js_global_field(NodeOrdinal(4), "proxy_shared_a"));
    assert!(snap.is_common_js_global_field(NodeOrdinal(4), "proxy_shared_b"));
    assert!(!snap.is_common_js_global_field(NodeOrdinal(4), "specific_proxy_a"));
}

#[test]
fn test_node_count() {
    let snap = make_test_snapshot();
    assert_eq!(snap.node_count(), 5);
}

#[test]
fn test_statistics() {
    let snap = make_test_snapshot();
    let stats = snap.get_statistics();
    assert_eq!(stats.total, 350.0);
    assert_eq!(stats.strings, 50.0);
    assert_eq!(stats.js_arrays, 200.0);
    assert_eq!(stats.system, 0.0);
    assert_eq!(stats.code, 0.0);
    assert_eq!(stats.native_total, 0.0);
    assert_eq!(stats.typed_arrays, 0.0);
    assert_eq!(stats.v8heap_total, 350.0);
}

#[test]
fn test_aggregates() {
    let snap = make_test_snapshot();
    let aggs = snap.aggregates_with_filter();

    // 3 entries: one each for Object, (string), (array)
    // Synthetic nodes have self_size=0, so (synthetic) is excluded
    assert_eq!(aggs.len(), 3);

    let obj = &aggs["Object"];
    assert_eq!(obj.count, 1);
    assert_eq!(obj.self_size, 100.0);
    assert_eq!(obj.max_ret, 150.0);
    assert_eq!(obj.distance, 1);

    let str_agg = &aggs["(string)"];
    assert_eq!(str_agg.count, 1);
    assert_eq!(str_agg.self_size, 50.0);
    assert_eq!(str_agg.max_ret, 50.0);
    assert_eq!(str_agg.distance, 2);

    let arr_agg = &aggs["(array)"];
    assert_eq!(arr_agg.count, 1);
    assert_eq!(arr_agg.self_size, 200.0);
    assert_eq!(arr_agg.max_ret, 200.0);
    assert_eq!(arr_agg.distance, 1);
}

// ====== Shared test helpers ======

fn s(strs: &[&str]) -> Vec<String> {
    strs.iter().map(|s| s.to_string()).collect()
}

fn standard_node_fields() -> Vec<String> {
    s(&["type", "name", "id", "self_size", "edge_count"])
}

fn standard_edge_fields() -> Vec<String> {
    s(&["type", "name_or_index", "to_node"])
}

fn standard_node_type_enum() -> Vec<String> {
    s(&[
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
    ])
}

fn standard_edge_type_enum() -> Vec<String> {
    s(&[
        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
    ])
}

fn build_snapshot(
    node_fields: Vec<String>,
    nodes: Vec<u32>,
    edges: Vec<u32>,
    strings: Vec<String>,
) -> HeapSnapshot {
    let nfc = node_fields.len();
    let efc = 3;
    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields,
                node_type_enum: standard_node_type_enum(),
                edge_fields: standard_edge_fields(),
                edge_type_enum: standard_edge_type_enum(),
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

// ====== Snapshot builders ======

/// Snapshot with a weak edge: node 2 --weak--> node 3, node 2 --property--> node 4
///
/// ```text
/// Node 0 (synthetic root): synthetic, "", id=1, size=0, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", id=2, size=0, 1 edge
/// Node 2: object, "Obj", id=3, size=100, 2 edges
/// Node 3: string, "weakTarget", id=5, size=80, 0 edges
/// Node 4: string, "strongTarget", id=7, size=60, 0 edges
///
/// Edges:
///   root --element[0]--> (GC roots)
///   (GC roots) --"obj"--> node2
///   node2 --weak "weak_ref"--> node3
///   node2 --"strong"--> node4
/// ```
fn make_weak_edge_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 3, 100, 2, // node 2: Obj
            2, 3, 5, 80, 0, // node 3: weakTarget
            2, 4, 7, 60, 0, // node 4: strongTarget
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots (element)
            2,
            5,
            n(2), // GC roots -> Obj (property "obj")
            6,
            7,
            n(3), // Obj -> weakTarget (weak "weak_ref")
            2,
            6,
            n(4), // Obj -> strongTarget (property "strong")
        ],
        s(&[
            "",
            "(GC roots)",
            "Obj",
            "weakTarget",
            "strongTarget",
            "obj",
            "strong",
            "weak_ref",
        ]),
    )
}

/// Snapshot with a concatenated string node that has "first" and "second"
/// internal edges pointing to regular string nodes.
///
/// ```text
/// Node 0 (synthetic root): synthetic, "", id=1, size=0, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", id=2, size=0, 1 edge
/// Node 2: object, "Container", id=3, size=10, 1 edge
/// Node 3: concatenated string, "cons_str", id=5, size=20, 2 edges
/// Node 4: string, "Hello", id=7, size=5, 0 edges
/// Node 5: string, "World", id=9, size=5, 0 edges
///
/// Edges:
///   root --element[0]--> (GC roots)
///   (GC roots) --"container"--> node2
///   node2 --"str"--> node3
///   node3 --internal "first"--> node4
///   node3 --internal "second"--> node5
/// ```
fn make_cons_string_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 3, 10, 1, // node 2: Container (object)
            10, 3, 5, 20, 2, // node 3: cons string (type=10)
            2, 4, 7, 5, 0, // node 4: "Hello"
            2, 5, 9, 5, 0, // node 5: "World"
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots (element)
            2,
            6,
            n(2), // GC roots -> Container (property "container")
            2,
            7,
            n(3), // Container -> cons_str (property "str")
            3,
            8,
            n(4), // cons_str -> Hello (internal "first")
            3,
            9,
            n(5), // cons_str -> World (internal "second")
        ],
        s(&[
            "",
            "(GC roots)",
            "Container",
            "cons_str",
            "Hello",
            "World",
            "container",
            "str",
            "first",
            "second",
        ]),
    )
}

/// Snapshot with multiple plain Objects sharing the same property shape,
/// used to test both plain_object_name display and interface inference.
///
/// ```text
/// Node 0 (synthetic root): synthetic, "", id=1, size=0, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", id=2, size=0, 3 edges
/// Node 2: object, "Object", id=3, size=100, 4 edges (alpha, beta, gamma, __proto__)
/// Node 3: object, "Object", id=5, size=100, 3 edges (alpha, beta, gamma)
/// Node 4: object, "Object", id=7, size=100, 3 edges (alpha, beta, gamma)
/// Node 5: string, "val1", id=9, size=10, 0 edges
/// Node 6: string, "val2", id=11, size=10, 0 edges
/// Node 7: string, "val3", id=13, size=10, 0 edges
/// ```
fn make_multi_property_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 3, // node 1: (GC roots), 3 edges
            3, 2, 3, 100, 4, // node 2: Object, 4 edges
            3, 2, 5, 100, 3, // node 3: Object, 3 edges
            3, 2, 7, 100, 3, // node 4: Object, 3 edges
            2, 3, 9, 10, 0, // node 5: val1
            2, 4, 11, 10, 0, // node 6: val2
            2, 5, 13, 10, 0, // node 7: val3
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots (element)
            2,
            6,
            n(2), // GC roots -> node 2 (property "obj1")
            2,
            7,
            n(3), // GC roots -> node 3 (property "obj2")
            2,
            8,
            n(4), // GC roots -> node 4 (property "obj3")
            2,
            9,
            n(5), // node 2 -> val1 (property "alpha")
            2,
            10,
            n(5), // node 2 -> val1 (property "beta")
            2,
            11,
            n(5), // node 2 -> val1 (property "gamma")
            2,
            12,
            n(5), // node 2 -> val1 (property "__proto__")
            2,
            9,
            n(6), // node 3 -> val2 (property "alpha")
            2,
            10,
            n(6), // node 3 -> val2 (property "beta")
            2,
            11,
            n(6), // node 3 -> val2 (property "gamma")
            2,
            9,
            n(7), // node 4 -> val3 (property "alpha")
            2,
            10,
            n(7), // node 4 -> val3 (property "beta")
            2,
            11,
            n(7), // node 4 -> val3 (property "gamma")
        ],
        s(&[
            "",
            "(GC roots)",
            "Object",
            "val1",
            "val2",
            "val3",
            "obj1",
            "obj2",
            "obj3",
            "alpha",
            "beta",
            "gamma",
            "__proto__",
        ]),
    )
}

/// Snapshot with a detachedness field and native nodes to test
/// detachedness reading and DOM state propagation.
///
/// ```text
/// Node 0 (synthetic root): synthetic, det=0
/// Node 1 (GC roots): synthetic, det=0
/// Node 2: native, "Document", det=1 (attached)
/// Node 3: native, "DetachedDiv", det=2 (detached)
/// Node 4: object, "ChildObj", det=0 (becomes 1 via propagation from node 2)
///
/// Edges:
///   root --element[0]--> (GC roots)
///   (GC roots) --"doc"--> node2
///   (GC roots) --"div"--> node3
///   node2 --"child"--> node4
/// ```
fn make_detachedness_snapshot() -> HeapSnapshot {
    let nfc = 6u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        s(&[
            "type",
            "name",
            "id",
            "self_size",
            "edge_count",
            "detachedness",
        ]),
        vec![
            //  type name id  size edges det
            9, 0, 1, 0, 1, 0, // node 0: synthetic root
            9, 1, 2, 0, 2, 0, // node 1: (GC roots)
            8, 2, 3, 100, 1, 1, // node 2: Document (native, attached)
            8, 3, 5, 50, 0, 2, // node 3: DetachedDiv (native, detached)
            3, 4, 7, 30, 0, 0, // node 4: ChildObj (object, unknown)
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots (element)
            2,
            5,
            n(2), // GC roots -> Document (property "doc")
            2,
            6,
            n(3), // GC roots -> DetachedDiv (property "div")
            2,
            7,
            n(4), // Document -> ChildObj (property "child")
        ],
        s(&[
            "",
            "(GC roots)",
            "Document",
            "DetachedDiv",
            "ChildObj",
            "doc",
            "div",
            "child",
        ]),
    )
}

// ====== 1. reachable_size tests ======

#[test]
fn test_reachable_size_basic() {
    let snap = make_test_snapshot();
    let info = snap.reachable_size(&[NodeOrdinal(2)]);
    assert_eq!(info.size, 150.0); // Object(100) + hello(50)
    assert!(info.native_contexts.is_empty());

    let info = snap.reachable_size(&[NodeOrdinal(4)]);
    assert_eq!(info.size, 200.0); // Array only
}

#[test]
fn test_reachable_size_multiple_roots() {
    let snap = make_test_snapshot();
    let info = snap.reachable_size(&[NodeOrdinal(2), NodeOrdinal(4)]);
    assert_eq!(info.size, 350.0); // Object(100) + hello(50) + Array(200)
}

#[test]
fn test_reachable_size_skips_weak_edges() {
    let snap = make_weak_edge_snapshot();
    let info = snap.reachable_size(&[NodeOrdinal(2)]);
    // Obj(100) + strongTarget(60) = 160, weakTarget(80) skipped
    assert_eq!(info.size, 160.0);
}

// ====== 3. cons_string_name tests ======

#[test]
fn test_cons_string_display_name() {
    let snap = make_cons_string_snapshot();
    assert_eq!(snap.node_display_name(NodeOrdinal(3)), "HelloWorld");
}

// ====== 4. plain_object_name tests ======

#[test]
fn test_plain_object_name_skips_proto() {
    let snap = make_multi_property_snapshot();
    // Node 2 has properties: alpha, beta, gamma, __proto__
    // __proto__ is skipped; alternating from start/end yields alpha, beta, gamma
    let name = snap.node_display_name(NodeOrdinal(2));
    assert_eq!(name, "{alpha, beta, gamma}");
}

#[test]
fn test_plain_object_name_multiple_properties() {
    let snap = make_multi_property_snapshot();
    // Node 3 has properties: alpha, beta, gamma (no __proto__)
    let name = snap.node_display_name(NodeOrdinal(3));
    assert_eq!(name, "{alpha, beta, gamma}");
}

// ====== 5. interface inference tests ======

#[test]
fn test_interface_inference_class_name() {
    let snap = make_multi_property_snapshot();
    // 3 Objects with shape {alpha, beta, gamma} should get interface class name
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "{alpha, beta, gamma}");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "{alpha, beta, gamma}");
    assert_eq!(snap.node_class_name(NodeOrdinal(4)), "{alpha, beta, gamma}");
}

// ====== 8. detachedness tests ======

#[test]
fn test_node_detachedness_values() {
    let snap = make_detachedness_snapshot();
    assert_eq!(snap.node_detachedness(NodeOrdinal(0)), 0); // synthetic
    assert_eq!(snap.node_detachedness(NodeOrdinal(1)), 0); // GC roots
    assert_eq!(snap.node_detachedness(NodeOrdinal(2)), 1); // attached native
    assert_eq!(snap.node_detachedness(NodeOrdinal(3)), 2); // detached native
}

#[test]
fn test_propagate_dom_state_to_children() {
    let snap = make_detachedness_snapshot();
    // Node 4 (object) is child of attached node 2 (native, det=1)
    // propagate_dom_state should propagate attached state to node 4
    assert_eq!(snap.node_detachedness(NodeOrdinal(4)), 1);
}

#[test]
fn test_detachedness_without_field() {
    // make_test_snapshot has no "detachedness" in node_fields
    let snap = make_test_snapshot();
    // Should return 0 for all nodes when detachedness field is absent
    assert_eq!(snap.node_detachedness(NodeOrdinal(0)), 0);
    assert_eq!(snap.node_detachedness(NodeOrdinal(2)), 0);
}

// ====== 2. get_dominated_children tests ======

#[test]
fn test_get_dominated_children() {
    let snap = make_test_snapshot();
    // GC roots (node 1) is dominator tree root.
    // Node 2 (Object) and node 4 (Array) are direct children of GC roots.
    // Node 0 (synthetic root) is unreachable from GC roots → parented to root.
    let mut children_1: Vec<usize> = snap
        .get_dominated_children(NodeOrdinal(1))
        .iter()
        .map(|o| o.0)
        .collect();
    children_1.sort();
    assert_eq!(children_1, vec![0, 2, 4]);

    // Node 2 (Object) dominates node 3 (hello) — only reachable through Object
    let children_2: Vec<usize> = snap
        .get_dominated_children(NodeOrdinal(2))
        .iter()
        .map(|o| o.0)
        .collect();
    assert_eq!(children_2, vec![3]);

    // Leaf nodes have no dominated children
    assert!(snap.get_dominated_children(NodeOrdinal(3)).is_empty());
    assert!(snap.get_dominated_children(NodeOrdinal(4)).is_empty());
}

// ====== 6. find_edge_target tests ======

#[test]
fn test_find_edge_target() {
    let snap = make_test_snapshot();
    // (GC roots) has property edges "global" -> node 2, "arr" -> node 4
    assert_eq!(
        snap.find_edge_target(NodeOrdinal(1), "global"),
        Some(NodeOrdinal(2))
    );
    assert_eq!(
        snap.find_edge_target(NodeOrdinal(1), "arr"),
        Some(NodeOrdinal(4))
    );
    // Object has property edge "str" -> node 3
    assert_eq!(
        snap.find_edge_target(NodeOrdinal(2), "str"),
        Some(NodeOrdinal(3))
    );
    // Non-existent edge name
    assert_eq!(snap.find_edge_target(NodeOrdinal(2), "nonexistent"), None);
    // Node with no string-named edges (root has only an element edge)
    assert_eq!(snap.find_edge_target(NodeOrdinal(0), "anything"), None);
}

// ====== 7. find_child_by_node_name tests ======

#[test]
fn test_find_child_by_node_name() {
    let snap = make_test_snapshot();
    // Synthetic root's child with node name "(GC roots)" is node 1
    assert_eq!(
        snap.find_child_by_node_name(NodeOrdinal(0), "(GC roots)"),
        Some(NodeOrdinal(1))
    );
    // (GC roots) has children "Object" (node 2) and "Array" (node 4)
    assert_eq!(
        snap.find_child_by_node_name(NodeOrdinal(1), "Object"),
        Some(NodeOrdinal(2))
    );
    assert_eq!(
        snap.find_child_by_node_name(NodeOrdinal(1), "Array"),
        Some(NodeOrdinal(4))
    );
    // Non-existent child name
    assert_eq!(
        snap.find_child_by_node_name(NodeOrdinal(1), "nonexistent"),
        None
    );
    // Leaf node has no children
    assert_eq!(
        snap.find_child_by_node_name(NodeOrdinal(3), "anything"),
        None
    );
}

// ====== NativeContext snapshot builder ======

/// Snapshot with three NativeContext nodes to test native_context_url,
/// native_context_detachedness, native_context_label, is_native_context,
/// and native_contexts.
///
/// ```text
/// Node 0:  synthetic root
/// Node 1:  (GC roots), 3 edges
/// Node 2:  "system / NativeContext / https://example.com" (main context)
///          -> global_object (node 5, Window attached)
///          -> global_proxy_object (node 6, Window proxy, 10 edges -> "main")
/// Node 3:  "system / NativeContext / https://iframe.test" (iframe context)
///          -> global_object (node 7, Window detached)
///          -> global_proxy_object (node 8, Window proxy, 0 edges -> "iframe")
/// Node 4:  "system / NativeContext" (utility context, no URL)
///          -> global_object (node 9, NonWindow -> "utility")
/// Node 5:  native, "Window (global*)", det=1 (attached)
/// Node 6:  object, "Window (global)", det=1, 10 edges (large proxy)
/// Node 7:  native, "Window (global*)", det=2 (detached)
/// Node 8:  object, "Window (global)", det=0, 0 edges (small proxy)
/// Node 9:  object, "NonWindow", det=0
/// Node 10: string, "dummy" (edge target for proxy edges)
/// ```
fn make_native_context_snapshot() -> HeapSnapshot {
    let nfc = 6u32;
    let n = |ord: u32| ord * nfc;

    // Strings table
    //  0: ""
    //  1: "(GC roots)"
    //  2: "system / NativeContext / https://example.com"
    //  3: "system / NativeContext / https://iframe.test"
    //  4: "system / NativeContext"
    //  5: "Window (global*)"
    //  6: "Window (global)"
    //  7: "NonWindow"
    //  8: "dummy"
    //  9: "global_object"
    // 10: "global_proxy_object"
    // 11: "ctx1"
    // 12: "ctx2"
    // 13: "ctx3"
    // 14: "p"

    build_snapshot(
        s(&[
            "type",
            "name",
            "id",
            "self_size",
            "edge_count",
            "detachedness",
        ]),
        vec![
            //   type name id  size edges det
            9, 0, 1, 0, 1, 0, // node 0: synthetic root
            9, 1, 2, 0, 3, 0, // node 1: (GC roots)
            0, 2, 3, 100, 2, 0, // node 2: NativeContext (main)
            0, 3, 5, 100, 2, 0, // node 3: NativeContext (iframe)
            0, 4, 7, 100, 1, 0, // node 4: NativeContext (utility)
            8, 5, 9, 50, 0, 1, // node 5: Window global* (native, attached)
            3, 6, 11, 50, 10, 1, // node 6: Window global proxy (10 edges)
            8, 5, 13, 50, 0, 2, // node 7: Window global* (native, detached)
            3, 6, 15, 50, 0, 0, // node 8: Window global proxy (0 edges)
            3, 7, 17, 50, 0, 0, // node 9: NonWindow
            2, 8, 19, 1, 0, 0, // node 10: dummy string
        ],
        vec![
            1,
            0,
            n(1), // edge 0: root -> GC roots (element)
            2,
            11,
            n(2), // edge 1: GC roots -> node 2 (property "ctx1")
            2,
            12,
            n(3), // edge 2: GC roots -> node 3 (property "ctx2")
            2,
            13,
            n(4), // edge 3: GC roots -> node 4 (property "ctx3")
            3,
            9,
            n(5), // edge 4: node 2 -> node 5 (internal "global_object")
            3,
            10,
            n(6), // edge 5: node 2 -> node 6 (internal "global_proxy_object")
            3,
            9,
            n(7), // edge 6: node 3 -> node 7 (internal "global_object")
            3,
            10,
            n(8), // edge 7: node 3 -> node 8 (internal "global_proxy_object")
            3,
            9,
            n(9), // edge 8: node 4 -> node 9 (internal "global_object")
            // 10 property edges from node 6 (proxy) to node 10 (dummy)
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
            2,
            14,
            n(10),
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://example.com",
            "system / NativeContext / https://iframe.test",
            "system / NativeContext",
            "Window (global*)",
            "Window (global)",
            "NonWindow",
            "dummy",
            "global_object",
            "global_proxy_object",
            "ctx1",
            "ctx2",
            "ctx3",
            "p",
        ]),
    )
}

// ====== 9. native_context_url tests ======

#[test]
fn test_native_context_url() {
    let snap = make_native_context_snapshot();
    assert_eq!(
        snap.native_context_url(NodeOrdinal(2)),
        Some("https://example.com")
    );
    assert_eq!(
        snap.native_context_url(NodeOrdinal(3)),
        Some("https://iframe.test")
    );
    // "system / NativeContext" with no URL suffix
    assert_eq!(snap.native_context_url(NodeOrdinal(4)), None);
    // Non-NativeContext node
    assert_eq!(snap.native_context_url(NodeOrdinal(0)), None);
}

// ====== 10. native_context_detachedness tests ======

#[test]
fn test_native_context_detachedness() {
    let snap = make_native_context_snapshot();
    // Main context: global_object (node 5) is attached (det=1)
    assert_eq!(snap.native_context_detachedness(NodeOrdinal(2)), 1);
    // Iframe context: global_object (node 7) is detached (det=2)
    assert_eq!(snap.native_context_detachedness(NodeOrdinal(3)), 2);
    // Utility context: global_object (node 9) has det=0, no proxy → returns 0
    assert_eq!(snap.native_context_detachedness(NodeOrdinal(4)), 0);
}

// ====== 11. native_context_label tests ======

#[test]
fn test_native_context_label() {
    let snap = make_native_context_snapshot();
    // Main context: Window global + proxy with ≥10 edges → "main"
    assert_eq!(
        snap.native_context_label(NodeOrdinal(2)),
        "[main] https://example.com @3"
    );
    // Iframe context: Window global + proxy with <10 edges → "iframe"
    assert_eq!(
        snap.native_context_label(NodeOrdinal(3)),
        "[iframe] https://iframe.test @5"
    );
    // Utility context: non-Window global → "utility", no URL
    assert_eq!(snap.native_context_label(NodeOrdinal(4)), "[utility] @7");
}

// ====== 12. is_native_context / native_contexts tests ======

#[test]
fn test_is_native_context() {
    let snap = make_native_context_snapshot();
    assert!(!snap.is_native_context(NodeOrdinal(0))); // synthetic root
    assert!(!snap.is_native_context(NodeOrdinal(1))); // GC roots
    assert!(snap.is_native_context(NodeOrdinal(2))); // system / NativeContext / ...
    assert!(snap.is_native_context(NodeOrdinal(3))); // system / NativeContext / ...
    assert!(snap.is_native_context(NodeOrdinal(4))); // system / NativeContext
    assert!(!snap.is_native_context(NodeOrdinal(5))); // Window
}

#[test]
fn test_native_contexts_list() {
    let snap = make_native_context_snapshot();
    let mut contexts = snap.native_contexts().to_vec();
    contexts.sort();
    assert_eq!(contexts, vec![2, 3, 4]);
}

// ====== native_context_vars tests ======

/// Snapshot with two NativeContexts to test native_context_vars.
///
/// ```text
/// Node  0: synthetic root
/// Node  1: (GC roots), 2 edges -> ctx_a, ctx_b
/// Node  2: "system / NativeContext / https://a.test" (context A)
///          -> global_object (node 4), script_context_table (node 6)
/// Node  3: "system / NativeContext / https://b.test" (context B)
///          -> global_object (node 5), script_context_table (node 9)
/// Node  4: "Window (global*)" — global object for A
///          property edges: "Array" (common), "myAppVar" (unique to A)
/// Node  5: "Window (global*)" — global object for B
///          property edges: "Array" (common), "bSpecial" (unique to B)
/// Node  6: "system / ScriptContextTable" for A
///          hidden edge -> node 7 (ScriptContext)
/// Node  7: "system / Context" — script context for A
///          context edges: "myLet", "myConst"
/// Node  8: dummy target for property edges
/// Node  9: "system / ScriptContextTable" for B (empty — no script vars)
/// Node 10: dummy target for context edges
/// ```
fn make_vars_snapshot() -> HeapSnapshot {
    let nfc = 6u32;
    let n = |ord: u32| ord * nfc;

    // String indices:
    //  0: ""
    //  1: "(GC roots)"
    //  2: "system / NativeContext / https://a.test"
    //  3: "system / NativeContext / https://b.test"
    //  4: "Window (global*)"
    //  5: "system / ScriptContextTable"
    //  6: "system / Context"
    //  7: "dummy"
    //  8: "global_object"
    //  9: "script_context_table"
    // 10: "ctx_a"
    // 11: "ctx_b"
    // 12: "Array"
    // 13: "myAppVar"
    // 14: "bSpecial"
    // 15: "myLet"
    // 16: "myConst"

    build_snapshot(
        s(&[
            "type",
            "name",
            "id",
            "self_size",
            "edge_count",
            "detachedness",
        ]),
        vec![
            //   type name id  size edges det
            9, 0, 1, 0, 1, 0, // node  0: synthetic root
            9, 1, 2, 0, 2, 0, // node  1: (GC roots)
            0, 2, 3, 100, 2, 0, // node  2: NativeContext A
            0, 3, 5, 100, 2, 0, // node  3: NativeContext B
            8, 4, 7, 50, 2, 1, // node  4: Window (global*) for A
            8, 4, 9, 50, 2, 1, // node  5: Window (global*) for B
            3, 5, 11, 10, 1, 0, // node  6: ScriptContextTable A
            3, 6, 13, 10, 2, 0, // node  7: Context (script ctx A)
            2, 7, 15, 1, 0, 0, // node  8: dummy
            3, 5, 17, 10, 0, 0, // node  9: ScriptContextTable B (empty)
            2, 7, 19, 1, 0, 0, // node 10: dummy for context edges
        ],
        vec![
            // root -> GC roots
            1,
            0,
            n(1),
            // GC roots -> ctx A, ctx B
            2,
            10,
            n(2),
            2,
            11,
            n(3),
            // NativeContext A -> global_object, script_context_table
            3,
            8,
            n(4),
            3,
            9,
            n(6),
            // NativeContext B -> global_object, script_context_table
            3,
            8,
            n(5),
            3,
            9,
            n(9),
            // Window A: Array (common), myAppVar (unique)
            2,
            12,
            n(8),
            2,
            13,
            n(8),
            // Window B: Array (common), bSpecial (unique)
            2,
            12,
            n(8),
            2,
            14,
            n(8),
            // ScriptContextTable A -> Context (hidden edge)
            4,
            0,
            n(7),
            // Context A: myLet, myConst (context-type edges)
            0,
            15,
            n(10),
            0,
            16,
            n(10),
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "Window (global*)",
            "system / ScriptContextTable",
            "system / Context",
            "dummy",
            "global_object",
            "script_context_table",
            "ctx_a",
            "ctx_b",
            "Array",
            "myAppVar",
            "bSpecial",
            "myLet",
            "myConst",
        ]),
    )
}

#[test]
fn test_native_context_vars_with_global_and_script_vars() {
    let snap = make_vars_snapshot();

    // Context A has unique global var "myAppVar" + script context vars "myConst", "myLet"
    let vars_a = snap.native_context_vars(NodeOrdinal(2));
    assert!(
        vars_a.contains("myAppVar"),
        "expected myAppVar in vars: {vars_a}"
    );
    assert!(vars_a.contains("myLet"), "expected myLet in vars: {vars_a}");
    assert!(
        vars_a.contains("myConst"),
        "expected myConst in vars: {vars_a}"
    );
    // "Array" is common to both globals, should NOT appear
    assert!(
        !vars_a.contains("Array"),
        "Array should be common, not in vars: {vars_a}"
    );
}

#[test]
fn test_native_context_vars_no_script_vars() {
    let snap = make_vars_snapshot();

    // Context B has unique global var "bSpecial" but no script context vars
    let vars_b = snap.native_context_vars(NodeOrdinal(3));
    assert!(
        vars_b.contains("bSpecial"),
        "expected bSpecial in vars: {vars_b}"
    );
    // Should not have context A's script vars
    assert!(
        !vars_b.contains("myLet"),
        "myLet belongs to context A, not B: {vars_b}"
    );
}

#[test]
fn test_native_context_vars_sorted() {
    let snap = make_vars_snapshot();

    // Context A vars should be sorted
    let vars_a = snap.native_context_vars(NodeOrdinal(2));
    let parts: Vec<&str> = vars_a.split(", ").collect();
    let mut sorted = parts.clone();
    sorted.sort();
    assert_eq!(parts, sorted, "vars should be sorted: {vars_a}");
}

#[test]
fn test_native_context_vars_empty_for_non_context() {
    let snap = make_vars_snapshot();

    // Non-NativeContext node should return empty
    assert_eq!(snap.native_context_vars(NodeOrdinal(0)), "");
    assert_eq!(snap.native_context_vars(NodeOrdinal(1)), "");
    assert_eq!(snap.native_context_vars(NodeOrdinal(4)), "");
}

// ====== 18. format_property_name_display / json_escape_string tests ======

#[test]
fn test_format_property_name_display_plain() {
    // Names without special characters pass through unchanged
    assert_eq!(HeapSnapshot::format_property_name_display("foo"), "foo");
    assert_eq!(
        HeapSnapshot::format_property_name_display("bar_baz"),
        "bar_baz"
    );
}

#[test]
fn test_format_property_name_display_escapes_special_chars() {
    // Comma triggers JSON escaping
    assert_eq!(HeapSnapshot::format_property_name_display("a,b"), "\"a,b\"");
    // Single quote
    assert_eq!(
        HeapSnapshot::format_property_name_display("it's"),
        "\"it's\""
    );
    // Double quote gets escaped inside the JSON string
    assert_eq!(
        HeapSnapshot::format_property_name_display("say\"hi"),
        "\"say\\\"hi\""
    );
    // Braces
    assert_eq!(
        HeapSnapshot::format_property_name_display("{key}"),
        "\"{key}\""
    );
}

#[test]
fn test_json_escape_string() {
    assert_eq!(HeapSnapshot::json_escape_string("hello"), "\"hello\"");
    // Double quote
    assert_eq!(HeapSnapshot::json_escape_string("a\"b"), "\"a\\\"b\"");
    // Backslash
    assert_eq!(HeapSnapshot::json_escape_string("a\\b"), "\"a\\\\b\"");
    // Newline, carriage return, tab
    assert_eq!(HeapSnapshot::json_escape_string("a\nb"), "\"a\\nb\"");
    assert_eq!(HeapSnapshot::json_escape_string("a\rb"), "\"a\\rb\"");
    assert_eq!(HeapSnapshot::json_escape_string("a\tb"), "\"a\\tb\"");
    // Control character (e.g. 0x01)
    assert_eq!(HeapSnapshot::json_escape_string("a\x01b"), "\"a\\u0001b\"");
}

// ====== 19. Location map / ClassKey::Location tests ======

/// Snapshot with two objects at different source locations but the same class
/// name, to verify that aggregates separate them by location.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges
/// Node 2: object, "MyClass", size=100  (location: script 1, line 10, col 5)
/// Node 3: object, "MyClass", size=200  (location: script 1, line 20, col 3)
/// ```
fn make_location_snapshot() -> HeapSnapshot {
    let nfc = 5;
    let efc = 3;
    let n = |ord: u32| ord * nfc as u32;

    let node_fields = standard_node_fields();
    let strings = s(&["", "(GC roots)", "MyClass", "a", "b"]);

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 2, // node 1: (GC roots)
        3, 2, 3, 100, 0, // node 2: MyClass (object)
        3, 2, 5, 200, 0, // node 3: MyClass (object)
    ];
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root -> GC roots (element)
        2,
        3,
        n(2), // GC roots -> node 2 (property "a")
        2,
        4,
        n(3), // GC roots -> node 3 (property "b")
    ];

    // Location fields: object_index, script_id, line, column
    let location_fields = s(&["object_index", "script_id", "line", "column"]);
    // Locations: node 2 at (script 1, line 10, col 5), node 3 at (script 1, line 20, col 3)
    let locations: Vec<u32> = vec![
        n(2),
        1,
        10,
        5, // node 2
        n(3),
        1,
        20,
        3, // node 3
    ];

    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields,
                node_type_enum: standard_node_type_enum(),
                edge_fields: standard_edge_fields(),
                edge_type_enum: standard_edge_type_enum(),
                location_fields,
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
        locations,
    };
    HeapSnapshot::new(raw)
}

#[test]
fn test_aggregates_split_by_location() {
    let snap = make_location_snapshot();
    let aggs = snap.aggregates_with_filter();

    // Two objects with the same class but different locations → separate entries
    let key_1 = "1,10,5,MyClass";
    let key_2 = "1,20,3,MyClass";

    assert!(
        aggs.contains_key(key_1),
        "missing aggregate for {key_1}, keys: {:?}",
        aggs.keys().collect::<Vec<_>>()
    );
    assert!(
        aggs.contains_key(key_2),
        "missing aggregate for {key_2}, keys: {:?}",
        aggs.keys().collect::<Vec<_>>()
    );

    assert_eq!(aggs[key_1].count, 1);
    assert_eq!(aggs[key_1].self_size, 100.0);
    assert_eq!(aggs[key_2].count, 1);
    assert_eq!(aggs[key_2].self_size, 200.0);
}

#[test]
fn test_aggregates_no_location_uses_class_index() {
    // make_test_snapshot has no locations → objects use ClassKey::Index
    let snap = make_test_snapshot();
    let aggs = snap.aggregates_with_filter();

    // "Object" is keyed by class name string, not a location key
    assert!(aggs.contains_key("Object"));
    assert_eq!(aggs["Object"].count, 1);
}

// ====== WeakMap ephemeron tests ======

/// Snapshot with a WeakMap ephemeron pattern: both a key node and a table
/// node have internal edges to the same value node.  The code should mark
/// the key→value edge non-essential so the table dominates the value.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges
/// Node 2: object, "KeyObj",  id=100, size=50,  1 edge (ephemeron → value)
/// Node 3: object, "WeakMap", id=300, size=30,  1 edge (ephemeron → value)
/// Node 4: object, "ValObj",  id=200, size=500, 0 edges
///
/// Edges:
///   root  --element[0]-->  (GC roots)
///   (GC roots) --"key"-->   KeyObj
///   (GC roots) --"table"--> WeakMap
///   KeyObj  --internal(ephemeron)--> ValObj   (non-essential: node_id!=table_id)
///   WeakMap --internal(ephemeron)--> ValObj   (essential:     node_id==table_id)
/// ```
fn make_ephemeron_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 100, 50, 1, // node 2: KeyObj, id=100
            3, 3, 300, 30, 1, // node 3: WeakMap, id=300
            3, 4, 200, 500, 0, // node 4: ValObj, id=200
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            5,
            n(2), // GC roots → KeyObj (property "key")
            2,
            6,
            n(3), // GC roots → WeakMap (property "table")
            3,
            7,
            n(4), // KeyObj → ValObj (internal, ephemeron name — key side)
            3,
            8,
            n(4), // WeakMap → ValObj (internal, ephemeron name — table side)
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "KeyObj",     // 2
            "WeakMap",    // 3
            "ValObj",     // 4
            "key",        // 5
            "table",      // 6
            // key→value ephemeron edge (node_id 100 != table_id 300 → non-essential)
            "456 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
            // table→value ephemeron edge (node_id 300 == table_id 300 → essential)
            "789 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
        ]),
    )
}

#[test]
fn test_ephemeron_table_dominates_value() {
    let snap = make_ephemeron_snapshot();
    // Key→value edge is non-essential, table→value is essential.
    // Therefore the table (WeakMap) dominates the value, not the key.
    // table retained = self(30) + value(500) = 530
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 530.0);
    // key retained = self(50) only — value is NOT dominated by key
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 50.0);
}

#[test]
fn test_ephemeron_value_dominated_by_table() {
    let snap = make_ephemeron_snapshot();
    // Value (node 4) should appear in table's dominated children, not key's
    let table_children: Vec<usize> = snap
        .get_dominated_children(NodeOrdinal(3))
        .iter()
        .map(|o| o.0)
        .collect();
    assert!(
        table_children.contains(&4),
        "value should be dominated by table, got: {:?}",
        table_children
    );

    let key_children: Vec<usize> = snap
        .get_dominated_children(NodeOrdinal(2))
        .iter()
        .map(|o| o.0)
        .collect();
    assert!(
        !key_children.contains(&4),
        "value should NOT be dominated by key, got: {:?}",
        key_children
    );
}

#[test]
fn test_ephemeron_value_has_valid_distance() {
    let snap = make_ephemeron_snapshot();
    // Value should still be reachable with a valid distance
    // GC roots (0) → key/table (1) → value (2)
    assert_eq!(snap.node_distance(NodeOrdinal(4)), 2);
}

/// Snapshot where the key and table are at very different BFS depths.
/// Key is at distance 1, table is at distance 5 (behind a chain of 4 hops).
/// BFS encounters the key's ephemeron edge first (skipped by dedup), then
/// the table's (allowed).  Value gets distance 6, not 2.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges
/// Node 2: object, "KeyObj",  id=100, distance=1, 1 edge (ephemeron → value)
/// Node 3: object, "Mid1",   id=401, distance=1, 1 edge (→ Mid2)
/// Node 4: object, "Mid2",   id=402, distance=2, 1 edge (→ Mid3)
/// Node 5: object, "Mid3",   id=403, distance=3, 1 edge (→ Mid4)
/// Node 6: object, "Mid4",   id=404, distance=4, 1 edge (→ WeakMap)
/// Node 7: object, "WeakMap", id=300, distance=5, 1 edge (ephemeron → value)
/// Node 8: object, "ValObj",  id=200, distance=6, 0 edges
/// ```
fn make_ephemeron_depth_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 100, 50, 1, // node 2: KeyObj, id=100
            3, 3, 401, 10, 1, // node 3: Mid1
            3, 4, 402, 10, 1, // node 4: Mid2
            3, 5, 403, 10, 1, // node 5: Mid3
            3, 6, 404, 10, 1, // node 6: Mid4
            3, 7, 300, 30, 1, // node 7: WeakMap, id=300
            3, 8, 200, 500, 0, // node 8: ValObj, id=200
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            9,
            n(2), // GC roots → KeyObj (property "key")
            2,
            10,
            n(3), // GC roots → Mid1 (property "mid1")
            3,
            15,
            n(8), // KeyObj → ValObj (internal, ephemeron — skipped first)
            2,
            11,
            n(4), // Mid1 → Mid2 (property "mid2")
            2,
            12,
            n(5), // Mid2 → Mid3 (property "mid3")
            2,
            13,
            n(6), // Mid3 → Mid4 (property "mid4")
            2,
            14,
            n(7), // Mid4 → WeakMap (property "tbl")
            3,
            16,
            n(8), // WeakMap → ValObj (internal, ephemeron — allowed second)
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "KeyObj",     // 2
            "Mid1",       // 3
            "Mid2",       // 4
            "Mid3",       // 5
            "Mid4",       // 6
            "WeakMap",    // 7
            "ValObj",     // 8
            "key",        // 9
            "mid1",       // 10
            "mid2",       // 11
            "mid3",       // 12
            "mid4",       // 13
            "tbl",        // 14
            // key→value ephemeron (encountered first during BFS → skipped)
            "456 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
            // table→value ephemeron (encountered second → allowed)
            "789 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
        ]),
    )
}

#[test]
fn test_ephemeron_distance_dedup_skips_first() {
    let snap = make_ephemeron_depth_snapshot();
    // Key at distance 1, table at distance 5 (behind 4 intermediate hops).
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1); // KeyObj
    assert_eq!(snap.node_distance(NodeOrdinal(7)), 5); // WeakMap

    // Value gets distance 6 (from table at depth 5), not 2 (from key at depth 1),
    // because the first ephemeron edge (from key) is skipped by dedup.
    assert_eq!(snap.node_distance(NodeOrdinal(8)), 6);
}

/// Mirror of make_ephemeron_depth_snapshot: table is close (distance 1),
/// key is far (distance 5).  BFS encounters the table's ephemeron edge
/// first (skipped), then the key's (allowed).  Value gets distance 6.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges
/// Node 2: object, "WeakMap", id=300, distance=1, 1 edge (ephemeron → value)
/// Node 3: object, "Mid1",   id=401, distance=1, 1 edge (→ Mid2)
/// Node 4: object, "Mid2",   id=402, distance=2, 1 edge (→ Mid3)
/// Node 5: object, "Mid3",   id=403, distance=3, 1 edge (→ Mid4)
/// Node 6: object, "Mid4",   id=404, distance=4, 1 edge (→ KeyObj)
/// Node 7: object, "KeyObj", id=100, distance=5, 1 edge (ephemeron → value)
/// Node 8: object, "ValObj", id=200, distance=6, 0 edges
/// ```
fn make_ephemeron_depth_reversed_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 300, 30, 1, // node 2: WeakMap, id=300
            3, 3, 401, 10, 1, // node 3: Mid1
            3, 4, 402, 10, 1, // node 4: Mid2
            3, 5, 403, 10, 1, // node 5: Mid3
            3, 6, 404, 10, 1, // node 6: Mid4
            3, 7, 100, 50, 1, // node 7: KeyObj, id=100
            3, 8, 200, 500, 0, // node 8: ValObj, id=200
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            9,
            n(2), // GC roots → WeakMap (property "tbl")
            2,
            10,
            n(3), // GC roots → Mid1 (property "mid1")
            3,
            16,
            n(8), // WeakMap → ValObj (internal, ephemeron — skipped first)
            2,
            11,
            n(4), // Mid1 → Mid2 (property "mid2")
            2,
            12,
            n(5), // Mid2 → Mid3 (property "mid3")
            2,
            13,
            n(6), // Mid3 → Mid4 (property "mid4")
            2,
            14,
            n(7), // Mid4 → KeyObj (property "key")
            3,
            15,
            n(8), // KeyObj → ValObj (internal, ephemeron — allowed second)
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "WeakMap",    // 2
            "Mid1",       // 3
            "Mid2",       // 4
            "Mid3",       // 5
            "Mid4",       // 6
            "KeyObj",     // 7
            "ValObj",     // 8
            "tbl",        // 9
            "mid1",       // 10
            "mid2",       // 11
            "mid3",       // 12
            "mid4",       // 13
            "key",        // 14
            // key→value ephemeron (encountered second → allowed)
            "456 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
            // table→value ephemeron (encountered first during BFS → skipped)
            "789 / part of key (KeyObj @100) -> value (ValObj @200) pair in WeakMap (table @300)",
        ]),
    )
}

#[test]
fn test_ephemeron_distance_dedup_skips_first_reversed() {
    let snap = make_ephemeron_depth_reversed_snapshot();
    // Table at distance 1, key at distance 5.
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1); // WeakMap
    assert_eq!(snap.node_distance(NodeOrdinal(7)), 5); // KeyObj

    // Value gets distance 6 (from key at depth 5), not 2 (from table at depth 1),
    // because the first ephemeron edge (from table) is skipped by dedup.
    assert_eq!(snap.node_distance(NodeOrdinal(8)), 6);
}

// ── sloppy_function_map filtering ──────────────────────────────────────

/// NativeContext node with a "sloppy_function_map" edge to Target.
/// Target is also reachable via a longer path (Root → Mid → Target).
/// The sloppy_function_map edge should be skipped, so Target gets
/// distance 3 (via Mid) not 2 (via NativeContext).
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges → NativeContext, Mid
/// Node 2: object, "system / NativeContext", distance=1, 2 edges
///         - property "sloppy_function_map" → Target (filtered!)
///         - property "array_function" → Keeper (not filtered)
/// Node 3: object, "Mid", distance=1, 1 edge → Target
/// Node 4: object, "Target", distance=3 (via Mid, not via NativeContext)
/// Node 5: object, "Keeper", distance=2 (via NativeContext, not filtered)
/// ```
fn make_sloppy_function_map_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 10, 20, 2, // node 2: NativeContext, id=10
            3, 3, 11, 30, 1, // node 3: Mid, id=11
            3, 4, 12, 40, 0, // node 4: Target, id=12
            3, 5, 13, 50, 0, // node 5: Keeper, id=13
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            6,
            n(2), // GC roots → NativeContext (property "ctx")
            2,
            7,
            n(3), // GC roots → Mid (property "mid")
            2,
            8,
            n(4), // NativeContext → Target (property "sloppy_function_map" — filtered)
            2,
            9,
            n(5), // NativeContext → Keeper (property "array_function" — not filtered)
            2,
            10,
            n(4), // Mid → Target (property "tgt")
        ],
        s(&[
            "",                       // 0
            "(GC roots)",             // 1
            "system / NativeContext", // 2
            "Mid",                    // 3
            "Target",                 // 4
            "Keeper",                 // 5
            "ctx",                    // 6
            "mid",                    // 7
            "sloppy_function_map",    // 8
            "array_function",         // 9
            "tgt",                    // 10
        ]),
    )
}

#[test]
fn test_sloppy_function_map_edge_filtered_in_distances() {
    let snap = make_sloppy_function_map_snapshot();
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1); // NativeContext
    assert_eq!(snap.node_distance(NodeOrdinal(3)), 1); // Mid

    // Target reachable via Mid → Target (distance 2), NOT via
    // NativeContext → sloppy_function_map (which is filtered out).
    assert_eq!(snap.node_distance(NodeOrdinal(4)), 2);

    // Keeper is reachable via NativeContext → array_function (not filtered).
    assert_eq!(snap.node_distance(NodeOrdinal(5)), 2);
}

// ── (map descriptors) array filtering ──────────────────────────────────

/// Array node named "(map descriptors)" with element edges at various indices.
/// Indices where index >= 2 && index % 3 == 1 are filtered (4, 7).
/// Indices 0, 1, 2, 5 are allowed.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 1 edge → Descriptors
/// Node 2: array, "(map descriptors)", distance=1, 5 element edges
///         - element[0] → Child0 (allowed: 0 < 2)
///         - element[1] → Child1 (allowed: 1 < 2)
///         - element[4] → Child4 (FILTERED: 4 >= 2 && 4 % 3 == 1)
///         - element[5] → Child5 (allowed: 5 >= 2, 5 % 3 == 2 ≠ 1)
///         - element[7] → Child7 (FILTERED: 7 >= 2 && 7 % 3 == 1)
/// Node 3: object, "Child0", distance=2
/// Node 4: object, "Child1", distance=2
/// Node 5: object, "Child4", distance=? (needs alt path)
/// Node 6: object, "Child5", distance=2
/// Node 7: object, "Child7", distance=? (needs alt path)
/// Node 8: object, "Alt", distance=1, 2 edges → Child4, Child7
/// ```
fn make_map_descriptors_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            1, 2, 10, 20, 5, // node 2: array "(map descriptors)", id=10
            3, 3, 11, 30, 0, // node 3: Child0, id=11
            3, 4, 12, 30, 0, // node 4: Child1, id=12
            3, 5, 13, 30, 0, // node 5: Child4, id=13
            3, 6, 14, 30, 0, // node 6: Child5, id=14
            3, 7, 15, 30, 0, // node 7: Child7, id=15
            3, 8, 16, 10, 2, // node 8: Alt, id=16
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            9,
            n(2), // GC roots → (map descriptors) (property "desc")
            2,
            10,
            n(8), // GC roots → Alt (property "alt")
            1,
            0,
            n(3), // descriptors → Child0 (element[0] — allowed)
            1,
            1,
            n(4), // descriptors → Child1 (element[1] — allowed)
            1,
            4,
            n(5), // descriptors → Child4 (element[4] — FILTERED)
            1,
            5,
            n(6), // descriptors → Child5 (element[5] — allowed)
            1,
            7,
            n(7), // descriptors → Child7 (element[7] — FILTERED)
            2,
            11,
            n(5), // Alt → Child4 (property "c4")
            2,
            12,
            n(7), // Alt → Child7 (property "c7")
        ],
        s(&[
            "",                  // 0
            "(GC roots)",        // 1
            "(map descriptors)", // 2
            "Child0",            // 3
            "Child1",            // 4
            "Child4",            // 5
            "Child5",            // 6
            "Child7",            // 7
            "Alt",               // 8
            "desc",              // 9
            "alt",               // 10
            "c4",                // 11
            "c7",                // 12
        ]),
    )
}

#[test]
fn test_map_descriptors_element_filtering() {
    let snap = make_map_descriptors_snapshot();
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1); // (map descriptors)
    assert_eq!(snap.node_distance(NodeOrdinal(8)), 1); // Alt

    // Allowed element edges: index 0, 1, 5 → distance 2
    assert_eq!(snap.node_distance(NodeOrdinal(3)), 2); // Child0 via element[0]
    assert_eq!(snap.node_distance(NodeOrdinal(4)), 2); // Child1 via element[1]
    assert_eq!(snap.node_distance(NodeOrdinal(6)), 2); // Child5 via element[5]

    // Filtered element edges: index 4, 7 (>= 2 && % 3 == 1)
    // These children are only reachable via Alt (distance 1) → property edge
    assert_eq!(snap.node_distance(NodeOrdinal(5)), 2); // Child4 via Alt
    assert_eq!(snap.node_distance(NodeOrdinal(7)), 2); // Child7 via Alt
}

#[test]
fn test_map_descriptors_property_edges_not_filtered() {
    // Property edges on (map descriptors) should NOT be filtered,
    // even if the node name matches. Only element/hidden edges are checked.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            1, 2, 10, 20, 1, // node 2: array "(map descriptors)", id=10
            3, 3, 11, 30, 0, // node 3: Child, id=11
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → descriptors (property "desc")
            2,
            5,
            n(3), // descriptors → Child (property "prop4" — NOT filtered even though it could be index 4)
        ],
        s(&[
            "",                  // 0
            "(GC roots)",        // 1
            "(map descriptors)", // 2
            "Child",             // 3
            "desc",              // 4
            "prop4",             // 5
        ]),
    );
    // Property edge from (map descriptors) is never filtered
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1);
    assert_eq!(snap.node_distance(NodeOrdinal(3)), 2);
}

// ── retained size: duplicate fields ────────────────────────────────────

/// Wrapper holds Inner via two separate property edges (field1 and field2).
/// Inner is only reachable through Wrapper, so Wrapper dominates Inner
/// and Wrapper's retained size includes Inner's self_size.
///
/// ```text
/// Node 0: synthetic root, self_size=0
/// Node 1: (GC roots), self_size=0, 1 edge → Wrapper
/// Node 2: object "Wrapper", self_size=100, 2 edges → Inner (field1, field2)
/// Node 3: object "Inner", self_size=200, 0 edges
/// ```
#[test]
fn test_retained_size_two_fields_same_target() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 10, 100, 2, // node 2: Wrapper, self_size=100
            3, 3, 11, 200, 0, // node 3: Inner, self_size=200
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → Wrapper (property "w")
            2,
            5,
            n(3), // Wrapper → Inner (property "field1")
            2,
            6,
            n(3), // Wrapper → Inner (property "field2")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Wrapper",    // 2
            "Inner",      // 3
            "w",          // 4
            "field1",     // 5
            "field2",     // 6
        ]),
    );
    // Wrapper dominates Inner (only path to Inner goes through Wrapper)
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 300.0); // 100 + 200
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 200.0); // just self
}

/// Two separate GC sub-roots (Root1 and Root2) both point to the same Target.
/// Neither root dominates Target — their common ancestor (GC roots) does.
/// So neither Root1 nor Root2 includes Target's retained size.
///
/// ```text
/// Node 0: synthetic root, self_size=0
/// Node 1: (GC roots), self_size=0, 2 edges → Root1, Root2
/// Node 2: object "Root1", self_size=50, 1 edge → Target
/// Node 3: object "Root2", self_size=60, 1 edge → Target
/// Node 4: object "Target", self_size=400, 0 edges
/// ```
#[test]
fn test_retained_size_two_roots_same_target() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 10, 50, 1, // node 2: Root1, self_size=50
            3, 3, 11, 60, 1, // node 3: Root2, self_size=60
            3, 4, 12, 400, 0, // node 4: Target, self_size=400
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            5,
            n(2), // GC roots → Root1 (property "r1")
            2,
            6,
            n(3), // GC roots → Root2 (property "r2")
            2,
            7,
            n(4), // Root1 → Target (property "ref")
            2,
            8,
            n(4), // Root2 → Target (property "ref")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Root1",      // 2
            "Root2",      // 3
            "Target",     // 4
            "r1",         // 5
            "r2",         // 6
            "ref",        // 7
            "ref",        // 8
        ]),
    );
    // Neither Root1 nor Root2 dominates Target — (GC roots) does.
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 50.0); // Root1: just self
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 60.0); // Root2: just self
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 400.0); // Target: just self
    // (GC roots) retains everything
    assert_eq!(snap.node_retained_size(NodeOrdinal(1)), 510.0); // 0 + 50 + 60 + 400
}

/// Single GC sub-root with two property edges to the same Target.
/// Root dominates Target (only path), so Root's retained size includes Target.
///
/// ```text
/// Node 0: synthetic root, self_size=0
/// Node 1: (GC roots), self_size=0, 1 edge → Root
/// Node 2: object "Root", self_size=80, 2 edges → Target (field1, field2)
/// Node 3: object "Target", self_size=300, 0 edges
/// ```
#[test]
fn test_retained_size_single_root_two_edges_same_target() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 10, 80, 2, // node 2: Root, self_size=80
            3, 3, 11, 300, 0, // node 3: Target, self_size=300
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → Root (property "r")
            2,
            5,
            n(3), // Root → Target (property "field1")
            2,
            6,
            n(3), // Root → Target (property "field2")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Root",       // 2
            "Target",     // 3
            "r",          // 4
            "field1",     // 5
            "field2",     // 6
        ]),
    );
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 380.0); // 80 + 300
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 300.0); // just self
}

// ── aggregates: multiple same-class objects ────────────────────────────

/// Three "Foo" objects at different distances with different sizes.
/// Aggregates should: sum self_size, count=3, pick min distance,
/// and accumulate max_ret correctly.
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots), 2 edges → Foo1, Mid
/// Node 2: object "Foo", self_size=100, distance=1, 0 edges
/// Node 3: object "Mid", self_size=10, distance=1, 2 edges → Foo2, Deep
/// Node 4: object "Foo", self_size=200, distance=2, 0 edges
/// Node 5: object "Deep", self_size=10, distance=2, 1 edge → Foo3
/// Node 6: object "Foo", self_size=300, distance=3, 0 edges
/// ```
///
/// Dominator tree:
///   (GC roots) → Foo1, Mid
///   Mid → Foo2, Deep
///   Deep → Foo3
///
/// Retained sizes:
///   Foo1=100, Foo2=200, Foo3=300, Deep=310, Mid=520
///
/// max_ret for "Foo": The dominator-tree DFS visits Foo1 (ret 100),
/// then enters Mid subtree where it visits Foo2 (ret 200) — but "Foo"
/// is already "seen" so Foo2 is skipped. Then Foo3 (also skipped, still
/// under Mid's subtree). After leaving Mid's subtree, "Foo" becomes
/// unseen again. So max_ret = 100 (only Foo1 counted).
///
/// Wait — let me re-read the algorithm. The "seen" flag prevents
/// double-counting when a class appears as a descendant of itself in the
/// dominator tree. Here Foo1 is a direct child of GC roots (not under
/// any other Foo), so it's counted. Then Mid is visited (class "Mid",
/// not Foo). Under Mid: Foo2 is visited — "Foo" not seen yet at this
/// level → counted (max_ret += 200). Then Deep under Mid: Foo3 — "Foo"
/// is now seen (from Foo2) → skipped. So max_ret = 100 + 200 = 300.
#[test]
fn test_aggregates_multiple_same_class() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 10, 100, 0, // node 2: Foo, self_size=100
            3, 3, 11, 10, 2, // node 3: Mid, self_size=10
            3, 2, 12, 200, 0, // node 4: Foo (name_idx=2), self_size=200
            3, 4, 13, 10, 1, // node 5: Deep, self_size=10
            3, 2, 14, 300, 0, // node 6: Foo (name_idx=2), self_size=300
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            5,
            n(2), // GC roots → Foo1 (property "f1")
            2,
            6,
            n(3), // GC roots → Mid (property "mid")
            2,
            7,
            n(4), // Mid → Foo2 (property "f2")
            2,
            8,
            n(5), // Mid → Deep (property "deep")
            2,
            9,
            n(6), // Deep → Foo3 (property "f3")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Foo",        // 2
            "Mid",        // 3
            "Deep",       // 4
            "f1",         // 5
            "mid",        // 6
            "f2",         // 7
            "deep",       // 8
            "f3",         // 9
        ]),
    );

    let aggs = snap.aggregates_with_filter();
    let foo = &aggs["Foo"];

    // count: 3 Foo objects
    assert_eq!(foo.count, 3);

    // self_size: 100 + 200 + 300
    assert_eq!(foo.self_size, 600.0);

    // distance: min of 1, 2, 3
    assert_eq!(foo.distance, 1);

    // node_ordinals: all three
    assert_eq!(foo.node_ordinals.len(), 3);
}

/// Two Foo objects where Foo1 dominates Foo2, plus a sibling Foo3.
/// max_ret should count Foo1's retained size (which already includes Foo2)
/// and Foo3's retained size, but NOT Foo2's separately (that would double-count).
///
/// ```text
/// Dominator tree:
///   (GC roots)
///   ├── Foo1 (self=100, retained=300 = 100+200)
///   │   └── Foo2 (self=200, retained=200)    ← skipped (Foo already "seen")
///   └── Foo3 (self=150, retained=150)        ← counted (Foo "unseen" after leaving Foo1 subtree)
/// ```
///
/// max_ret for "Foo" = 300 + 150 = 450   (not 300 + 200 + 150 = 650)
#[test]
fn test_aggregates_max_ret_dedup() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 10, 100, 1, // node 2: Foo1, self_size=100
            3, 2, 11, 200, 0, // node 3: Foo2, self_size=200 (same name_idx=2)
            3, 2, 12, 150, 0, // node 4: Foo3, self_size=150 (same name_idx=2)
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            3,
            n(2), // GC roots → Foo1 (property "a")
            2,
            4,
            n(4), // GC roots → Foo3 (property "c")
            2,
            5,
            n(3), // Foo1 → Foo2 (property "b")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Foo",        // 2
            "a",          // 3
            "c",          // 4
            "b",          // 5
        ]),
    );

    let aggs = snap.aggregates_with_filter();
    let foo = &aggs["Foo"];

    assert_eq!(foo.count, 3);
    assert_eq!(foo.self_size, 450.0); // 100 + 200 + 150

    // max_ret: Foo1's retained (300) + Foo3's retained (150) = 450
    // Foo2 is skipped because "Foo" is marked seen while inside Foo1's subtree.
    // Without dedup it would be 300 + 200 + 150 = 650.
    assert_eq!(foo.max_ret, 450.0);
}

// ── aggregates: node type → class name mapping ─────────────────────────

/// Each node type maps to a specific class name in aggregates:
///   hidden(0)  → "(system)"
///   code(4)    → "(compiled code)"
///   closure(5) → "Function"
///   regexp(6)  → "RegExp"
#[test]
fn test_aggregates_class_names_by_node_type() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 4, // node 1: (GC roots)
            0, 2, 10, 40, 0, // node 2: hidden, self_size=40
            4, 3, 11, 50, 0, // node 3: code, self_size=50
            5, 4, 12, 60, 0, // node 4: closure, self_size=60
            6, 5, 13, 70, 0, // node 5: regexp, self_size=70
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            6,
            n(2), // GC roots → hidden (property "h")
            2,
            7,
            n(3), // GC roots → code (property "c")
            2,
            8,
            n(4), // GC roots → closure (property "f")
            2,
            9,
            n(5), // GC roots → regexp (property "r")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "stuff",      // 2 (raw name, ignored for hidden)
            "compile_me", // 3 (raw name, ignored for code)
            "myFunc",     // 4 (raw name, ignored for closure)
            "myRegexp",   // 5 (raw name, ignored for regexp)
            "h",          // 6
            "c",          // 7
            "f",          // 8
            "r",          // 9
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let system = &aggs["(system)"];
    assert_eq!(system.count, 1);
    assert_eq!(system.self_size, 40.0);

    let code = &aggs["(compiled code)"];
    assert_eq!(code.count, 1);
    assert_eq!(code.self_size, 50.0);

    let func = &aggs["Function"];
    assert_eq!(func.count, 1);
    assert_eq!(func.self_size, 60.0);

    let re = &aggs["RegExp"];
    assert_eq!(re.count, 1);
    assert_eq!(re.self_size, 70.0);
}

// ── aggregates: <tag ...> truncation ───────────────────────────────────

/// Object/native nodes whose name starts with '<' get their class name
/// truncated to "<first_word>".  E.g. '<div class="foo">' → "<div>".
#[test]
fn test_aggregates_angle_bracket_name_truncation() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 3, // node 1: (GC roots)
            3, 2, 10, 100, 0, // node 2: object '<div class="foo">', self_size=100
            3, 3, 11, 200, 0, // node 3: object '<div id="bar">', self_size=200
            3, 4, 12, 150, 0, // node 4: object '<span style="x">', self_size=150
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            5,
            n(2), // GC roots → div1 (property "a")
            2,
            6,
            n(3), // GC roots → div2 (property "b")
            2,
            7,
            n(4), // GC roots → span (property "c")
        ],
        s(&[
            "",                    // 0
            "(GC roots)",          // 1
            "<div class=\"foo\">", // 2
            "<div id=\"bar\">",    // 3
            "<span style=\"x\">",  // 4
            "a",                   // 5
            "b",                   // 6
            "c",                   // 7
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    // Both <div ...> objects grouped under "<div>"
    let div = &aggs["<div>"];
    assert_eq!(div.count, 2);
    assert_eq!(div.self_size, 300.0); // 100 + 200

    // <span ...> grouped under "<span>"
    let span = &aggs["<span>"];
    assert_eq!(span.count, 1);
    assert_eq!(span.self_size, 150.0);
}

// ── statistics: calculate_array_size with internal elements edge ────────

/// An Array node with an internal "elements" edge to a backing store.
/// js_arrays should include both the Array's self_size and the elements
/// node's self_size (when elements has exactly 1 retainer).
#[test]
fn test_statistics_array_with_elements() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 10, 80, 1, // node 2: object "Array", self_size=80
            0, 3, 11, 320, 0, // node 3: hidden "elements_store", self_size=320
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → Array (property "arr")
            3,
            5,
            n(3), // Array → elements_store (internal "elements")
        ],
        s(&[
            "",               // 0
            "(GC roots)",     // 1
            "Array",          // 2
            "elements_store", // 3
            "arr",            // 4
            "elements",       // 5
        ]),
    );

    let stats = snap.get_statistics();
    // js_arrays = Array self_size (80) + elements self_size (320) = 400
    assert_eq!(stats.js_arrays, 400.0);
    // elements_store is hidden → counted in system
    assert_eq!(stats.system, 320.0);
}

// ── statistics: native JSArrayBufferData ────────────────────────────────

/// Native node named "system / JSArrayBufferData" contributes to both
/// native_total and typed_arrays.
#[test]
fn test_statistics_native_array_buffer_data() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            8, 2, 10, 1000, 0, // node 2: native "system / JSArrayBufferData", self_size=1000
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            3,
            n(2), // GC roots → buffer (property "buf")
        ],
        s(&[
            "",                           // 0
            "(GC roots)",                 // 1
            "system / JSArrayBufferData", // 2
            "buf",                        // 3
        ]),
    );

    let stats = snap.get_statistics();
    assert_eq!(stats.native_total, 1000.0);
    assert_eq!(stats.typed_arrays, 1000.0);
    assert_eq!(stats.v8heap_total, stats.total - 1000.0);
}

// ── statistics: extra_native_bytes ──────────────────────────────────────

/// extra_native_bytes adds to total and native_total.
#[test]
fn test_statistics_extra_native_bytes() {
    let nfc = 5;
    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields: standard_node_fields(),
                node_type_enum: standard_node_type_enum(),
                edge_fields: standard_edge_fields(),
                edge_type_enum: standard_edge_type_enum(),
                location_fields: vec![],
                sample_fields: vec![],
                trace_function_info_fields: vec![],
                trace_node_fields: vec![],
            },
            node_count: 3,
            edge_count: 2,
            trace_function_count: 0,
            root_index: Some(0),
            extra_native_bytes: Some(500.0),
        },
        nodes: vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 10, 100, 0, // node 2: object "Obj", self_size=100
        ],
        edges: vec![
            1,
            0,
            (1 * nfc) as u32, // root → GC roots (element)
            2,
            3,
            (2 * nfc) as u32, // GC roots → Obj (property "o")
        ],
        strings: s(&["", "(GC roots)", "Obj", "o"]),
        locations: vec![],
    };
    let snap = HeapSnapshot::new(raw);

    let stats = snap.get_statistics();
    // total = gc_roots retained (100) + extra_native_bytes (500) = 600
    assert_eq!(stats.total, 600.0);
    // native_total = extra_native_bytes (500) only (no native nodes)
    assert_eq!(stats.native_total, 500.0);
    // v8heap_total = total - native_total = 100
    assert_eq!(stats.v8heap_total, 100.0);
}

// ── aggregates: zero self_size excluded ─────────────────────────────────

/// Objects with self_size=0 should not appear in aggregates, even if they
/// have a real class name.
#[test]
fn test_aggregates_zero_size_excluded() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            3, 2, 10, 0, 0, // node 2: object "Ghost", self_size=0
            3, 3, 11, 100, 0, // node 3: object "Real", self_size=100
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → Ghost (property "g")
            2,
            5,
            n(3), // GC roots → Real (property "r")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Ghost",      // 2
            "Real",       // 3
            "g",          // 4
            "r",          // 5
        ]),
    );

    let aggs = snap.aggregates_with_filter();
    assert!(!aggs.contains_key("Ghost"));
    assert!(aggs.contains_key("Real"));
    assert_eq!(aggs["Real"].count, 1);
}

// ── aggregates: first_seen ordering ─────────────────────────────────────

/// first_seen tracks the order classes are first encountered during the
/// ordinal scan. Earlier ordinals get lower first_seen values.
#[test]
fn test_aggregates_first_seen_ordering() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 3, // node 1: (GC roots)
            3, 2, 10, 50, 0, // node 2: "Alpha", ordinal 2
            3, 3, 11, 60, 0, // node 3: "Beta", ordinal 3
            3, 2, 12, 70, 0, // node 4: "Alpha" again (same name_idx=2)
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            4,
            n(2), // GC roots → Alpha1 (property "a1")
            2,
            5,
            n(3), // GC roots → Beta (property "b")
            2,
            6,
            n(4), // GC roots → Alpha2 (property "a2")
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Alpha",      // 2
            "Beta",       // 3
            "a1",         // 4
            "b",          // 5
            "a2",         // 6
        ]),
    );

    let aggs = snap.aggregates_with_filter();
    // Alpha encountered first (ordinal 2), Beta second (ordinal 3)
    assert!(aggs["Alpha"].first_seen < aggs["Beta"].first_seen);
}

/// Builds a snapshot with one unreachable node:
///
/// ```text
/// Node 0 (synthetic root): synthetic, "", id=1, size=0, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", id=2, size=0, 1 edge
/// Node 2: object, "Reachable", id=3, size=100, 1 edge (weak → node 3)
/// Node 3: object, "Unreachable", id=5, size=300, 1 edge (→ node 4)
/// Node 4: object, "Child", id=7, size=150, 0 edges
///
/// Edges:
///   root --element[0]--> (GC roots)
///   (GC roots) --"ref"--> node 2
///   node 2 --weak "weak_ref"--> node 3     (weak edge, does NOT make node 3 reachable)
///   node 3 --"child"--> node 4              (strong, but parent is unreachable)
/// ```
///
/// Node 3 and node 4 are unreachable because the only path to them is via a weak edge.
fn make_unreachable_snapshot() -> HeapSnapshot {
    let node_fields: Vec<String> = ["type", "name", "id", "self_size", "edge_count"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let nfc = node_fields.len(); // 5

    let node_type_enum: Vec<String> = [
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

    let edge_fields: Vec<String> = ["type", "name_or_index", "to_node"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let efc = edge_fields.len(); // 3

    let edge_type_enum: Vec<String> = [
        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Strings: 0: "", 1: "(GC roots)", 2: "Reachable", 3: "Unreachable",
    //          4: "Child", 5: "ref", 6: "weak_ref", 7: "child"
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "Reachable",
        "Unreachable",
        "Child",
        "ref",
        "weak_ref",
        "child",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let n = |ordinal: u32| ordinal * nfc as u32;

    //              type name id  size edges
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 1, // node 2: Reachable, size=100, 1 edge (weak)
        3, 3, 5, 300, 1, // node 3: Unreachable, size=300, 1 edge
        3, 4, 7, 150, 0, // node 4: Child, size=150, 0 edges
    ];

    let edges: Vec<u32> = vec![
        1,
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

#[test]
fn test_unreachable_node_distance() {
    let snap = make_unreachable_snapshot();

    // Reachable node: distance 1 (GC roots → Reachable)
    assert_eq!(snap.node_distance(NodeOrdinal(2)), 1);

    // Unreachable nodes: only reachable via weak edge, so distance = NO_DISTANCE (-5)
    assert_eq!(snap.node_distance(NodeOrdinal(3)), -5);
    assert_eq!(snap.node_distance(NodeOrdinal(4)), -5);
}

#[test]
fn test_unreachable_node_retained_size() {
    let snap = make_unreachable_snapshot();

    // Reachable node retains only itself (the weak edge doesn't count for dominance)
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 100.0);

    // Unreachable nodes still have retained sizes computed via the dominator tree.
    // Node 3 dominates node 4, so retained = 300 (self) + 150 (child) = 450.
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 450.0);
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 150.0);
}

#[test]
fn test_unreachable_node_reachable_size() {
    let snap = make_unreachable_snapshot();

    // Reachable size from node 2: just itself (weak edge is skipped)
    let info2 = snap.reachable_size(&[NodeOrdinal(2)]);
    assert_eq!(info2.size, 100.0);

    // Reachable size from node 3: itself (300) + child (150) = 450
    let info3 = snap.reachable_size(&[NodeOrdinal(3)]);
    assert_eq!(info3.size, 450.0);

    // Reachable size from node 4: just itself (150)
    let info4 = snap.reachable_size(&[NodeOrdinal(4)]);
    assert_eq!(info4.size, 150.0);
}
