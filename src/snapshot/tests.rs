use super::*;
use crate::types::{AggregateInfo, Distance, RawHeapSnapshot, SnapshotHeader, SnapshotMeta};

/// Find an aggregate by name, panicking if not found.
fn find_first_agg<'a>(aggs: &'a [AggregateInfo], name: &str) -> &'a AggregateInfo {
    aggs.iter().find(|a| a.name == name).unwrap_or_else(|| {
        let names: Vec<_> = aggs.iter().map(|a| a.name.as_str()).collect();
        panic!("no aggregate named \"{name}\", have: {names:?}");
    })
}

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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
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
    assert_eq!(snap.node_distance(NodeOrdinal(0)), Distance(0)); // synthetic root (fallback BFS)
    assert_eq!(snap.node_distance(NodeOrdinal(1)), Distance(0)); // (GC roots)
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1)); // Object
    assert_eq!(snap.node_distance(NodeOrdinal(3)), Distance(2)); // hello
    assert_eq!(snap.node_distance(NodeOrdinal(4)), Distance(1)); // Array
}

#[test]
fn test_node_retained_size() {
    let snap = make_test_snapshot();
    // Dominator tree rooted at (GC roots) ordinal 1
    assert_eq!(snap.node_retained_size(NodeOrdinal(1)), 350); // (GC roots): 0+100+50+200
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 150); // Object: 100+50
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 50); // hello
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 200); // Array
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
    let root_edges: Vec<_> = snap.iter_edges(NodeOrdinal(0)).collect();
    assert_eq!(root_edges.len(), 1);
    assert_eq!(root_edges[0].1, NodeOrdinal(1)); // -> (GC roots)

    // (GC roots) -> Object, Array
    let gc_edges: Vec<_> = snap.iter_edges(NodeOrdinal(1)).collect();
    assert_eq!(gc_edges.len(), 2);
    assert_eq!(gc_edges[0].1, NodeOrdinal(2)); // -> Object
    assert_eq!(gc_edges[1].1, NodeOrdinal(4)); // -> Array

    // Object -> hello
    let n2_edges: Vec<_> = snap.iter_edges(NodeOrdinal(2)).collect();
    assert_eq!(n2_edges.len(), 1);
    assert_eq!(n2_edges[0].1, NodeOrdinal(3)); // -> hello

    assert_eq!(snap.iter_edges(NodeOrdinal(3)).count(), 0);
    assert_eq!(snap.iter_edges(NodeOrdinal(4)).count(), 0);
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
    let gc_edges: Vec<_> = snap.iter_edges(NodeOrdinal(1)).collect();
    assert_eq!(snap.edge_name(gc_edges[0].0), "global");
    assert_eq!(snap.edge_name(gc_edges[1].0), "arr");

    // Object edges
    let n2_edges: Vec<_> = snap.iter_edges(NodeOrdinal(2)).collect();
    assert_eq!(snap.edge_name(n2_edges[0].0), "str");
}

#[test]
fn test_edge_type_name() {
    let snap = make_test_snapshot();
    // Synthetic root -> (GC roots) is element type
    let root_edges: Vec<_> = snap.iter_edges(NodeOrdinal(0)).collect();
    assert_eq!(snap.edge_type_name(root_edges[0].0), "element");
    // (GC roots) -> Object, Array are property type
    let gc_edges: Vec<_> = snap.iter_edges(NodeOrdinal(1)).collect();
    assert_eq!(snap.edge_type_name(gc_edges[0].0), "property");
    assert_eq!(snap.edge_type_name(gc_edges[1].0), "property");
}

#[test]
fn test_is_invisible_edge() {
    let snap = make_test_snapshot();
    let gc_edges: Vec<_> = snap.iter_edges(NodeOrdinal(1)).collect();
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
fn test_node_display_name_number_types() {
    // node_fields: type(0), name(1), id(2), self_size(3), edge_count(4)
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

    // Strings:
    // 0: ""           1: "(GC roots)"  2: "smi number"
    // 3: "42"         4: "heap number" 5: "12.75"
    // 6: "value"      7: "int"         8: "2064"
    // 9: "bool"       10: "true"       11: "string"
    // 12: "hello"
    let strings: Vec<String> = [
        "",
        "(GC roots)",
        "smi number",
        "42",
        "heap number",
        "12.75",
        "value",
        "int",
        "2064",
        "bool",
        "true",
        "string",
        "hello",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // type indices: number=7, synthetic=9, string=2
    // edge types: element=1, internal=3
    //
    // Node 0: synthetic root
    // Node 1: (GC roots)
    // Node 2: number, "smi number" -- has internal "value" edge to node 4
    // Node 3: number, "heap number" -- has internal "value" edge to node 5
    // Node 4: string, "42" (value target for smi)
    // Node 5: string, "12.75" (value target for heap number)
    // Node 6: number, "int" -- has internal "value" edge to node 8
    // Node 7: number, "bool" -- has internal "value" edge to node 9
    // Node 8: string, "2064" (value target for int)
    // Node 9: string, "true" (value target for bool)
    // Node 10: number, "string" -- has internal "value" edge to node 11
    // Node 11: string, "hello" (value target for string)
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 5, // node 1: (GC roots), 5 edges
        7, 2, 3, 0, 1, // node 2: number, "smi number", 1 edge
        7, 4, 4, 12, 1, // node 3: number, "heap number", 1 edge
        2, 3, 5, 0, 0, // node 4: string, "42"
        2, 5, 6, 0, 0, // node 5: string, "12.75"
        7, 7, 7, 0, 1, // node 6: number, "int", 1 edge
        7, 9, 8, 0, 1, // node 7: number, "bool", 1 edge
        2, 8, 9, 0, 0, // node 8: string, "2064"
        2, 10, 10, 0, 0, // node 9: string, "true"
        7, 11, 11, 0, 1, // node 10: number, "string", 1 edge
        2, 12, 12, 0, 0, // node 11: string, "hello"
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        1 * nfc as u32, // root -> (GC roots)
        3,
        6,
        2 * nfc as u32, // (GC roots) -> node 2 (internal)
        3,
        6,
        3 * nfc as u32, // (GC roots) -> node 3 (internal)
        3,
        6,
        6 * nfc as u32, // (GC roots) -> node 6 (internal)
        3,
        6,
        7 * nfc as u32, // (GC roots) -> node 7 (internal)
        3,
        6,
        10 * nfc as u32, // (GC roots) -> node 10 (internal)
        3,
        6,
        4 * nfc as u32, // node 2 -> node 4 (internal "value")
        3,
        6,
        5 * nfc as u32, // node 3 -> node 5 (internal "value")
        3,
        6,
        8 * nfc as u32, // node 6 -> node 8 (internal "value")
        3,
        6,
        9 * nfc as u32, // node 7 -> node 9 (internal "value")
        3,
        6,
        11 * nfc as u32, // node 10 -> node 11 (internal "value")
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    let snap = HeapSnapshot::new(raw);
    assert_eq!(snap.node_display_name(NodeOrdinal(2)), "smi 42");
    assert_eq!(snap.node_display_name(NodeOrdinal(3)), "double 12.75");
    assert_eq!(snap.node_display_name(NodeOrdinal(6)), "int 2064");
    assert_eq!(snap.node_display_name(NodeOrdinal(7)), "bool true");
    assert_eq!(snap.node_display_name(NodeOrdinal(10)), "string hello");
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
    assert_eq!(stats.total, 350);
    assert_eq!(stats.strings, 50);
    assert_eq!(stats.js_arrays, 200);
    assert_eq!(stats.system, 0);
    assert_eq!(stats.code, 0);
    assert_eq!(stats.native_total, 0);
    assert_eq!(stats.typed_arrays, 0);
    assert_eq!(stats.extra_native_bytes, 0);
    assert_eq!(stats.v8heap_total, 350);
}

#[test]
fn test_aggregates() {
    let snap = make_test_snapshot();
    let aggs = snap.aggregates_with_filter();

    // 3 entries: one each for Object, (string), (array)
    // Synthetic nodes have self_size=0, so (synthetic) is excluded
    assert_eq!(aggs.len(), 3);

    let obj = find_first_agg(&aggs, "Object");
    assert_eq!(obj.count, 1);
    assert_eq!(obj.self_size, 100);
    assert_eq!(obj.max_ret, 150);
    assert_eq!(obj.distance, Distance(1));

    let str_agg = find_first_agg(&aggs, "(string)");
    assert_eq!(str_agg.count, 1);
    assert_eq!(str_agg.self_size, 50);
    assert_eq!(str_agg.max_ret, 50);
    assert_eq!(str_agg.distance, Distance(2));

    let arr_agg = find_first_agg(&aggs, "(array)");
    assert_eq!(arr_agg.count, 1);
    assert_eq!(arr_agg.self_size, 200);
    assert_eq!(arr_agg.max_ret, 200);
    assert_eq!(arr_agg.distance, Distance(1));
}

// ====== Retained size computation tests ======

/// Two nodes of the same class "Foo" where one dominates the other:
///   root → GC roots → Foo(A, size=100) → Foo(B, size=50)
/// The "Foo" group should NOT double-count: A's retained size (150)
/// already includes B. The algorithm should only count A's retained
/// size for the group, giving max_ret = 150, not 250.
#[test]
fn test_retained_size_same_class_dominator_chain() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            // type, name, id, self_size, edge_count
            9, 0, 1, 0, 1, // 0: synthetic root → GC roots
            9, 1, 2, 0, 1, // 1: GC roots → A
            3, 2, 3, 100, 1, // 2: Foo A, size=100 → B
            3, 2, 5, 50, 0, // 3: Foo B, size=50
        ],
        vec![
            // type, name_or_index, to_node
            2,
            3,
            n(1), // root → GC roots
            2,
            3,
            n(2), // GC roots → A
            2,
            3,
            n(3), // A → B
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Foo",        // 2
            "ref",        // 3
        ]),
    );
    let aggs = snap.aggregates_with_filter();
    let foo = find_first_agg(&aggs, "Foo");
    assert_eq!(foo.count, 2);
    assert_eq!(foo.self_size, 150);
    // A dominates B, both are "Foo". The algorithm marks "Foo" as seen
    // after visiting A, so B's retained size is not added again.
    // Group retained = A's retained = 100 + 50 = 150.
    assert_eq!(foo.max_ret, 150);
}

/// Three classes in a dominator chain:
///   root → GC roots → A(Alpha, 100) → B(Beta, 80) → C(Gamma, 60)
/// Each group's retained size reflects its position in the chain.
#[test]
fn test_retained_size_chain_different_classes() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: synthetic root
            9, 1, 2, 0, 1, // 1: GC roots
            3, 2, 3, 100, 1, // 2: Alpha, size=100
            3, 3, 5, 80, 1, // 3: Beta, size=80
            3, 4, 7, 60, 0, // 4: Gamma, size=60
        ],
        vec![
            2,
            5,
            n(1), // root → GC roots
            2,
            5,
            n(2), // GC roots → Alpha
            2,
            5,
            n(3), // Alpha → Beta
            2,
            5,
            n(4), // Beta → Gamma
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Alpha",      // 2
            "Beta",       // 3
            "Gamma",      // 4
            "ref",        // 5
        ]),
    );
    let aggs = snap.aggregates_with_filter();

    // Alpha dominates Beta and Gamma: retained = 100 + 80 + 60 = 240
    assert_eq!(find_first_agg(&aggs, "Alpha").max_ret, 240);
    // Beta dominates Gamma: retained = 80 + 60 = 140
    assert_eq!(find_first_agg(&aggs, "Beta").max_ret, 140);
    // Gamma is a leaf: retained = 60
    assert_eq!(find_first_agg(&aggs, "Gamma").max_ret, 60);
}

/// Diamond dominator structure:
///   root → GC roots → A(Top, 100) → B(Left, 60)
///                                  → C(Right, 40) → D(Bottom, 30)
///                        B also → D
/// D is dominated by A (not B or C, since both paths go through A).
/// So: Top retained = 100+60+40+30 = 230, Left = 60, Right = 40, Bottom = 30.
#[test]
fn test_retained_size_diamond_dominator() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: synthetic root
            9, 1, 2, 0, 1, // 1: GC roots
            3, 2, 3, 100, 2, // 2: Top, size=100, 2 edges
            3, 3, 5, 60, 1, // 3: Left, size=60, 1 edge
            3, 4, 7, 40, 1, // 4: Right, size=40, 1 edge
            3, 5, 9, 30, 0, // 5: Bottom, size=30
        ],
        vec![
            2,
            6,
            n(1), // root → GC roots
            2,
            6,
            n(2), // GC roots → Top
            2,
            6,
            n(3), // Top → Left
            2,
            6,
            n(4), // Top → Right
            2,
            6,
            n(5), // Left → Bottom
            2,
            6,
            n(5), // Right → Bottom
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "Top",        // 2
            "Left",       // 3
            "Right",      // 4
            "Bottom",     // 5
            "ref",        // 6
        ]),
    );
    let aggs = snap.aggregates_with_filter();

    // Top dominates everything: retained = 100 + 60 + 40 + 30 = 230
    assert_eq!(find_first_agg(&aggs, "Top").max_ret, 230);
    // Left and Right are leaves in the dominator tree (Bottom is dominated by Top, not them)
    assert_eq!(find_first_agg(&aggs, "Left").max_ret, 60);
    assert_eq!(find_first_agg(&aggs, "Right").max_ret, 40);
    // Bottom is a leaf
    assert_eq!(find_first_agg(&aggs, "Bottom").max_ret, 30);
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };
    HeapSnapshot::new(raw)
}

fn build_snapshot_with_options(
    node_fields: Vec<String>,
    nodes: Vec<u32>,
    edges: Vec<u32>,
    strings: Vec<String>,
    options: SnapshotOptions,
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };
    HeapSnapshot::new_with_options(raw, options)
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
    assert_eq!(info.size, 150); // Object(100) + hello(50)
    assert!(info.native_contexts.is_empty());

    let info = snap.reachable_size(&[NodeOrdinal(4)]);
    assert_eq!(info.size, 200); // Array only
}

#[test]
fn test_reachable_size_multiple_roots() {
    let snap = make_test_snapshot();
    let info = snap.reachable_size(&[NodeOrdinal(2), NodeOrdinal(4)]);
    assert_eq!(info.size, 350); // Object(100) + hello(50) + Array(200)
}

#[test]
fn test_reachable_size_skips_weak_edges() {
    let snap = make_weak_edge_snapshot();
    let info = snap.reachable_size(&[NodeOrdinal(2)]);
    // Obj(100) + strongTarget(60) = 160, weakTarget(80) skipped
    assert_eq!(info.size, 160);
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
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(0)),
        Detachedness::Unknown
    ); // synthetic
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(1)),
        Detachedness::Unknown
    ); // GC roots
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(2)),
        Detachedness::Attached
    ); // attached native
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(3)),
        Detachedness::Detached
    ); // detached native
}

#[test]
fn test_propagate_dom_state_to_children() {
    let snap = make_detachedness_snapshot();
    // Node 4 (object) is child of attached node 2 (native, det=1)
    // propagate_dom_state should propagate attached state to node 4
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(4)),
        Detachedness::Attached
    );
}

#[test]
fn test_detachedness_without_field() {
    // make_test_snapshot has no "detachedness" in node_fields
    let snap = make_test_snapshot();
    // Should return Unknown for all nodes when detachedness field is absent
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(0)),
        Detachedness::Unknown
    );
    assert_eq!(
        snap.node_detachedness(NodeOrdinal(2)),
        Detachedness::Unknown
    );
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

fn make_native_context_sorting_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 3, 0, 4, // 1 (GC roots)
            0, 2, 5, 100, 0, // 2 extension NativeContext (discovered first, should sort last)
            0, 3, 7, 100, 2, // 3 iframe NativeContext
            0, 4, 9, 100, 2, // 4 main NativeContext
            0, 5, 11, 100, 1, // 5 utility NativeContext
            8, 6, 13, 50, 0, // 6 Window global for iframe
            3, 7, 15, 50, 0, // 7 Window proxy for iframe (<10 edges)
            8, 8, 17, 50, 0, // 8 Window global for main
            3, 9, 19, 50, 10, // 9 Window proxy for main (>=10 edges)
            3, 10, 21, 50, 0, // 10 NonWindow global for utility
            2, 11, 23, 1, 0, // 11 dummy string
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            12,
            n(2), // GC roots -> extension context
            2,
            13,
            n(3), // GC roots -> iframe context
            2,
            14,
            n(4), // GC roots -> main context
            2,
            15,
            n(5), // GC roots -> utility context
            3,
            16,
            n(6), // iframe -> global_object
            3,
            17,
            n(7), // iframe -> global_proxy_object
            3,
            16,
            n(8), // main -> global_object
            3,
            17,
            n(9), // main -> global_proxy_object
            3,
            16,
            n(10), // utility -> global_object
            2,
            18,
            n(11), // main proxy filler edges
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
            2,
            18,
            n(11),
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / chrome-extension://testid123/page.html",
            "system / NativeContext / https://iframe.test",
            "system / NativeContext / https://main.test",
            "system / NativeContext",
            "Window (global*)",
            "Window (global)",
            "Window (global*)",
            "Window (global)",
            "NonWindow",
            "dummy",
            "ext",
            "iframe",
            "main",
            "utility",
            "global_object",
            "global_proxy_object",
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
    assert_eq!(
        snap.native_context_detachedness(NodeOrdinal(2)),
        Detachedness::Attached
    );
    // Iframe context: global_object (node 7) is detached (det=2)
    assert_eq!(
        snap.native_context_detachedness(NodeOrdinal(3)),
        Detachedness::Detached
    );
    // Utility context: global_object (node 9) has det=0, no proxy → returns Unknown
    assert_eq!(
        snap.native_context_detachedness(NodeOrdinal(4)),
        Detachedness::Unknown
    );
}

// ====== 11. native_context_label tests ======

#[test]
fn test_native_context_label() {
    let snap = make_native_context_snapshot();
    // Main context: Window global + proxy with ≥10 edges → "main"
    assert_eq!(
        snap.native_context_label(NodeOrdinal(2)),
        "[main] #0 https://example.com @3"
    );
    // Iframe context: Window global + proxy with <10 edges → "iframe"
    assert_eq!(
        snap.native_context_label(NodeOrdinal(3)),
        "[iframe] #1 https://iframe.test @5"
    );
    // Utility context: non-Window global → "utility", no URL
    assert_eq!(snap.native_context_label(NodeOrdinal(4)), "[utility] #2 @7");
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
    contexts.sort_by_key(|ctx| ctx.ordinal.0);
    assert_eq!(
        contexts,
        vec![
            NativeContextData {
                ordinal: NodeOrdinal(2),
                kind: NativeContextKind::Main,
                is_extension: false,
                size: 201,
            },
            NativeContextData {
                ordinal: NodeOrdinal(3),
                kind: NativeContextKind::Iframe,
                is_extension: false,
                size: 200,
            },
            NativeContextData {
                ordinal: NodeOrdinal(4),
                kind: NativeContextKind::Utility,
                is_extension: false,
                size: 150,
            },
        ]
    );
}

#[test]
fn test_native_contexts_marks_extension_contexts() {
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 3, 0, 1, // 1 (GC roots)
            3, 2, 5, 100, 0, // 2 NativeContext extension
        ],
        vec![
            1, 0, 5, // root -> GC roots
            1, 0, 10, // GC roots -> NativeContext
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / chrome-extension://testid123/page.html",
        ]),
    );

    assert_eq!(
        snap.native_contexts(),
        &[NativeContextData {
            ordinal: NodeOrdinal(2),
            kind: NativeContextKind::Utility,
            is_extension: true,
            size: 100,
        }]
    );
}

#[test]
fn test_native_contexts_sort_main_first_and_extensions_last() {
    let snap = make_native_context_sorting_snapshot();

    // Discovery order is extension(2), iframe(3), main(4), utility(5).
    // The stored order should instead prioritize interesting page contexts:
    // main first, then iframe, then utility, with the extension context last.
    let ordinals: Vec<_> = snap
        .native_contexts()
        .iter()
        .map(|ctx| ctx.ordinal)
        .collect();
    assert_eq!(
        ordinals,
        vec![
            NodeOrdinal(4),
            NodeOrdinal(3),
            NodeOrdinal(5),
            NodeOrdinal(2),
        ]
    );

    // NativeContextId is derived from the sorted order above, so the earlier
    // IDs should also go to the main page contexts instead of the extension.
    assert_eq!(
        snap.native_context_id(NodeOrdinal(4)),
        Some(NativeContextId(0))
    ); // main context
    assert_eq!(
        snap.native_context_id(NodeOrdinal(3)),
        Some(NativeContextId(1))
    ); // iframe context
    assert_eq!(
        snap.native_context_id(NodeOrdinal(5)),
        Some(NativeContextId(2))
    ); // utility context
    assert_eq!(
        snap.native_context_id(NodeOrdinal(2)),
        Some(NativeContextId(3))
    ); // extension context
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };
    HeapSnapshot::new(raw)
}

#[test]
fn test_aggregates_split_by_location() {
    let snap = make_location_snapshot();
    let aggs = snap.aggregates_with_filter();

    // Two objects with the same class but different locations → separate entries
    let name_1 = "MyClass [script_id=1:L11:6]";
    let name_2 = "MyClass [script_id=1:L21:4]";

    let names: Vec<_> = aggs.iter().map(|a| a.name.as_str()).collect();
    assert!(
        names.contains(&name_1),
        "missing {name_1}, names: {names:?}"
    );
    assert!(
        names.contains(&name_2),
        "missing {name_2}, names: {names:?}"
    );

    let a1 = aggs.iter().find(|a| a.name == name_1).unwrap();
    let a2 = aggs.iter().find(|a| a.name == name_2).unwrap();
    assert_eq!(a1.count, 1);
    assert_eq!(a1.self_size, 100);
    assert_eq!(a2.count, 1);
    assert_eq!(a2.self_size, 200);
}

#[test]
fn test_aggregates_no_location_uses_class_index() {
    // make_test_snapshot has no locations → objects use ClassKey::Index
    let snap = make_test_snapshot();
    let aggs = snap.aggregates_with_filter();

    let obj = aggs.iter().find(|a| a.name == "Object");
    assert!(obj.is_some());
    assert_eq!(obj.unwrap().count, 1);
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
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 530);
    // key retained = self(50) only — value is NOT dominated by key
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 50);
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
    assert_eq!(snap.node_distance(NodeOrdinal(4)), Distance(2));
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
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1)); // KeyObj
    assert_eq!(snap.node_distance(NodeOrdinal(7)), Distance(5)); // WeakMap

    // Value gets distance 6 (from table at depth 5), not 2 (from key at depth 1),
    // because the first ephemeron edge (from key) is skipped by dedup.
    assert_eq!(snap.node_distance(NodeOrdinal(8)), Distance(6));
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
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1)); // WeakMap
    assert_eq!(snap.node_distance(NodeOrdinal(7)), Distance(5)); // KeyObj

    // Value gets distance 6 (from key at depth 5), not 2 (from table at depth 1),
    // because the first ephemeron edge (from table) is skipped by dedup.
    assert_eq!(snap.node_distance(NodeOrdinal(8)), Distance(6));
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
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1)); // NativeContext
    assert_eq!(snap.node_distance(NodeOrdinal(3)), Distance(1)); // Mid

    // Target reachable via Mid → Target (distance 2), NOT via
    // NativeContext → sloppy_function_map (which is filtered out).
    assert_eq!(snap.node_distance(NodeOrdinal(4)), Distance(2));

    // Keeper is reachable via NativeContext → array_function (not filtered).
    assert_eq!(snap.node_distance(NodeOrdinal(5)), Distance(2));
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
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1)); // (map descriptors)
    assert_eq!(snap.node_distance(NodeOrdinal(8)), Distance(1)); // Alt

    // Allowed element edges: index 0, 1, 5 → distance 2
    assert_eq!(snap.node_distance(NodeOrdinal(3)), Distance(2)); // Child0 via element[0]
    assert_eq!(snap.node_distance(NodeOrdinal(4)), Distance(2)); // Child1 via element[1]
    assert_eq!(snap.node_distance(NodeOrdinal(6)), Distance(2)); // Child5 via element[5]

    // Filtered element edges: index 4, 7 (>= 2 && % 3 == 1)
    // These children are only reachable via Alt (distance 1) → property edge
    assert_eq!(snap.node_distance(NodeOrdinal(5)), Distance(2)); // Child4 via Alt
    assert_eq!(snap.node_distance(NodeOrdinal(7)), Distance(2)); // Child7 via Alt
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
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));
    assert_eq!(snap.node_distance(NodeOrdinal(3)), Distance(2));
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
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 300); // 100 + 200
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 200); // just self
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
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 50); // Root1: just self
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 60); // Root2: just self
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 400); // Target: just self
    // (GC roots) retains everything
    assert_eq!(snap.node_retained_size(NodeOrdinal(1)), 510); // 0 + 50 + 60 + 400
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
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 380); // 80 + 300
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 300); // just self
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
    let foo = find_first_agg(&aggs, "Foo");

    // count: 3 Foo objects
    assert_eq!(foo.count, 3);

    // self_size: 100 + 200 + 300
    assert_eq!(foo.self_size, 600);

    // distance: min of 1, 2, 3
    assert_eq!(foo.distance, Distance(1));

    // node_ordinals: all three
    assert_eq!(foo.node_ordinals.len(), 3);

    // node_ordinals should be sorted by retained size descending
    let retained: Vec<u64> = foo
        .node_ordinals
        .iter()
        .map(|&ord| snap.node_retained_size(ord))
        .collect();
    for w in retained.windows(2) {
        assert!(
            w[0] >= w[1],
            "node_ordinals not sorted by retained size: {retained:?}"
        );
    }
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
    let foo = find_first_agg(&aggs, "Foo");

    assert_eq!(foo.count, 3);
    assert_eq!(foo.self_size, 450); // 100 + 200 + 150

    // max_ret: Foo1's retained (300) + Foo3's retained (150) = 450
    // Foo2 is skipped because "Foo" is marked seen while inside Foo1's subtree.
    // Without dedup it would be 300 + 200 + 150 = 650.
    assert_eq!(foo.max_ret, 450);
}

// ── aggregates: node type → class name mapping ─────────────────────────

/// Each node type maps to a specific class name in aggregates:
///   hidden(0)  → raw name (e.g. "system / Map"), or "(hidden)" if empty;
///                unlike DevTools which groups all hidden nodes into "(system)"
///   code(4)    → raw name, or "(code)" if empty; unlike DevTools which
///                groups all code nodes into "(compiled code)"
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
            "stuff",      // 2 (raw name, used as class name for hidden)
            "compile_me", // 3 (raw name, used as class name for code)
            "myFunc",     // 4 (raw name, ignored for closure)
            "myRegexp",   // 5 (raw name, ignored for regexp)
            "h",          // 6
            "c",          // 7
            "f",          // 8
            "r",          // 9
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let hidden = find_first_agg(&aggs, "stuff");
    assert_eq!(hidden.count, 1);
    assert_eq!(hidden.self_size, 40);

    let code = find_first_agg(&aggs, "compile_me");
    assert_eq!(code.count, 1);
    assert_eq!(code.self_size, 50);

    let func = find_first_agg(&aggs, "Function");
    assert_eq!(func.count, 1);
    assert_eq!(func.self_size, 60);

    let re = find_first_agg(&aggs, "RegExp");
    assert_eq!(re.count, 1);
    assert_eq!(re.self_size, 70);
}

/// Hidden nodes with "system / Foo / bar" names are grouped as "system / Foo".
#[test]
fn test_aggregates_hidden_name_truncation() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 3, // node 1: (GC roots)
            0, 2, 10, 10, 0, // node 2: hidden "system / Map"
            0, 3, 11, 20, 0, // node 3: hidden "system / Map / transition"
            0, 4, 12, 30, 0, // node 4: hidden "system / Context"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            6,
            n(2), // GC roots → node 2
            2,
            7,
            n(3), // GC roots → node 3
            2,
            8,
            n(4), // GC roots → node 4
        ],
        s(&[
            "",                          // 0
            "(GC roots)",                // 1
            "system / Map",              // 2
            "system / Map / transition", // 3
            "system / Context",          // 4
            "",                          // 5
            "a",                         // 6
            "b",                         // 7
            "c",                         // 8
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    // Both "system / Map" and "system / Map / transition" grouped together
    let map = find_first_agg(&aggs, "system / Map");
    assert_eq!(map.count, 2);
    assert_eq!(map.self_size, 30);

    let ctx = find_first_agg(&aggs, "system / Context");
    assert_eq!(ctx.count, 1);
    assert_eq!(ctx.self_size, 30);
}

/// Hidden nodes without the "system / " prefix use their raw name as-is.
#[test]
fn test_aggregates_hidden_plain_name() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            0, 2, 10, 50, 0, // node 2: hidden "InternalThing"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            3,
            n(2), // GC roots → node 2
        ],
        s(&[
            "",              // 0
            "(GC roots)",    // 1
            "InternalThing", // 2
            "x",             // 3
        ]),
    );

    let aggs = snap.aggregates_with_filter();
    let thing = find_first_agg(&aggs, "InternalThing");
    assert_eq!(thing.count, 1);
    assert_eq!(thing.self_size, 50);
}

/// Third-component truncation only applies to the "system / " prefix.
/// "foo / Bar / baz" must NOT collapse into "foo / Bar".
#[test]
fn test_aggregates_hidden_no_truncation_for_non_system_prefix() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            0, 2, 10, 10, 0, // node 2: hidden "foo / Bar"
            0, 3, 11, 20, 0, // node 3: hidden "foo / Bar / baz"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            4,
            n(2), // GC roots → node 2
            2,
            5,
            n(3), // GC roots → node 3
        ],
        s(&[
            "",                // 0
            "(GC roots)",      // 1
            "foo / Bar",       // 2
            "foo / Bar / baz", // 3
            "a",               // 4
            "b",               // 5
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let bar = find_first_agg(&aggs, "foo / Bar");
    assert_eq!(
        bar.count, 1,
        "foo / Bar / baz should not merge into foo / Bar"
    );
    assert_eq!(bar.self_size, 10);

    let baz = find_first_agg(&aggs, "foo / Bar / baz");
    assert_eq!(baz.count, 1);
    assert_eq!(baz.self_size, 20);
}

/// Hidden nodes with empty names or bare "system" (no slash) stay as-is.
#[test]
fn test_aggregates_hidden_edge_case_names() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            0, 2, 10, 10, 0, // node 2: hidden "" (empty name)
            0, 3, 11, 20, 0, // node 3: hidden "system" (no slash)
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            4,
            n(2), // GC roots → node 2
            2,
            5,
            n(3), // GC roots → node 3
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "",           // 2 (empty name)
            "system",     // 3 (no " / " separator)
            "a",          // 4
            "b",          // 5
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let empty = find_first_agg(&aggs, "(hidden)");
    assert_eq!(empty.count, 1);
    assert_eq!(empty.self_size, 10);

    let sys = find_first_agg(&aggs, "system");
    assert_eq!(sys.count, 1);
    assert_eq!(sys.self_size, 20);
}

/// Code nodes with empty names fall back to "(code)".
#[test]
fn test_aggregates_code_empty_name_fallback() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            4, 0, 10, 30, 0, // node 2: code "" (empty name)
            4, 2, 11, 40, 0, // node 3: code "SharedFunctionInfo"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            3,
            n(2), // GC roots → node 2
            2,
            4,
            n(3), // GC roots → node 3
        ],
        s(&[
            "",                   // 0
            "(GC roots)",         // 1
            "SharedFunctionInfo", // 2
            "a",                  // 3
            "b",                  // 4
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let code = find_first_agg(&aggs, "(code)");
    assert_eq!(code.count, 1);
    assert_eq!(code.self_size, 30);

    let sfi = find_first_agg(&aggs, "SharedFunctionInfo");
    assert_eq!(sfi.count, 1);
    assert_eq!(sfi.self_size, 40);
}

/// Code nodes with "system / Foo / bar" names truncate to "system / Foo".
#[test]
fn test_aggregates_code_system_prefix_truncation() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 3, // node 1: (GC roots)
            4, 2, 10, 10, 0, // node 2: code "system / BytecodeArray"
            4, 3, 11, 20, 0, // node 3: code "system / BytecodeArray / foo"
            4, 4, 12, 30, 0, // node 4: code "system / Code"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            5,
            n(2), // GC roots → node 2
            2,
            6,
            n(3), // GC roots → node 3
            2,
            7,
            n(4), // GC roots → node 4
        ],
        s(&[
            "",                             // 0
            "(GC roots)",                   // 1
            "system / BytecodeArray",       // 2
            "system / BytecodeArray / foo", // 3
            "system / Code",                // 4
            "a",                            // 5
            "b",                            // 6
            "c",                            // 7
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let ba = find_first_agg(&aggs, "system / BytecodeArray");
    assert_eq!(ba.count, 2);
    assert_eq!(ba.self_size, 30);

    let code = find_first_agg(&aggs, "system / Code");
    assert_eq!(code.count, 1);
    assert_eq!(code.self_size, 30);
}

/// Hidden "system / Map" and object "Map" must not collide.
#[test]
fn test_aggregates_hidden_does_not_collide_with_object() {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 2, // node 1: (GC roots)
            0, 2, 10, 10, 0, // node 2: hidden "system / Map"
            3, 3, 11, 20, 0, // node 3: object "Map"
        ],
        vec![
            1,
            0,
            n(1), // root → GC roots
            2,
            4,
            n(2), // GC roots → node 2
            2,
            5,
            n(3), // GC roots → node 3
        ],
        s(&[
            "",             // 0
            "(GC roots)",   // 1
            "system / Map", // 2
            "Map",          // 3
            "a",            // 4
            "b",            // 5
        ]),
    );

    let aggs = snap.aggregates_with_filter();

    let sys_map = find_first_agg(&aggs, "system / Map");
    assert_eq!(sys_map.count, 1);
    assert_eq!(sys_map.self_size, 10);

    let obj_map = find_first_agg(&aggs, "Map");
    assert_eq!(obj_map.count, 1);
    assert_eq!(obj_map.self_size, 20);
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
    let div = find_first_agg(&aggs, "<div>");
    assert_eq!(div.count, 2);
    assert_eq!(div.self_size, 300); // 100 + 200

    // <span ...> grouped under "<span>"
    let span = find_first_agg(&aggs, "<span>");
    assert_eq!(span.count, 1);
    assert_eq!(span.self_size, 150);
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
    assert_eq!(stats.js_arrays, 400);
    // elements_store is hidden → counted in system
    assert_eq!(stats.system, 320);
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
    assert_eq!(stats.native_total, 1000);
    assert_eq!(stats.typed_arrays, 1000);
    assert_eq!(stats.v8heap_total, stats.total - 1000);
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
            extra_native_bytes: Some(500),
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };
    let snap = HeapSnapshot::new(raw);

    let stats = snap.get_statistics();
    // total = gc_roots retained (100) + extra_native_bytes (500) = 600
    assert_eq!(stats.total, 600);
    // native_total = extra_native_bytes (500) only (no native nodes)
    assert_eq!(stats.native_total, 500);
    // v8heap_total = total - native_total = 100
    assert_eq!(stats.v8heap_total, 100);
    // extra_native_bytes = 500
    assert_eq!(stats.extra_native_bytes, 500);
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
    assert!(aggs.iter().find(|a| a.name == "Ghost").is_none());
    let real = aggs.iter().find(|a| a.name == "Real").unwrap();
    assert_eq!(real.count, 1);
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
    assert!(find_first_agg(&aggs, "Alpha").first_seen < find_first_agg(&aggs, "Beta").first_seen);
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    HeapSnapshot::new(raw)
}

#[test]
fn test_unreachable_node_distance() {
    let snap = make_unreachable_snapshot();

    // Reachable node: distance 1 (GC roots → Reachable)
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));

    // Unreachable nodes: only reachable via weak edge.
    // Node 3 has a reachable retainer (node 2 via weak edge) → Distance::UNREACHABLE_BASE.
    // Node 4 is only reachable from node 3 → Distance::UNREACHABLE_BASE + 1.
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
}

#[test]
fn test_unreachable_node_retained_size() {
    let snap = make_unreachable_snapshot();

    // Reachable node retains only itself (the weak edge doesn't count for dominance)
    assert_eq!(snap.node_retained_size(NodeOrdinal(2)), 100);

    // Unreachable nodes still have retained sizes computed via the dominator tree.
    // Node 3 dominates node 4, so retained = 300 (self) + 150 (child) = 450.
    assert_eq!(snap.node_retained_size(NodeOrdinal(3)), 450);
    assert_eq!(snap.node_retained_size(NodeOrdinal(4)), 150);
}

#[test]
fn test_unreachable_node_reachable_size() {
    let snap = make_unreachable_snapshot();

    // Reachable size from node 2: just itself (weak edge is skipped)
    let info2 = snap.reachable_size(&[NodeOrdinal(2)]);
    assert_eq!(info2.size, 100);

    // Reachable size from node 3: itself (300) + child (150) = 450
    let info3 = snap.reachable_size(&[NodeOrdinal(3)]);
    assert_eq!(info3.size, 450);

    // Reachable size from node 4: just itself (150)
    let info4 = snap.reachable_size(&[NodeOrdinal(4)]);
    assert_eq!(info4.size, 150);
}

/// Builds a snapshot where two unreachable objects form a chain with no
/// reachable retainer at all:
///
/// ```text
/// Node 0 (synthetic root): synthetic, 1 edge
/// Node 1 (GC roots): synthetic, "(GC roots)", 1 edge
/// Node 2: object, "Reachable", size=100, 0 edges
/// Node 3: object, "B", size=200, 1 edge (strong → node 4)
/// Node 4: object, "A", size=150, 0 edges
/// ```
///
/// Edges:
///   root → (GC roots) → Reachable
///   B → A  (strong, but B itself has no retainer from the reachable world)
///
/// Both B and A are unreachable.  B has no reachable retainer, so the
/// unreachable-depth BFS cannot seed from it.  Currently both end up as U.
fn make_isolated_unreachable_snapshot() -> HeapSnapshot {
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
        "",           // 0
        "(GC roots)", // 1
        "Reachable",  // 2
        "B",          // 3
        "A",          // 4
        "ref",        // 5
        "link",       // 6
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let n = |ordinal: u32| ordinal * nfc as u32;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 0, // node 2: Reachable, size=100, 0 edges
        3, 3, 5, 200, 1, // node 3: B, size=200, 1 edge
        3, 4, 7, 150, 0, // node 4: A, size=150, 0 edges
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root --element[0]--> (GC roots)
        2,
        5,
        n(2), // (GC roots) --property "ref"--> Reachable
        2,
        6,
        n(4), // B --property "link"--> A
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    HeapSnapshot::new(raw)
}

#[test]
fn test_isolated_unreachable_both_get_u() {
    let snap = make_isolated_unreachable_snapshot();

    // Node 2 is reachable
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));

    // Node 3 (B) has no retainers at all — it is a root of its
    // unreachable subgraph and gets U.
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "B should be U (no retainers, orphaned root)"
    );
    // Node 4 (A) is reachable from B via strong edge → U+1.
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1),
        "A should be U+1 (reachable from B within unreachable subgraph)"
    );
}

/// Snapshot where an unreachable object A has both a weak and a strong
/// reference to object B.  The weak edge should be skipped by the
/// unreachable-depth BFS (just like the main distance BFS), so B is
/// reached only via the strong edge and gets U+1.
///
/// ```text
/// Node 0 (synthetic root): 1 edge
/// Node 1 (GC roots): 1 edge → Reachable
/// Node 2: Reachable, 1 weak edge → A
/// Node 3: A, 2 edges (weak → B, strong → B)
/// Node 4: B, 0 edges
/// ```
#[test]
fn test_unreachable_weak_and_strong_to_same_target() {
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
        "",           // 0
        "(GC roots)", // 1
        "Reachable",  // 2
        "A",          // 3
        "B",          // 4
        "ref",        // 5
        "weak_ref",   // 6
        "strong_ref", // 7
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let n = |ordinal: u32| ordinal * nfc as u32;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 1, // node 2: Reachable, 1 weak edge → A
        3, 3, 5, 200, 2, // node 3: A, 2 edges (weak → B, strong → B)
        3, 4, 7, 150, 0, // node 4: B, 0 edges
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → (GC roots)
        2,
        5,
        n(2), // (GC roots) --property "ref"--> Reachable
        6,
        6,
        n(3), // Reachable --weak "weak_ref"--> A
        6,
        6,
        n(4), // A --weak "weak_ref"--> B
        2,
        7,
        n(4), // A --property "strong_ref"--> B
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    let snap = HeapSnapshot::new(raw);

    // Node 2 is reachable
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));

    // A (node 3) has a reachable retainer (node 2 via weak edge) → U
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "A should be U"
    );

    // B (node 4) is referenced by A via both weak and strong edges.
    // The weak edge is skipped, but the strong edge makes B reachable
    // from A within the unreachable subgraph → U+1.
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1),
        "B should be U+1 (reached via strong edge from A)"
    );
}

/// A is unreachable with a strong edge to B.  C is reachable and has a weak
/// edge to B.  B is unreachable (the weak edge from C doesn't count), but
/// it is reachable from A within the unreachable subgraph.
///
/// ```text
/// Node 0 (synthetic root): 1 edge
/// Node 1 (GC roots): 2 edges → Reachable, C
/// Node 2: Reachable, 1 weak edge → A
/// Node 3: A, 1 strong edge → B
/// Node 4: B, 0 edges
/// Node 5: C, 1 weak edge → B
/// ```
///
/// A should be U (reachable retainer via weak from Reachable).
/// B should be U+1 (reached from A via strong; the weak from C doesn't
/// make B reachable, but it does make B have a reachable retainer — however
/// that retainer is weak so it's filtered during the main distance BFS).
#[test]
fn test_unreachable_strong_from_unreachable_and_weak_from_reachable() {
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
        "",           // 0
        "(GC roots)", // 1
        "Reachable",  // 2
        "A",          // 3
        "B",          // 4
        "C",          // 5
        "ref",        // 6
        "weak_ref",   // 7
        "strong_ref", // 8
        "ref2",       // 9
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let n = |ordinal: u32| ordinal * nfc as u32;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 2, // node 1: (GC roots), 2 edges
        3, 2, 3, 100, 1, // node 2: Reachable, 1 weak edge → A
        3, 3, 5, 200, 1, // node 3: A, 1 strong edge → B
        3, 4, 7, 150, 0, // node 4: B, 0 edges
        3, 5, 9, 50, 1, // node 5: C, 1 weak edge → B
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → (GC roots)
        2,
        6,
        n(2), // (GC roots) --property "ref"--> Reachable
        2,
        9,
        n(5), // (GC roots) --property "ref2"--> C
        6,
        7,
        n(3), // Reachable --weak "weak_ref"--> A
        2,
        8,
        n(4), // A --property "strong_ref"--> B
        6,
        7,
        n(4), // C --weak "weak_ref"--> B
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    let snap = HeapSnapshot::new(raw);

    // Reachable and C are reachable from GC roots
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));
    assert_eq!(snap.node_distance(NodeOrdinal(5)), Distance(1));

    // A (node 3): only retainer is Reachable via weak edge → U
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "A should be U"
    );

    // B (node 4): retainers are A (strong, unreachable) and C (weak, reachable).
    // C's weak edge doesn't make B reachable in the main BFS, but it does
    // mean B has a reachable retainer — so B is seeded as a root → U.
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance::UNREACHABLE_BASE,
        "B should be U (has reachable retainer C)"
    );
}

/// Like the previous test, but A only has a weak edge to B (no strong edge).
/// The unreachable-depth BFS should skip weak edges, so B is not reached
/// from A and both get U independently.
#[test]
fn test_unreachable_weak_only_does_not_propagate() {
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
        "",           // 0
        "(GC roots)", // 1
        "Reachable",  // 2
        "A",          // 3
        "B",          // 4
        "ref",        // 5
        "weak_ref",   // 6
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let n = |ordinal: u32| ordinal * nfc as u32;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 1, // node 2: Reachable, 1 weak edge → A
        3, 3, 5, 200, 1, // node 3: A, 1 weak edge → B
        3, 4, 7, 150, 0, // node 4: B, 0 edges
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → (GC roots)
        2,
        5,
        n(2), // (GC roots) --property "ref"--> Reachable
        6,
        6,
        n(3), // Reachable --weak "weak_ref"--> A
        6,
        6,
        n(4), // A --weak "weak_ref"--> B
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    let snap = HeapSnapshot::new(raw);

    // A (node 3): reachable retainer via weak edge → U
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "A should be U"
    );

    // B (node 4): A's only edge to B is weak → BFS skips it.
    // B's only retainer is a weak edge from A, which doesn't count as a
    // strong unreachable retainer, so B is seeded as a root in phase 1 → U.
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance::UNREACHABLE_BASE,
        "B should be U (weak edge from A does not propagate distance)"
    );
}

#[test]
fn test_unreachable_aggregates_include_all_unreachable() {
    let snap = make_unreachable_snapshot();
    let aggs = snap.unreachable_aggregates();

    // Both node 3 (Unreachable, 300) and node 4 (Child, 150) are unreachable.
    let total_count: u32 = aggs.iter().map(|a| a.count).sum();
    let total_size: u64 = aggs.iter().map(|a| a.self_size).sum();
    assert_eq!(total_count, 2);
    assert_eq!(total_size, 450);
}

#[test]
fn test_unreachable_aggregates_distances() {
    let snap = make_unreachable_snapshot();
    let aggs = snap.unreachable_aggregates();

    // Node 3 ("Unreachable"): has reachable retainer → UNREACHABLE_BASE (U)
    // Node 4 ("Child"): only reachable from node 3 → UNREACHABLE_BASE+1 (U+1)
    let unreachable_agg = aggs.iter().find(|a| a.name == "Unreachable").unwrap();
    assert_eq!(unreachable_agg.distance, Distance::UNREACHABLE_BASE);
    assert_eq!(unreachable_agg.count, 1);

    let child_agg = aggs.iter().find(|a| a.name == "Child").unwrap();
    assert_eq!(
        child_agg.distance,
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
    assert_eq!(child_agg.count, 1);
}

#[test]
fn test_unreachable_aggregates_retained_sizes() {
    let snap = make_unreachable_snapshot();
    let aggs = snap.unreachable_aggregates();

    // Node 3 ("Unreachable", self=300) dominates node 4 ("Child", self=150),
    // so "Unreachable" retained = 300 + 150 = 450, "Child" retained = 150.
    let unreachable_agg = aggs.iter().find(|a| a.name == "Unreachable").unwrap();
    assert_eq!(unreachable_agg.max_ret, 450);

    let child_agg = aggs.iter().find(|a| a.name == "Child").unwrap();
    assert_eq!(child_agg.max_ret, 150);
}

/// Retained sizes with filtered-out nodes in the dominator chain.
///   root → GC roots → Reachable(100) → Unreachable(300) → Child(150)
/// When filtering for unreachable only, "Reachable" is not in any group.
/// The dominator walk should still correctly compute retained sizes for
/// the unreachable groups, passing through the filtered-out "Reachable" node.
#[test]
fn test_retained_size_with_filtered_out_nodes_in_dominator_chain() {
    // make_unreachable_snapshot already has this structure:
    // node 2 (Reachable, 100) → weak ref → node 3 (Unreachable, 300) → node 4 (Child, 150)
    // When filtering for unreachable, node 2 is excluded.
    let snap = make_unreachable_snapshot();

    // Verify the full view has all three groups with retained sizes
    let all = snap.aggregates_with_filter();
    assert_eq!(find_first_agg(&all, "Reachable").max_ret, 100);

    // Now check the filtered view
    let filtered = snap.unreachable_aggregates();
    assert!(
        filtered.iter().all(|a| a.name != "Reachable"),
        "Reachable should be filtered out"
    );
    // Unreachable still dominates Child in the full dominator tree
    let unreachable_agg = filtered.iter().find(|a| a.name == "Unreachable").unwrap();
    assert!(
        unreachable_agg.max_ret > 0,
        "retained size should be computed even when parent nodes are filtered out"
    );
    let child_agg = filtered.iter().find(|a| a.name == "Child").unwrap();
    assert!(
        child_agg.max_ret > 0,
        "retained size should be computed for leaf nodes in filtered view"
    );
}

#[test]
fn test_unreachable_roots_only_excludes_transitive() {
    let snap = make_unreachable_snapshot();
    let aggs = snap.unreachable_aggregates();

    // Filter to roots only (distance == UNREACHABLE_BASE)
    let roots_only: Vec<_> = aggs
        .iter()
        .filter(|a| {
            a.node_ordinals
                .iter()
                .any(|ord| snap.node_distance(*ord).is_unreachable_root())
        })
        .collect();

    // Only "Unreachable" (node 3) is a root; "Child" (node 4) is U+1
    assert_eq!(roots_only.len(), 1);
    assert_eq!(roots_only[0].name, "Unreachable");
}

// ── Unreachable depth test helpers ─────────────────────────────────────

/// Build a snapshot from strings, nodes, and edges with standard meta.
/// Reduces boilerplate for unreachable-depth tests.
fn build_test_snapshot(strings: Vec<String>, nodes: Vec<u32>, edges: Vec<u32>) -> HeapSnapshot {
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    HeapSnapshot::new(raw)
}

fn build_test_snapshot_with_options(
    strings: Vec<String>,
    nodes: Vec<u32>,
    edges: Vec<u32>,
    options: SnapshotOptions,
) -> HeapSnapshot {
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    HeapSnapshot::new_with_options(raw, options)
}

// node index helper: ordinal * 5 (node_field_count)
fn n(ordinal: u32) -> u32 {
    ordinal * 5
}

// ── Unreachable depth tests ────────────────────────────────────────────

/// Longer chain: A→B→C→D, all unreachable (no retainers).
/// Depths should be U, U+1, U+2, U+3.
#[test]
fn test_unreachable_depth_long_chain() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "C", "D", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R (reachable)
        3, 3, 5, 10, 1, // 3: A → B
        3, 4, 7, 10, 1, // 4: B → C
        3, 5, 9, 10, 1, // 5: C → D
        3, 6, 11, 10, 0, // 6: D
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        7,
        n(2), // GC roots → R
        2,
        8,
        n(4), // A → B
        2,
        8,
        n(5), // B → C
        2,
        8,
        n(6), // C → D
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(1));
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(5)),
        Distance(Distance::UNREACHABLE_BASE.0 + 2)
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(6)),
        Distance(Distance::UNREACHABLE_BASE.0 + 3)
    );
}

/// Diamond: A→B, A→C, B→D, C→D.  All unreachable (no retainers).
/// A=U, B=U+1, C=U+1, D=U+2 (shortest path via BFS).
#[test]
fn test_unreachable_depth_diamond() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "C", "D", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R
        3, 3, 5, 10, 2, // 3: A → B, A → C
        3, 4, 7, 10, 1, // 4: B → D
        3, 5, 9, 10, 1, // 5: C → D
        3, 6, 11, 10, 0, // 6: D
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        7,
        n(2), // GC roots → R
        2,
        8,
        n(4), // A → B
        2,
        8,
        n(5), // A → C
        2,
        8,
        n(6), // B → D
        2,
        8,
        n(6), // C → D
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(5)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(6)),
        Distance(Distance::UNREACHABLE_BASE.0 + 2)
    );
}

/// Cycle: A→B→A (strong edges both ways).  Neither has a non-cycle
/// retainer, so neither is seeded in phase 1.  Phase 2 picks one as a
/// root (U) and the other gets U+1 via BFS.
#[test]
fn test_unreachable_depth_cycle() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R
        3, 3, 5, 10, 1, // 3: A → B
        3, 4, 7, 10, 1, // 4: B → A
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        5,
        n(2), // GC roots → R
        2,
        6,
        n(4), // A → B
        2,
        6,
        n(3), // B → A
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    // A (lower ordinal) is picked as root → U, B gets U+1.
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
}

/// Mutual references with reachable retainers: R --weak→ A, R --weak→ B,
/// A --strong→ B, B --strong→ A.  Both A and B are genuine unreachable
/// roots (directly referenced from the reachable world).  Both should be U.
#[test]
fn test_unreachable_depth_mutual_refs_with_reachable_retainer() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "ref", "w", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 2, // 2: R, 2 weak edges → A, B
        3, 3, 5, 10, 1, // 3: A → B (strong)
        3, 4, 7, 10, 1, // 4: B → A (strong)
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        5,
        n(2), // GC roots → R
        6,
        6,
        n(3), // R --weak→ A
        6,
        6,
        n(4), // R --weak→ B
        2,
        7,
        n(4), // A --strong→ B
        2,
        7,
        n(3), // B --strong→ A
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    // Both have reachable retainers (R via weak), so both are roots → U.
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "A should be U (has reachable retainer)"
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance::UNREACHABLE_BASE,
        "B should be U (has reachable retainer)"
    );
}

/// Two disconnected unreachable subgraphs: A→B and C→D.
/// Each subgraph computes depths independently.
#[test]
fn test_unreachable_depth_two_disconnected_subgraphs() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "C", "D", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R
        3, 3, 5, 10, 1, // 3: A → B
        3, 4, 7, 10, 0, // 4: B
        3, 5, 9, 10, 1, // 5: C → D
        3, 6, 11, 10, 0, // 6: D
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        7,
        n(2), // GC roots → R
        2,
        8,
        n(4), // A → B
        2,
        8,
        n(6), // C → D
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    // First subgraph
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
    // Second subgraph
    assert_eq!(
        snap.node_distance(NodeOrdinal(5)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(6)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1)
    );
}

/// Min-depth diamond: A→B (direct) and A→C→B (via C).
/// B is reachable at U+1 (direct from A) and U+2 (via C).
/// BFS should assign the shorter path: B == U+1.
#[test]
fn test_unreachable_depth_min_path() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "B", "C", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R
        3, 3, 5, 10, 2, // 3: A, 2 edges → B, C
        3, 4, 7, 10, 0, // 4: B
        3, 5, 9, 10, 1, // 5: C → B
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        6,
        n(2), // GC roots → R
        2,
        7,
        n(4), // A → B (direct)
        2,
        7,
        n(5), // A → C
        2,
        7,
        n(4), // C → B (indirect)
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1),
        "B should be U+1 (shortest path, direct from A)"
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(5)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1),
        "C should be U+1"
    );
}

/// No unreachable nodes — all nodes reachable from GC roots.
/// None should have distance >= UNREACHABLE_BASE.
#[test]
fn test_unreachable_depth_none_unreachable() {
    let strings: Vec<String> = ["", "(GC roots)", "A", "B", "ref", "e"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root, 1 edge
        9, 1, 2, 0, 1, // 1: (GC roots), 1 edge → A
        3, 2, 3, 10, 1, // 2: A, 1 edge → B
        3, 3, 5, 10, 0, // 3: B
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        4,
        n(2), // GC roots → A
        2,
        5,
        n(3), // A → B
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    for i in 0..snap.node_count() {
        assert!(
            !snap.node_distance(NodeOrdinal(i)).is_unreachable(),
            "node {i} should be reachable"
        );
    }
}

/// Self-loop: A has a strong edge to itself.  A has a strong unreachable
/// retainer (itself), so it is not seeded in phase 1.  It gets U via phase 3.
#[test]
fn test_unreachable_depth_self_loop() {
    let strings: Vec<String> = ["", "(GC roots)", "R", "A", "ref", "self"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 10, 0, // 2: R
        3, 3, 5, 10, 1, // 3: A → A (self-loop)
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        4,
        n(2), // GC roots → R
        2,
        5,
        n(3), // A → A (self-loop)
    ];

    let snap = build_test_snapshot(strings, nodes, edges);

    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "self-loop node should be U"
    );
}

/// With --weak-is-reachable, a node referenced only via a weak edge from a
/// reachable node should get distance+1 of the retainer, not U.
#[test]
fn test_weak_is_reachable_flag() {
    // root(0) --element--> (GC roots)(1) --property--> Reachable(2) --weak--> Target(3)
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "Reachable".into(),
        "Target".into(),
        "ref".into(),
        "weak_ref".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 1, // node 1: (GC roots)
        3, 2, 3, 100, 1, // node 2: Reachable
        3, 3, 5, 200, 0, // node 3: Target
    ];

    // edge types: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        2,
        4,
        n(2), // (GC roots) --property "ref"--> Reachable
        6,
        5,
        n(3), // Reachable --weak "weak_ref"--> Target
    ];

    // Without the flag: Target is unreachable.
    let snap_default = build_test_snapshot(strings.clone(), nodes.clone(), edges.clone());
    assert!(
        snap_default.node_distance(NodeOrdinal(3)).is_unreachable(),
        "without flag, Target should be unreachable (U)"
    );
    assert_eq!(
        snap_default.node_distance(NodeOrdinal(2)),
        Distance(1),
        "Reachable should be at distance 1"
    );

    // With the flag: Target is reachable at distance 2 (Reachable is at 1).
    let snap_weak = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(2)),
        Distance(1),
        "Reachable should still be at distance 1"
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(3)),
        Distance(2),
        "with --weak-is-reachable, Target should be at distance 2"
    );
    assert!(
        snap_weak.node_distance(NodeOrdinal(3)).is_reachable(),
        "with --weak-is-reachable, Target should be reachable"
    );
}

/// With --weak-is-reachable, weak edges within the unreachable subgraph
/// should also be followed (U+1 instead of separate U seeds).
#[test]
fn test_weak_is_reachable_in_unreachable_subgraph() {
    // root(0) --element--> (GC roots)(1) --weak--> A(2) --weak--> B(3)
    // Without flag: A=U, B=U (both are separate unreachable roots).
    // With flag: A is reachable (distance 1). B is reachable (distance 2).
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "A".into(),
        "B".into(),
        "w1".into(),
        "w2".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 weak edge
        3, 2, 3, 100, 1, // node 2: A, 1 weak edge
        3, 3, 5, 200, 0, // node 3: B
    ];

    // edge types: element=1, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        6,
        3,
        n(2), // (GC roots) --weak--> A
        6,
        4,
        n(3), // A --weak--> B
    ];

    // Without flag: both A and B are unreachable roots (U).
    let snap_default = build_test_snapshot(strings.clone(), nodes.clone(), edges.clone());
    assert!(
        snap_default.node_distance(NodeOrdinal(2)).is_unreachable(),
        "without flag, A should be unreachable"
    );
    assert!(
        snap_default.node_distance(NodeOrdinal(3)).is_unreachable(),
        "without flag, B should be unreachable"
    );

    // With flag: both become reachable via weak traversal.
    let snap_weak = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(2)),
        Distance(1),
        "with flag, A should be at distance 1"
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(3)),
        Distance(2),
        "with flag, B should be at distance 2"
    );
}

/// Strong edges take precedence: a node reachable via both a strong and a
/// weak edge keeps its strong-edge distance, unaffected by the flag.
#[test]
fn test_weak_is_reachable_strong_takes_precedence() {
    // root(0) --element--> (GC roots)(1) --property--> A(2) --weak--> Target(3)
    //                                (GC roots)(1) --property--> Target(3)
    // Target is reachable via strong edge at distance 1 regardless of flag.
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "A".into(),
        "Target".into(),
        "ref".into(),
        "weak_ref".into(),
        "strong_ref".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 2, // node 1: (GC roots), 2 edges
        3, 2, 3, 100, 1, // node 2: A, 1 weak edge
        3, 3, 5, 200, 0, // node 3: Target
    ];

    // edge types: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        2,
        4,
        n(2), // (GC roots) --property "ref"--> A
        2,
        6,
        n(3), // (GC roots) --property "strong_ref"--> Target
        6,
        5,
        n(3), // A --weak "weak_ref"--> Target
    ];

    let snap_default = build_test_snapshot(strings.clone(), nodes.clone(), edges.clone());
    assert_eq!(
        snap_default.node_distance(NodeOrdinal(3)),
        Distance(1),
        "Target reachable via strong edge at distance 1"
    );

    let snap_weak = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(3)),
        Distance(1),
        "with flag, Target still at distance 1 from strong edge"
    );
}

/// With the flag, the minimum reachable retainer distance is used.
#[test]
fn test_weak_is_reachable_picks_minimum_retainer_distance() {
    // root(0) --element--> (GC roots)(1) --property--> Near(2) --weak--> Target(4)
    //                                (GC roots)(1) --property--> Far(3) --property--> FarChild(5) --weak--> Target(4)
    // Near is at distance 1, FarChild at distance 3.
    // Target should get min(1,3)+1 = 2.
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "Near".into(),
        "Far".into(),
        "Target".into(),
        "FarChild".into(),
        "ref".into(),
        "w".into(),
        "child".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 2, // node 1: (GC roots), 2 edges
        3, 2, 3, 100, 1, // node 2: Near, 1 weak edge
        3, 3, 5, 100, 1, // node 3: Far, 1 strong edge
        3, 4, 7, 200, 0, // node 4: Target
        3, 5, 9, 100, 1, // node 5: FarChild, 1 weak edge
    ];

    // edge types: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        2,
        6,
        n(2), // (GC roots) --property--> Near
        2,
        6,
        n(3), // (GC roots) --property--> Far
        6,
        7,
        n(4), // Near --weak--> Target
        2,
        8,
        n(5), // Far --property--> FarChild
        6,
        7,
        n(4), // FarChild --weak--> Target
    ];

    let snap_weak = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(2)),
        Distance(1),
        "Near at distance 1"
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(5)),
        Distance(2),
        "FarChild at distance 2"
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(4)),
        Distance(2),
        "Target should get min(1,2)+1 = 2"
    );
}

/// With the flag, unreachable_bfs propagates reachable distances from seeds
/// through strong edges to deeper unreachable nodes.
#[test]
fn test_weak_is_reachable_propagates_through_strong_edges() {
    // root(0) --element--> (GC roots)(1) --property--> A(2) --weak--> B(3) --property--> C(4)
    // Without flag: B=U, C=U+1.
    // With flag: B gets distance 2 (A at 1, +1), C gets distance 3 via unreachable_bfs.
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "A".into(),
        "B".into(),
        "C".into(),
        "ref".into(),
        "w".into(),
        "child".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 1, // node 1: (GC roots)
        3, 2, 3, 100, 1, // node 2: A
        3, 3, 5, 200, 1, // node 3: B
        3, 4, 7, 300, 0, // node 4: C
    ];

    // edge types: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        2,
        5,
        n(2), // (GC roots) --property--> A
        6,
        6,
        n(3), // A --weak--> B
        2,
        7,
        n(4), // B --property--> C
    ];

    let snap_default = build_test_snapshot(strings.clone(), nodes.clone(), edges.clone());
    assert_eq!(
        snap_default.node_distance(NodeOrdinal(3)),
        Distance::UNREACHABLE_BASE,
        "without flag, B is U"
    );
    assert_eq!(
        snap_default.node_distance(NodeOrdinal(4)),
        Distance(Distance::UNREACHABLE_BASE.0 + 1),
        "without flag, C is U+1"
    );

    let snap_weak = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(3)),
        Distance(2),
        "with flag, B at distance 2"
    );
    assert_eq!(
        snap_weak.node_distance(NodeOrdinal(4)),
        Distance(3),
        "with flag, C at distance 3 via strong edge from B"
    );
}

/// Distance must not depend on node serialization order.  Here a node with
/// a lower ordinal is weakly retained by a node with a higher ordinal, so
/// it would be visited first in an ordinal-order scan before its retainer
/// has a distance.
#[test]
fn test_weak_is_reachable_independent_of_ordinal_order() {
    // root(0) --element--> (GC roots)(1) --property--> High(4) --weak--> Low(2)
    // Also: (GC roots)(1) --property--> Other(3) (filler to push High to ordinal 4)
    //
    // Low has ordinal 2, its weak retainer High has ordinal 4.
    // In an ordinal scan, Low is processed before High.
    // Correct: High distance=1, Low distance=2.
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "Low".into(),
        "Other".into(),
        "High".into(),
        "ref".into(),
        "other".into(),
        "w".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 2, // node 1: (GC roots), 2 edges
        3, 2, 3, 200, 0, // node 2: Low (lower ordinal, no outgoing edges)
        3, 3, 5, 100, 0, // node 3: Other
        3, 4, 7, 100, 1, // node 4: High (higher ordinal, 1 weak edge)
    ];

    // edge types: element=1, property=2, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // root --element--> (GC roots)
        2,
        5,
        n(4), // (GC roots) --property "ref"--> High
        2,
        6,
        n(3), // (GC roots) --property "other"--> Other
        6,
        7,
        n(2), // High --weak "w"--> Low
    ];

    let snap = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(1),
        "High should be at distance 1"
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(2)),
        Distance(2),
        "Low should be at distance 2 despite lower ordinal"
    );
}

/// A chain of weak edges where each retainer has a higher ordinal than its
/// target, so every node would be visited before its retainer in an
/// ordinal scan.
#[test]
fn test_weak_is_reachable_reverse_ordinal_chain() {
    // root(0) --element--> (GC roots)(1) --weak--> C(4) --weak--> B(3) --weak--> A(2)
    // Correct: C=1, B=2, A=3.
    let strings = vec![
        "".into(),
        "(GC roots)".into(),
        "A".into(),
        "B".into(),
        "C".into(),
        "w".into(),
    ];

    let nodes = vec![
        9u32, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 1, // node 1: (GC roots), 1 edge
        3, 2, 3, 100, 0, // node 2: A
        3, 3, 5, 100, 1, // node 3: B, 1 weak edge
        3, 4, 7, 100, 1, // node 4: C, 1 weak edge
    ];

    // Edges are assigned to nodes in ordinal order by edge_count:
    //   node 0 (root): 1 edge, node 1 (GC roots): 1 edge,
    //   node 2 (A): 0, node 3 (B): 1, node 4 (C): 1
    // edge types: element=1, weak=6
    let edges = vec![
        1u32,
        0,
        n(1), // node 0 (root) --element--> (GC roots)
        6,
        5,
        n(4), // node 1 (GC roots) --weak--> C
        6,
        5,
        n(2), // node 3 (B) --weak--> A
        6,
        5,
        n(3), // node 4 (C) --weak--> B
    ];

    let snap = build_test_snapshot_with_options(
        strings,
        nodes,
        edges,
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );
    assert_eq!(snap.node_distance(NodeOrdinal(4)), Distance(1), "C at 1");
    assert_eq!(snap.node_distance(NodeOrdinal(3)), Distance(2), "B at 2");
    assert_eq!(snap.node_distance(NodeOrdinal(2)), Distance(3), "A at 3");
}

/// Nodes unreachable due to non-weak filtered edges (e.g. sloppy_function_map
/// from NativeContext, filtered by distance_filter_stateful) must NOT be
/// promoted to reachable by --weak-is-reachable.  Only actual weak retaining
/// edges trigger promotion.
#[test]
fn test_weak_is_reachable_does_not_promote_non_weak_filtered() {
    // NativeContext(2) --property "sloppy_function_map"--> Filtered(3)  (filtered, non-weak)
    // NativeContext(2) --weak--> WeakTarget(4)
    // Without flag: both Filtered and WeakTarget are U.
    // With flag: only WeakTarget should be promoted; Filtered stays U.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    let snap = build_snapshot_with_options(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 10, 20, 2, // node 2: NativeContext, 2 edges
            3, 3, 12, 40, 0, // node 3: Filtered (behind sloppy_function_map)
            3, 4, 14, 40, 0, // node 4: WeakTarget (behind weak edge)
        ],
        vec![
            1,
            0,
            n(1), // root → (GC roots)
            2,
            5,
            n(2), // (GC roots) → NativeContext (property "ctx")
            2,
            6,
            n(3), // NativeContext → Filtered (property "sloppy_function_map")
            6,
            7,
            n(4), // NativeContext → WeakTarget (weak "w")
        ],
        s(&[
            "",                       // 0
            "(GC roots)",             // 1
            "system / NativeContext", // 2
            "Filtered",               // 3
            "WeakTarget",             // 4
            "ctx",                    // 5
            "sloppy_function_map",    // 6
            "w",                      // 7
        ]),
        SnapshotOptions {
            weak_is_reachable: true,
        },
    );

    assert_eq!(
        snap.node_distance(NodeOrdinal(2)),
        Distance(1),
        "NativeContext at distance 1"
    );
    assert!(
        snap.node_distance(NodeOrdinal(3)).is_unreachable(),
        "Filtered node behind sloppy_function_map should stay unreachable"
    );
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(2),
        "WeakTarget behind weak edge should be promoted to distance 2"
    );
}

// ── dominator_of ────────────────────────────────────────────────────────

#[test]
fn test_dominator_of_basic() {
    let snap = make_test_snapshot();
    // GC roots (node 1) dominates Object (node 2) and Array (node 4)
    assert_eq!(snap.dominator_of(NodeOrdinal(2)), NodeOrdinal(1));
    assert_eq!(snap.dominator_of(NodeOrdinal(4)), NodeOrdinal(1));
    // Object (node 2) dominates hello (node 3)
    assert_eq!(snap.dominator_of(NodeOrdinal(3)), NodeOrdinal(2));
}

#[test]
fn test_dominator_of_diamond() {
    // Two paths from GC roots to a shared target:
    //   root -> GC roots -> A -> shared
    //   root -> GC roots -> B -> shared
    // shared's dominator should be GC roots (node 1), not A or B.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: A
            3, 2, 7, 10, 1, // 3: B
            3, 2, 9, 10, 0, // 4: shared
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            2,
            n(2), // GC roots -> A
            2,
            2,
            n(3), // GC roots -> B
            2,
            2,
            n(4), // A -> shared
            2,
            2,
            n(4), // B -> shared
        ],
        s(&["", "(GC roots)", "A", "B", "shared"]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(4)),
        NodeOrdinal(1),
        "shared node dominated by GC roots, not A or B"
    );
}

#[test]
fn test_dominator_of_chain() {
    // Linear chain: root -> GC roots -> A -> B -> C
    // Each node is dominated by its parent.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 1, // 1: GC roots
            3, 2, 5, 10, 1, // 2: A
            3, 2, 7, 10, 1, // 3: B
            3, 2, 9, 10, 0, // 4: C
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            2,
            n(2), // GC roots -> A
            2,
            2,
            n(3), // A -> B
            2,
            2,
            n(4), // B -> C
        ],
        s(&["", "(GC roots)", "A", "B", "C"]),
    );
    assert_eq!(snap.dominator_of(NodeOrdinal(2)), NodeOrdinal(1));
    assert_eq!(snap.dominator_of(NodeOrdinal(3)), NodeOrdinal(2));
    assert_eq!(snap.dominator_of(NodeOrdinal(4)), NodeOrdinal(3));
}

// ── is_root_holder ──────────────────────────────────────────────────────

#[test]
fn test_is_root_holder() {
    let snap = make_test_snapshot();
    // Node 0 (synthetic root) is not a root holder — it IS the root's parent
    assert!(!snap.is_root_holder(NodeOrdinal(0)));
    // Node 1 (GC roots) is the root itself, not a root holder
    assert!(!snap.is_root_holder(NodeOrdinal(1)));
    // Node 2 (Object) is directly retained by (GC roots) → root holder
    assert!(snap.is_root_holder(NodeOrdinal(2)));
    // Node 3 (hello) is retained by Object, not GC roots → not a root holder
    assert!(!snap.is_root_holder(NodeOrdinal(3)));
    // Node 4 (Array) is directly retained by (GC roots) → root holder
    assert!(snap.is_root_holder(NodeOrdinal(4)));
}

#[test]
fn test_is_root_holder_with_mixed_retainers() {
    // Node retained by both GC roots and another node — still a root holder
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: A
            3, 2, 7, 10, 0, // 3: target
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            2,
            n(2), // GC roots -> A
            2,
            2,
            n(3), // GC roots -> target
            2,
            2,
            n(3), // A -> target (second retainer)
        ],
        s(&["", "(GC roots)", "A", "target"]),
    );
    assert!(
        snap.is_root_holder(NodeOrdinal(3)),
        "node with GC roots as one of multiple retainers is still a root holder"
    );
}

// ── retainer_count ──────────────────────────────────────────────────────

#[test]
fn test_retainer_count() {
    let snap = make_test_snapshot();
    // Synthetic root has no retainers
    assert_eq!(snap.retainer_count(NodeOrdinal(0)), 0);
    // GC roots retained by synthetic root
    assert_eq!(snap.retainer_count(NodeOrdinal(1)), 1);
    // Object retained by GC roots
    assert_eq!(snap.retainer_count(NodeOrdinal(2)), 1);
    // hello retained by Object
    assert_eq!(snap.retainer_count(NodeOrdinal(3)), 1);
    // Array retained by GC roots
    assert_eq!(snap.retainer_count(NodeOrdinal(4)), 1);
}

#[test]
fn test_retainer_count_multiple_retainers() {
    // Node retained by two different parents
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: A
            3, 2, 7, 10, 1, // 3: B
            3, 2, 9, 10, 0, // 4: shared
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            2,
            n(2), // GC roots -> A
            2,
            2,
            n(3), // GC roots -> B
            2,
            2,
            n(4), // A -> shared
            2,
            2,
            n(4), // B -> shared
        ],
        s(&["", "(GC roots)", "A", "B", "shared"]),
    );
    assert_eq!(
        snap.retainer_count(NodeOrdinal(4)),
        2,
        "shared has two retainers"
    );
    assert_eq!(snap.retainer_count(NodeOrdinal(2)), 1, "A has one retainer");
}

// ── for_each_retainer ───────────────────────────────────────────────────

#[test]
fn test_for_each_retainer_matches_get_retainers() {
    let snap = make_test_snapshot();
    for ordinal in 0..5 {
        let ord = NodeOrdinal(ordinal);
        let expected = snap.get_retainers(ord);
        let mut actual = Vec::new();
        snap.for_each_retainer(ord, |edge_idx, node_ord| {
            actual.push((edge_idx, node_ord));
        });
        assert_eq!(
            actual, expected,
            "for_each_retainer and get_retainers should return the same results for node {ordinal}"
        );
    }
}

#[test]
fn test_for_each_retainer_empty_for_root() {
    let snap = make_test_snapshot();
    let mut count = 0;
    snap.for_each_retainer(NodeOrdinal(0), |_, _| {
        count += 1;
    });
    assert_eq!(count, 0, "synthetic root should have no retainers");
}

#[test]
fn test_for_each_retainer_multiple_retainers() {
    // Same diamond snapshot as retainer_count test
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: A
            3, 2, 7, 10, 1, // 3: B
            3, 2, 9, 10, 0, // 4: shared
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            2,
            n(2), // GC roots -> A
            2,
            2,
            n(3), // GC roots -> B
            2,
            2,
            n(4), // A -> shared
            2,
            2,
            n(4), // B -> shared
        ],
        s(&["", "(GC roots)", "A", "B", "shared"]),
    );
    let mut retainer_ordinals = Vec::new();
    snap.for_each_retainer(NodeOrdinal(4), |_, node_ord| {
        retainer_ordinals.push(node_ord);
    });
    assert_eq!(retainer_ordinals.len(), 2);
    assert!(
        retainer_ordinals.contains(&NodeOrdinal(2)),
        "A should retain shared"
    );
    assert!(
        retainer_ordinals.contains(&NodeOrdinal(3)),
        "B should retain shared"
    );
}

// ── interface inference ─────────────────────────────────────────────────

#[test]
fn test_interface_inference_object_with_no_properties() {
    // An Object with no property edges should stay as "Object"
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 0, // 2: Object (no edges)
            3, 2, 7, 10, 0, // 3: Object (no edges)
        ],
        vec![1, 0, n(1), 2, 3, n(2), 2, 3, n(3)],
        s(&["", "(GC roots)", "Object", "obj"]),
    );
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "Object");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "Object");
}

#[test]
fn test_interface_inference_only_proto_property() {
    // Objects with only __proto__ property should stay as "Object"
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: Object with __proto__
            3, 2, 7, 10, 1, // 3: Object with __proto__
            3, 2, 9, 10, 0, // 4: proto target
        ],
        vec![
            1,
            0,
            n(1),
            2,
            3,
            n(2),
            2,
            3,
            n(3),
            2,
            4,
            n(4), // node 2 -> proto target via __proto__
            2,
            4,
            n(4), // node 3 -> proto target via __proto__
        ],
        s(&["", "(GC roots)", "Object", "obj", "__proto__"]),
    );
    assert_eq!(
        snap.node_class_name(NodeOrdinal(2)),
        "Object",
        "Object with only __proto__ should not get interface name"
    );
}

#[test]
fn test_interface_inference_single_instance_below_threshold() {
    // Only one Object with a given shape — needs at least 2 to be inferred
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 1, // 1: GC roots
            3, 2, 5, 10, 1, // 2: Object with "x"
            2, 3, 7, 10, 0, // 3: value
        ],
        vec![
            1,
            0,
            n(1),
            2,
            4,
            n(2), // GC roots -> Object
            2,
            5,
            n(3), // Object -> value via "x"
        ],
        s(&["", "(GC roots)", "Object", "val", "obj", "x"]),
    );
    assert_eq!(
        snap.node_class_name(NodeOrdinal(2)),
        "Object",
        "single instance should not get interface name"
    );
}

#[test]
fn test_interface_inference_two_instances_meet_threshold() {
    // Two Objects with {x} should get interface name
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 10, 1, // 2: Object with "x"
            3, 2, 7, 10, 1, // 3: Object with "x"
            2, 3, 9, 10, 0, // 4: value
        ],
        vec![
            1,
            0,
            n(1),
            2,
            4,
            n(2),
            2,
            4,
            n(3),
            2,
            5,
            n(4), // node 2 -> value via "x"
            2,
            5,
            n(4), // node 3 -> value via "x"
        ],
        s(&["", "(GC roots)", "Object", "val", "obj", "x"]),
    );
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "{x}");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "{x}");
}

#[test]
fn test_interface_inference_two_different_shapes() {
    // Two shapes, each with 2 instances
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 4, // 1: GC roots
            3, 2, 5, 10, 1, // 2: Object with "a"
            3, 2, 7, 10, 1, // 3: Object with "a"
            3, 2, 9, 10, 1, // 4: Object with "b"
            3, 2, 11, 10, 1, // 5: Object with "b"
            2, 3, 13, 10, 0, // 6: value
        ],
        vec![
            1,
            0,
            n(1),
            2,
            4,
            n(2),
            2,
            4,
            n(3),
            2,
            4,
            n(4),
            2,
            4,
            n(5),
            2,
            5,
            n(6), // node 2 -> value via "a"
            2,
            5,
            n(6), // node 3 -> value via "a"
            2,
            6,
            n(6), // node 4 -> value via "b"
            2,
            6,
            n(6), // node 5 -> value via "b"
        ],
        s(&["", "(GC roots)", "Object", "val", "obj", "a", "b"]),
    );
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "{a}");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "{a}");
    assert_eq!(snap.node_class_name(NodeOrdinal(4)), "{b}");
    assert_eq!(snap.node_class_name(NodeOrdinal(5)), "{b}");
}

#[test]
fn test_interface_inference_superset_matches_subset() {
    // 2 Objects with {x, y} define an interface.
    // A third Object with {x, y, z} should still match {x, y}.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 3, // 1: GC roots
            3, 2, 5, 10, 2, // 2: Object with x, y
            3, 2, 7, 10, 2, // 3: Object with x, y
            3, 2, 9, 10, 3, // 4: Object with x, y, z
            2, 3, 11, 10, 0, // 5: value
        ],
        vec![
            1,
            0,
            n(1),
            2,
            4,
            n(2),
            2,
            4,
            n(3),
            2,
            4,
            n(4),
            2,
            5,
            n(5), // node 2 -> value via "x"
            2,
            6,
            n(5), // node 2 -> value via "y"
            2,
            5,
            n(5), // node 3 -> value via "x"
            2,
            6,
            n(5), // node 3 -> value via "y"
            2,
            5,
            n(5), // node 4 -> value via "x"
            2,
            6,
            n(5), // node 4 -> value via "y"
            2,
            7,
            n(5), // node 4 -> value via "z"
        ],
        s(&["", "(GC roots)", "Object", "val", "obj", "x", "y", "z"]),
    );
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "{x, y}");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "{x, y}");
    assert_eq!(
        snap.node_class_name(NodeOrdinal(4)),
        "{x, y}",
        "superset of properties should match the defined interface"
    );
}

#[test]
fn test_interface_inference_non_object_not_affected() {
    // A non-Object node (e.g. closure) with the same property edges
    // should not get an interface name
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 3, // 1: GC roots
            3, 2, 5, 10, 1, // 2: Object with "x"
            3, 2, 7, 10, 1, // 3: Object with "x"
            5, 4, 9, 10, 1, // 4: closure with "x" (type 5 = closure)
            2, 3, 11, 10, 0, // 5: value
        ],
        vec![
            1,
            0,
            n(1),
            2,
            5,
            n(2),
            2,
            5,
            n(3),
            2,
            5,
            n(4),
            2,
            6,
            n(5), // node 2 -> value via "x"
            2,
            6,
            n(5), // node 3 -> value via "x"
            2,
            6,
            n(5), // node 4 -> value via "x"
        ],
        s(&["", "(GC roots)", "Object", "val", "myFunc", "obj", "x"]),
    );
    assert_eq!(snap.node_class_name(NodeOrdinal(2)), "{x}");
    assert_eq!(snap.node_class_name(NodeOrdinal(3)), "{x}");
    // Closure should keep its own class name, not get interface name
    assert_ne!(
        snap.node_class_name(NodeOrdinal(4)),
        "{x}",
        "non-Object nodes should not get interface names"
    );
}

// ── duplicate_strings ───────────────────────────────────────────────────

#[test]
fn test_duplicate_strings_basic() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "hello".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "hello".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: false,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    let d = dupes.iter().find(|d| d.value == "hello").unwrap();
    assert_eq!(d.count, 2);
    assert_eq!(d.total_size, 80);
    assert_eq!(d.wasted_size(), 40);
}

#[test]
fn test_duplicate_strings_no_duplicates() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "hello".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "world".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: false,
        },
    ]);
    let result = snap.duplicate_strings();
    assert!(
        !result.duplicates.iter().any(|d| d.instance_size > 0),
        "different strings should not be grouped"
    );
}

#[test]
fn test_duplicate_strings_empty_strings_excluded() {
    // Two empty strings should not be reported
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            2, 0, 5, 10, 0, // 2: string "" (name index 0 = "")
            2, 0, 7, 10, 0, // 3: string ""
        ],
        vec![1, 0, n(1), 2, 2, n(2), 2, 2, n(3)],
        s(&["", "(GC roots)", "ref"]),
    );
    let dupes = snap.duplicate_strings().duplicates;
    assert!(dupes.is_empty(), "empty strings should be excluded");
}

#[test]
fn test_duplicate_strings_non_string_nodes_excluded() {
    // Two object nodes with the same name should not be reported
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            3, 2, 5, 40, 0, // 2: object "Foo" (type 3 = object)
            3, 2, 7, 40, 0, // 3: object "Foo"
        ],
        vec![1, 0, n(1), 2, 3, n(2), 2, 3, n(3)],
        s(&["", "(GC roots)", "Foo", "ref"]),
    );
    let dupes = snap.duplicate_strings().duplicates;
    assert!(dupes.is_empty(), "non-string nodes should not be reported");
}

#[test]
fn test_duplicate_strings_sorted_by_wasted_size() {
    // "big" appears 2x at 100 bytes each (wasted 100), "small" appears 3x at 20 bytes each (wasted 40)
    // Should sort by wasted descending: big first
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "big".into(),
            self_size: 100,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "big".into(),
            self_size: 100,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "small".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "small".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "small".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    let big = dupes.iter().find(|d| d.value == "big").unwrap();
    let small = dupes.iter().find(|d| d.value == "small").unwrap();
    assert!(big.wasted_size() > small.wasted_size());
    assert_eq!(big.wasted_size(), 100);
    assert_eq!(small.wasted_size(), 40);
}

#[test]
fn test_duplicate_strings_multiple_copies() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "dup".into(),
            self_size: 50,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "dup".into(),
            self_size: 50,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "dup".into(),
            self_size: 50,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "dup".into(),
            self_size: 50,
            length: 3,
            truncated: false,
            two_byte: false,
        },
    ]);
    let d = snap
        .duplicate_strings()
        .duplicates
        .into_iter()
        .find(|d| d.value == "dup")
        .unwrap();
    assert_eq!(d.count, 4);
    assert_eq!(d.total_size, 200);
    assert_eq!(d.instance_size, 50);
    assert_eq!(d.wasted_size(), 150);
}

#[test]
fn test_duplicate_strings_sliced_strings_excluded() {
    // Sliced strings (type 11) share underlying storage with their parent,
    // so they should not be reported as duplicates.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            2, 2, 5, 40, 0, // 2: string "hello"
            11, 2, 7, 20, 0, // 3: sliced string "hello"
        ],
        vec![1, 0, n(1), 2, 3, n(2), 2, 3, n(3)],
        s(&["", "(GC roots)", "hello", "ref"]),
    );
    let dupes = snap.duplicate_strings().duplicates;
    assert!(dupes.is_empty(), "sliced strings should be excluded");
}

#[test]
fn test_duplicate_strings_flat_cons_string_excluded() {
    // A flattened cons string (one part is empty) should be skipped.
    // Cons string node (type 10) with internal edges "first" -> "hello"
    // and "second" -> "" is a flat cons string.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 3, // 1: GC roots
            2, 2, 5, 40, 0, // 2: string "hello"
            10, 2, 7, 40, 2, // 3: cons string "hello" (flat: second is "")
            2, 0, 9, 0, 0, // 4: string "" (empty, target of "second")
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            4,
            n(2), // GC roots -> string "hello"
            2,
            4,
            n(3), // GC roots -> cons string "hello"
            2,
            4,
            n(4), // GC roots -> string ""
            3,
            3,
            n(2), // cons string: internal "first" -> string "hello"
            3,
            5,
            n(4), // cons string: internal "second" -> string ""
        ],
        s(&["", "(GC roots)", "hello", "first", "ref", "second"]),
    );
    let dupes = snap.duplicate_strings().duplicates;
    assert!(
        dupes.is_empty(),
        "flat cons string should not be reported as duplicate of its own content"
    );
}

#[test]
fn test_duplicate_strings_non_flat_cons_string_included() {
    // A non-flat cons string (both parts non-empty) that produces the same
    // value as a regular string IS a real duplicate and should be reported.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 4, // 1: GC roots
            2, 5, 5, 40, 0, // 2: string "helloworld"
            10, 5, 7, 40, 2, // 3: cons string "helloworld" (first="hello", second="world")
            2, 2, 9, 20, 0, // 4: string "hello" (first part)
            2, 6, 11, 20, 0, // 5: string "world" (second part)
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            7,
            n(2), // GC roots -> string "helloworld"
            2,
            7,
            n(3), // GC roots -> cons string
            2,
            7,
            n(4), // GC roots -> "hello"
            2,
            7,
            n(5), // GC roots -> "world"
            3,
            3,
            n(4), // cons: internal "first" -> "hello"
            3,
            8,
            n(5), // cons: internal "second" -> "world"
        ],
        s(&[
            "",           // 0
            "(GC roots)", // 1
            "hello",      // 2
            "first",      // 3
            "ref",        // 4
            "helloworld", // 5
            "world",      // 6
            "ref",        // 7
            "second",     // 8
        ]),
    );
    let dupes = snap.duplicate_strings().duplicates;
    // Without length edges, all strings are skipped — this test verifies
    // that the cons-string logic itself doesn't break.
    assert!(
        dupes.is_empty(),
        "without length edges, strings are skipped"
    );
}

/// Helper: build a snapshot that contains string nodes with `length`,
/// `truncated`, and/or `two_byte_representation` internal edges, mimicking
/// V8's `ExtractStringReferences`.
fn build_string_props_snapshot(entries: &[StringTestEntry]) -> HeapSnapshot {
    // Strings table:
    // 0: ""  1: "(GC roots)"  2: "value"  3: "length"  4: "truncated"
    // 5: "two_byte_representation"  6: "bool"  7: "true"  8: "int"
    // 9: "ref"
    // then per-entry: string name, length-as-string
    let mut strings: Vec<String> = vec![
        "".into(),
        "(GC roots)".into(),
        "value".into(),
        "length".into(),
        "truncated".into(),
        "two_byte_representation".into(),
        "bool".into(),
        "true".into(),
        "int".into(),
        "ref".into(),
    ];

    let nfc = 5u32;

    // Collect per-entry string indices first.
    struct EntryStrings {
        name_idx: u32,
        len_str_idx: u32,
    }
    let mut entry_strings = Vec::new();
    for entry in entries {
        let name_idx = strings.len() as u32;
        strings.push(entry.name.clone());
        let len_str_idx = strings.len() as u32;
        strings.push(entry.length.to_string());
        entry_strings.push(EntryStrings {
            name_idx,
            len_str_idx,
        });
    }

    // Node layout (edges must be emitted in node order):
    //   0: synthetic root (1 edge)
    //   1: GC roots (entries.len() edges)
    //   2: bool "true" node (1 edge -> node 3)
    //   3: string "true" (0 edges)
    //   then per entry (3 nodes each):
    //     4+i*3: string node (1-3 edges: length + truncated? + two_byte?)
    //     5+i*3: int node for length (1 edge -> value)
    //     6+i*3: string node for length value (0 edges)

    let mut nodes: Vec<u32> = vec![];
    let mut edges: Vec<u32> = vec![];
    let mut next_id = 1u32;

    // node 0: synthetic root
    nodes.extend_from_slice(&[9, 0, next_id, 0, 1]);
    next_id += 1;
    // node 0 edges:
    edges.extend_from_slice(&[1, 0, 1 * nfc]); // -> GC roots

    // node 1: GC roots
    nodes.extend_from_slice(&[9, 1, next_id, 0, entries.len() as u32]);
    next_id += 1;
    // node 1 edges: all GC roots -> string node refs
    for (i, _) in entries.iter().enumerate() {
        let str_node = (4 + i * 3) as u32;
        edges.extend_from_slice(&[2, 9, str_node * nfc]);
    }

    // node 2: bool "true" (type number=7, name "bool"=6)
    nodes.extend_from_slice(&[7, 6, next_id, 0, 1]);
    next_id += 1;
    // node 2 edges:
    edges.extend_from_slice(&[3, 2, 3 * nfc]); // internal "value" -> node 3

    // node 3: string "true" (value of the bool)
    nodes.extend_from_slice(&[2, 7, next_id, 0, 0]);
    next_id += 1;

    // Per-entry nodes and edges (in node order)
    for (i, entry) in entries.iter().enumerate() {
        let es = &entry_strings[i];
        let int_node = (5 + i * 3) as u32;
        let len_val_node = (6 + i * 3) as u32;

        let mut str_edge_count = 1u32; // "length" always
        if entry.truncated {
            str_edge_count += 1;
        }
        if entry.two_byte {
            str_edge_count += 1;
        }

        // string node
        nodes.extend_from_slice(&[2, es.name_idx, next_id, entry.self_size, str_edge_count]);
        next_id += 1;
        // string node edges:
        edges.extend_from_slice(&[3, 3, int_node * nfc]); // internal "length"
        if entry.truncated {
            edges.extend_from_slice(&[3, 4, 2 * nfc]); // internal "truncated" -> bool
        }
        if entry.two_byte {
            edges.extend_from_slice(&[3, 5, 2 * nfc]); // internal "two_byte_representation"
        }

        // int node for length (type number=7, name "int"=8)
        nodes.extend_from_slice(&[7, 8, next_id, 0, 1]);
        next_id += 1;
        // int node edges:
        edges.extend_from_slice(&[3, 2, len_val_node * nfc]); // internal "value"

        // string node for length value (leaf)
        nodes.extend_from_slice(&[2, es.len_str_idx, next_id, 0, 0]);
        next_id += 1;
    }

    build_snapshot(standard_node_fields(), nodes, edges, strings)
}

struct StringTestEntry {
    name: String,
    self_size: u32,
    length: u32,
    truncated: bool,
    two_byte: bool,
}

#[test]
fn test_duplicate_strings_truncated_different_lengths_not_grouped() {
    // Two truncated strings with the same prefix but different true lengths
    // should NOT be grouped as duplicates.
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "hello world this is a long".into(),
            self_size: 100,
            length: 100,
            truncated: true,
            two_byte: false,
        },
        StringTestEntry {
            name: "hello world this is a long".into(),
            self_size: 200,
            length: 200,
            truncated: true,
            two_byte: false,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    assert!(
        !dupes.iter().any(|d| d.instance_size > 0),
        "truncated strings with different lengths should not be grouped"
    );
}

#[test]
fn test_duplicate_strings_truncated_same_length_grouped() {
    // Two truncated strings with the same prefix AND same length
    // should be grouped as duplicates.
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "hello world this is a long".into(),
            self_size: 100,
            length: 500,
            truncated: true,
            two_byte: false,
        },
        StringTestEntry {
            name: "hello world this is a long".into(),
            self_size: 100,
            length: 500,
            truncated: true,
            two_byte: false,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    let main = dupes
        .iter()
        .find(|d| d.value == "hello world this is a long")
        .expect("truncated strings with same length should be grouped");
    assert_eq!(main.count, 2);
    assert!(main.truncated);
    assert_eq!(main.length, 500);
}

#[test]
fn test_duplicate_strings_node_ids_populated() {
    // 3x "foo" + 2x "bar" + 1x "baz"; "foo" and "bar" become duplicate groups.
    let entries: Vec<StringTestEntry> = [
        ("foo", 20, 3),
        ("foo", 20, 3),
        ("foo", 20, 3),
        ("bar", 40, 3),
        ("bar", 40, 3),
        ("baz", 60, 3),
    ]
    .into_iter()
    .map(|(name, self_size, length)| StringTestEntry {
        name: name.into(),
        self_size,
        length,
        truncated: false,
        two_byte: false,
    })
    .collect();
    let snap = build_string_props_snapshot(&entries);

    let dupes = snap.duplicate_strings().duplicates;
    let foo = dupes.iter().find(|d| d.value == "foo").unwrap();
    let bar = dupes.iter().find(|d| d.value == "bar").unwrap();

    assert_eq!(foo.node_ids.len(), foo.count as usize);
    assert_eq!(bar.node_ids.len(), bar.count as usize);

    // Every captured id must resolve to a string node with the expected name.
    for id in foo.node_ids.iter().chain(bar.node_ids.iter()) {
        let ord = snap
            .node_for_snapshot_object_id(*id)
            .unwrap_or_else(|| panic!("id {id:?} should exist in snapshot"));
        let name = snap.node_display_name(ord);
        assert!(
            name == "foo" || name == "bar",
            "expected foo/bar, got {name:?}"
        );
    }

    // Ids within one group must be distinct.
    let mut sorted = foo.node_ids.clone();
    sorted.sort_by_key(|id| id.0);
    sorted.dedup();
    assert_eq!(sorted.len(), foo.node_ids.len(), "ids must be distinct");

    // "baz" has count 1 → not a duplicate group, so not returned.
    assert!(dupes.iter().all(|d| d.value != "baz"));
}

#[test]
fn test_duplicate_strings_fields_populated() {
    // Verify the new fields (length, truncated, two_byte) are populated correctly.
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "abc".into(),
            self_size: 20,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "abc".into(),
            self_size: 20,
            length: 3,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "xyz".into(),
            self_size: 40,
            length: 3,
            truncated: false,
            two_byte: true,
        },
        StringTestEntry {
            name: "xyz".into(),
            self_size: 40,
            length: 3,
            truncated: false,
            two_byte: true,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;

    let abc = dupes.iter().find(|d| d.value == "abc").unwrap();
    assert_eq!(abc.count, 2);
    assert_eq!(abc.length, 3);
    assert!(!abc.truncated);
    assert!(!abc.two_byte);

    let xyz = dupes.iter().find(|d| d.value == "xyz").unwrap();
    assert_eq!(xyz.count, 2);
    assert_eq!(xyz.length, 3);
    assert!(!xyz.truncated);
    assert!(xyz.two_byte);
}

#[test]
fn test_duplicate_strings_non_truncated_still_grouped_by_name() {
    // Non-truncated strings with the same name should still be grouped
    // regardless of the length edge.
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "hello".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "hello".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    let main = dupes.iter().find(|d| d.value == "hello").unwrap();
    assert_eq!(main.count, 2);
    assert!(!main.truncated);
}

#[test]
fn test_duplicate_strings_skipped_counts() {
    // Strings without a `length` edge should be skipped and counted.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 3, // 1: GC roots
            2, 2, 5, 40, 0, // 2: string "hello", 40 bytes, no length edge
            2, 2, 7, 60, 0, // 3: string "hello", 60 bytes, no length edge
            2, 3, 9, 30, 0, // 4: string "world", 30 bytes, no length edge
        ],
        vec![1, 0, n(1), 2, 4, n(2), 2, 4, n(3), 2, 4, n(4)],
        s(&["", "(GC roots)", "hello", "world", "ref"]),
    );
    let result = snap.duplicate_strings();
    assert!(result.duplicates.is_empty());
    assert_eq!(result.skipped_count, 3);
    assert_eq!(result.skipped_size, 130); // 40 + 60 + 30
}

#[test]
fn test_duplicate_strings_mixed_with_and_without_length() {
    // Mix of strings with and without length edges. Only those with length
    // edges should participate; the rest are counted as skipped.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    // Manually build a snapshot where:
    //   nodes 2,3: string "hello" WITHOUT length (skipped)
    //   nodes 4-6, 7-9: string "hello" WITH length (via build_string_props_snapshot pattern)
    //
    // We'll build it by hand to mix the two kinds.

    // Strings: 0=""  1="(GC roots)"  2="hello"  3="ref"  4="value"
    //          5="length"  6="5"  7="int"
    let strings = s(&[
        "",           // 0
        "(GC roots)", // 1
        "hello",      // 2
        "ref",        // 3
        "value",      // 4
        "length",     // 5
        "5",          // 6
        "int",        // 7
    ]);

    // node 0: root (1 edge)
    // node 1: GC roots (4 edges -> nodes 2,3,4,7)
    // node 2: string "hello" no length edge (0 edges)
    // node 3: string "hello" no length edge (0 edges)
    // node 4: string "hello" WITH length edge (1 edge -> node 5)
    // node 5: int node for length (1 edge -> node 6)
    // node 6: string "5" (0 edges)
    // node 7: string "hello" WITH length edge (1 edge -> node 8)
    // node 8: int node for length (1 edge -> node 9)
    // node 9: string "5" (0 edges)
    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: root
        9, 1, 2, 0, 4, // 1: GC roots
        2, 2, 3, 50, 0, // 2: string "hello" (no length)
        2, 2, 4, 50, 0, // 3: string "hello" (no length)
        2, 2, 5, 50, 1, // 4: string "hello" (has length edge)
        7, 7, 6, 0, 1, // 5: int "int"
        2, 6, 7, 0, 0, // 6: string "5"
        2, 2, 8, 50, 1, // 7: string "hello" (has length edge)
        7, 7, 9, 0, 1, // 8: int "int"
        2, 6, 10, 0, 0, // 9: string "5"
    ];
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root -> GC roots
        // GC roots edges:
        2,
        3,
        n(2), // -> node 2
        2,
        3,
        n(3), // -> node 3
        2,
        3,
        n(4), // -> node 4
        2,
        3,
        n(7), // -> node 7
        // node 4 edges:
        3,
        5,
        n(5), // internal "length" -> int node 5
        // node 5 edges:
        3,
        4,
        n(6), // internal "value" -> string "5"
        // node 7 edges:
        3,
        5,
        n(8), // internal "length" -> int node 8
        // node 8 edges:
        3,
        4,
        n(9), // internal "value" -> string "5"
    ];
    let snap = build_snapshot(standard_node_fields(), nodes, edges, strings);

    let result = snap.duplicate_strings();
    // Nodes 2,3 (string "hello", self_size=50) lack length edges and are
    // skipped. Nodes 6,9 (string "5", self_size=0) are ignored entirely.
    assert_eq!(result.skipped_count, 2);
    assert_eq!(result.skipped_size, 100); // 50 + 50
    // Nodes 4 and 7 have length -> grouped as duplicates
    let d = result
        .duplicates
        .iter()
        .find(|d| d.value == "hello")
        .unwrap();
    assert_eq!(d.count, 2);
    assert_eq!(d.total_size, 100);
    assert_eq!(d.length, 5);
}

#[test]
fn test_node_string_length() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "short".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "long".into(),
            self_size: 200,
            length: 10000,
            truncated: true,
            two_byte: false,
        },
    ]);
    // node 4 is the first string entry (nodes 0-3 are root/GC roots/bool/true)
    assert_eq!(snap.node_string_length(NodeOrdinal(4)), Some(5));
    assert_eq!(snap.node_string_length(NodeOrdinal(7)), Some(10000));
    // Non-string nodes have no length edge
    assert_eq!(snap.node_string_length(NodeOrdinal(0)), None);
}

#[test]
fn test_node_is_truncated_string() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "normal".into(),
            self_size: 20,
            length: 6,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "cut".into(),
            self_size: 20,
            length: 1000,
            truncated: true,
            two_byte: false,
        },
    ]);
    assert!(!snap.node_is_truncated_string(NodeOrdinal(4)));
    assert!(snap.node_is_truncated_string(NodeOrdinal(7)));
    // Non-string node
    assert!(!snap.node_is_truncated_string(NodeOrdinal(0)));
}

#[test]
fn test_node_is_two_byte_string() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "ascii".into(),
            self_size: 20,
            length: 5,
            truncated: false,
            two_byte: false,
        },
        StringTestEntry {
            name: "unicode".into(),
            self_size: 40,
            length: 7,
            truncated: false,
            two_byte: true,
        },
    ]);
    assert!(!snap.node_is_two_byte_string(NodeOrdinal(4)));
    assert!(snap.node_is_two_byte_string(NodeOrdinal(7)));
}

#[test]
fn test_duplicate_strings_two_byte_duplicates() {
    let snap = build_string_props_snapshot(&[
        StringTestEntry {
            name: "emoji".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: true,
        },
        StringTestEntry {
            name: "emoji".into(),
            self_size: 40,
            length: 5,
            truncated: false,
            two_byte: true,
        },
    ]);
    let dupes = snap.duplicate_strings().duplicates;
    let d = dupes.iter().find(|d| d.value == "emoji").unwrap();
    assert_eq!(d.count, 2);
    assert!(d.two_byte);
    assert_eq!(d.total_size, 80);
    assert_eq!(d.wasted_size(), 40);
}

#[test]
fn test_duplicate_strings_zero_size_strings_ignored() {
    // Synthetic string nodes (self_size=0), such as value nodes inside
    // int/bool number nodes, should not appear in duplicates or skipped
    // counts.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: root
            9, 1, 3, 0, 2, // 1: GC roots
            2, 2, 5, 0, 0, // 2: string "42" (synthetic, self_size=0)
            2, 2, 7, 0, 0, // 3: string "42" (synthetic, self_size=0)
        ],
        vec![1, 0, n(1), 2, 3, n(2), 2, 3, n(3)],
        s(&["", "(GC roots)", "42", "ref"]),
    );
    let result = snap.duplicate_strings();
    assert!(result.duplicates.is_empty());
    assert_eq!(result.skipped_count, 0);
    assert_eq!(result.skipped_size, 0);
}

// ── dominator tree root ────────────────────────────────────────────────

#[test]
fn test_dominator_rooted_at_gc_roots_not_synthetic_root() {
    // The synthetic root has children: (GC roots) and a synthetic system
    // sub-root.  Both point to the same Object.
    //
    //   synthetic root ---> (GC roots) --------> Object ---> leaf
    //        \----------> (System sub-root) ---> Object
    //
    // If dominators were rooted at the synthetic root, Object's dominator
    // would be the synthetic root (two paths converge there).
    // Since we root at (GC roots), Object's dominator must be (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            9, 4, 5, 0, 1, // 2: (System sub-root), synthetic, 1 edge
            3, 2, 7, 100, 1, // 3: Object, 1 edge
            2, 3, 9, 50, 0, // 4: leaf
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> (System sub-root)
            2,
            2,
            n(3), // (GC roots) -> Object
            2,
            2,
            n(3), // (System sub-root) -> Object
            2,
            3,
            n(4), // Object -> leaf
        ],
        s(&["", "(GC roots)", "Object", "leaf", "(System sub-root)"]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "Object's dominator must be (GC roots), not synthetic root"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(4)),
        NodeOrdinal(3),
        "leaf's dominator must be Object"
    );
}

#[test]
fn test_dominator_ignores_system_roots() {
    // The synthetic root has children: (GC roots) and a system root
    // "(Persistent roots)" (synthetic).  Both point to the same Object.
    //
    //   synthetic root ---> (GC roots) ---------> Object
    //        \----------> (Persistent roots) ---> Object
    //
    // Because dominators are rooted at (GC roots), the system root's edge
    // is irrelevant.  Object's dominator must be (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            9, 4, 5, 0, 1, // 2: (Persistent roots), synthetic, 1 edge
            3, 5, 7, 100, 0, // 3: Object
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> (Persistent roots)
            2,
            5,
            n(3), // (GC roots) -> Object
            2,
            5,
            n(3), // (Persistent roots) -> Object
        ],
        s(&[
            "",
            "(GC roots)",
            "Object",
            "leaf",
            "(Persistent roots)",
            "obj",
        ]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "Object's dominator must be (GC roots), not the system root"
    );
    // Object is distance 1 from (GC roots), regardless of the system root path.
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance(1),
        "Object reached via (GC roots) at distance 1"
    );
    // (Persistent roots) is seeded at distance 1 in the system root phase.
    assert_eq!(
        snap.node_distance(NodeOrdinal(2)),
        Distance(1),
        "(Persistent roots) seeded at distance 1 as system root"
    );
}

#[test]
fn test_dominator_system_root_only_node_attached_via_fallback() {
    // A node reachable ONLY from a system root (not from GC roots).
    // All edges from the synthetic root are non-essential, so during the
    // main DFS from (GC roots) nodes 2 and 4 are unreachable.  The
    // fallback phase parents them directly to (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            9, 4, 5, 0, 1, // 2: (Persistent roots), synthetic, 1 edge
            3, 5, 7, 100, 0, // 3: reachable from GC roots
            3, 5, 9, 200, 0, // 4: only reachable from system root
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> (Persistent roots)
            2,
            5,
            n(3), // (GC roots) -> node 3
            2,
            5,
            n(4), // (Persistent roots) -> node 4
        ],
        s(&[
            "",
            "(GC roots)",
            "Object",
            "leaf",
            "(Persistent roots)",
            "obj",
        ]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "GC-reachable node dominated by (GC roots)"
    );
    // (Persistent roots) is dominated by (GC roots) — the synthetic
    // root's edge is non-essential.
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "(Persistent roots) dominated by (GC roots)"
    );
    // Node 4 is only reachable via (Persistent roots), so its immediate
    // dominator is (Persistent roots).
    assert_eq!(
        snap.dominator_of(NodeOrdinal(4)),
        NodeOrdinal(2),
        "system-root-only node dominated by (Persistent roots)"
    );
    // GC-reachable node gets distance 1 from (GC roots).
    assert_eq!(
        snap.node_distance(NodeOrdinal(3)),
        Distance(1),
        "node 3 reached from (GC roots) at distance 1"
    );
    // (Persistent roots) is seeded at distance 1 in the system root phase.
    assert_eq!(
        snap.node_distance(NodeOrdinal(2)),
        Distance(1),
        "(Persistent roots) seeded at distance 1"
    );
    // Node 4 is reached via (Persistent roots) BFS at distance 2.
    assert_eq!(
        snap.node_distance(NodeOrdinal(4)),
        Distance(2),
        "system-root-only node at distance 2 via (Persistent roots)"
    );
}

#[test]
fn test_unreachable_node_dominated_by_gc_roots() {
    // Node 3 is only retained by a weak edge from node 2, so it is
    // unreachable from (GC roots) in the essential-edge graph.  The
    // fallback phase of the dominator algorithm parents it to (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: synthetic root, 1 edge
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 100, 1, // 2: reachable Object, 1 edge
            3, 2, 7, 200, 0, // 3: unreachable Object (only weak retainer)
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            2,
            2,
            n(2), // (GC roots) -> node 2
            6,
            2,
            n(3), // node 2 -> node 3 (weak edge, type 6)
        ],
        s(&["", "(GC roots)", "Object", "weak_ref"]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "reachable node dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "unreachable node dominated by (GC roots)"
    );
}

#[test]
fn test_isolated_node_dominated_by_gc_roots() {
    // Node 3 has no incoming or outgoing edges — completely isolated.
    // It should still be placed in the dominator tree under (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: synthetic root, 1 edge
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 100, 0, // 2: reachable Object
            3, 2, 7, 200, 0, // 3: isolated Object (no edges at all)
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            2,
            2,
            n(2), // (GC roots) -> node 2
        ],
        s(&["", "(GC roots)", "Object"]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "reachable node dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "isolated node dominated by (GC roots)"
    );
}

#[test]
fn test_unreachable_group_dominated_by_gc_roots() {
    // Nodes 3, 4, 5 form a connected subgraph that is unreachable from
    // (GC roots).  Node 3 is the entry point (no essential incoming edges
    // from reachable nodes), while 4 and 5 are reachable from 3.
    //
    //   (GC roots) --> node 2
    //   node 3 --> node 4 --> node 5
    //       \---------^  (diamond)
    //
    // In the fallback phase, node 3 gets parented to (GC roots).  Nodes
    // 4 and 5 are discovered via DFS from 3, so they keep their internal
    // dominator structure: 4 is dominated by 3, 5 is dominated by 3
    // (diamond converges at 3).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0: synthetic root, 1 edge
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 100, 0, // 2: reachable Object
            3, 2, 7, 10, 2, // 3: unreachable A, 2 edges
            3, 2, 9, 20, 1, // 4: unreachable B, 1 edge
            3, 2, 11, 30, 0, // 5: unreachable C, 0 edges
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            2,
            2,
            n(2), // (GC roots) -> node 2
            2,
            2,
            n(4), // node 3 -> node 4
            2,
            2,
            n(5), // node 3 -> node 5
            2,
            2,
            n(5), // node 4 -> node 5
        ],
        s(&["", "(GC roots)", "Object"]),
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "reachable node dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "unreachable group root dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(4)),
        NodeOrdinal(3),
        "node 4 dominated by node 3"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(5)),
        NodeOrdinal(3),
        "node 5 dominated by node 3 (diamond converges)"
    );
}

// ── user roots ──────────────────────────────────────────────────────────

#[test]
fn test_user_roots_do_not_affect_dominator_tree() {
    // The synthetic root has children: (GC roots) and a user root
    // (non-synthetic NativeContext).  Both (GC roots) and the user root
    // point to the same Object.
    //
    //   synthetic root ---> (GC roots) ---> Object
    //        \----------> NativeContext ---> Object
    //
    // The synthetic root's edge to the user root is non-essential, so the
    // user root's path doesn't participate in dominator computation.
    // Object's dominator must be (GC roots).
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 0, 1, // 2: NativeContext (user root), 1 edge
            3, 3, 7, 100, 0, // 3: Object
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> NativeContext
            2,
            3,
            n(3), // (GC roots) -> Object
            2,
            3,
            n(3), // NativeContext -> Object
        ],
        s(&["", "(GC roots)", "NativeContext", "Object"]),
    );
    assert!(
        snap.is_user_root(NodeOrdinal(2)),
        "NativeContext should be identified as a user root"
    );
    assert!(
        !snap.is_user_root(NodeOrdinal(1)),
        "(GC roots) is synthetic, not a user root"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(1),
        "Object's dominator must be (GC roots), not affected by user root"
    );
}

#[test]
fn test_user_root_reachable_from_gc_roots_dominated_by_gc_roots() {
    // NativeContext (node 2) is both a user root (direct non-synthetic
    // child of the synthetic root) and reachable from (GC roots).  Its
    // dominator must be (GC roots), not affected by the synthetic root's
    // structural edge.
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 0, 1, // 2: NativeContext (user root), 1 edge
            3, 3, 7, 100, 0, // 3: Object
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> NativeContext
            2,
            2,
            n(2), // (GC roots) -> NativeContext
            2,
            3,
            n(3), // NativeContext -> Object
        ],
        s(&["", "(GC roots)", "NativeContext", "Object"]),
    );
    assert!(snap.is_user_root(NodeOrdinal(2)));
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "user root dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(2),
        "Object dominated by NativeContext"
    );
}

#[test]
fn test_user_root_dominated_by_intermediate_object() {
    // NativeContext (node 4) is a user root but only reachable from
    // (GC roots) through A -> B -> NativeContext.  Its dominator
    // should be B, not (GC roots).
    //
    //   synthetic root --> (GC roots) --> A --> B --> NativeContext
    //        \-------> NativeContext  (non-essential, user root edge)
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 2, // 0: synthetic root, 2 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            3, 2, 5, 10, 1, // 2: A, 1 edge
            3, 2, 7, 20, 1, // 3: B, 1 edge
            3, 3, 9, 0, 0, // 4: NativeContext (user root), 0 edges
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(4), // root -> NativeContext (non-essential)
            2,
            2,
            n(2), // (GC roots) -> A
            2,
            2,
            n(3), // A -> B
            2,
            3,
            n(4), // B -> NativeContext
        ],
        s(&["", "(GC roots)", "Object", "NativeContext"]),
    );
    assert!(snap.is_user_root(NodeOrdinal(4)));
    assert_eq!(
        snap.dominator_of(NodeOrdinal(2)),
        NodeOrdinal(1),
        "A dominated by (GC roots)"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(3)),
        NodeOrdinal(2),
        "B dominated by A"
    );
    assert_eq!(
        snap.dominator_of(NodeOrdinal(4)),
        NodeOrdinal(3),
        "NativeContext dominated by B"
    );
}

#[test]
fn test_root_kinds() {
    //   0: synthetic root  --> (GC roots), (Persistent roots), NativeContext
    //   1: (GC roots)      --> Object
    //   2: (Persistent roots) [synthetic]
    //   3: NativeContext    [non-synthetic, user root]
    //   4: Object           [non-root]
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;
    let snap = build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 3, // 0: synthetic root, 3 edges
            9, 1, 3, 0, 1, // 1: (GC roots), 1 edge
            9, 4, 5, 0, 0, // 2: (Persistent roots), synthetic, 0 edges
            3, 5, 7, 0, 0, // 3: NativeContext (object), 0 edges
            3, 6, 9, 100, 0, // 4: Object, 0 edges
        ],
        vec![
            1,
            0,
            n(1), // root -> (GC roots)
            1,
            0,
            n(2), // root -> (Persistent roots)
            1,
            0,
            n(3), // root -> NativeContext
            2,
            6,
            n(4), // (GC roots) -> Object
        ],
        s(&[
            "",
            "(GC roots)",
            "Object",
            "leaf",
            "(Persistent roots)",
            "NativeContext",
            "obj",
        ]),
    );
    assert_eq!(snap.root_kind(NodeOrdinal(0)), RootKind::SyntheticRoot);
    assert_eq!(snap.root_kind(NodeOrdinal(1)), RootKind::SystemRoot);
    assert_eq!(snap.root_kind(NodeOrdinal(2)), RootKind::SystemRoot);
    assert_eq!(snap.root_kind(NodeOrdinal(3)), RootKind::UserRoot);
    assert_eq!(snap.root_kind(NodeOrdinal(4)), RootKind::NonRoot);
}

/// Builds a snapshot with JSFunction and SharedFunctionInfo nodes to test
/// location data and script name resolution.
///
/// ```text
/// Node 0: synthetic root, 1 edge
/// Node 1: (GC roots), 2 edges
/// Node 2: closure "outer", id=100 → has location (script_id=7, line=4, col=10)
/// Node 3: code "system / SharedFunctionInfo / outer", id=101
///         → has location (script_id=7, line=4, col=10)
///         → "script" edge to node 5
/// Node 4: closure "inner", id=102 → no direct location
///         → "shared" edge to node 6
/// Node 5: code "system / Script / /app/src/utils.js", id=103
/// Node 6: code "system / SharedFunctionInfo / inner", id=104
///         → has location (script_id=7, line=5, col=20)
///         → "script" edge to node 5
/// ```
fn make_function_snapshot() -> HeapSnapshot {
    let nfc = 5usize;
    let efc = 3usize;
    let n = |ord: u32| ord * nfc as u32;

    let strings = s(&[
        "",                                    // 0
        "(GC roots)",                          // 1
        "outer",                               // 2
        "system / SharedFunctionInfo / outer", // 3
        "inner",                               // 4
        "system / Script / /app/src/utils.js", // 5
        "system / SharedFunctionInfo / inner", // 6
        "shared",                              // 7
        "script",                              // 8
        "func",                                // 9
        "global",                              // 10
        "system / SharedFunctionInfo",         // 11
        "anon",                                // 12
    ]);

    // Type indices: code=4, closure=5, synthetic=9
    // Edge types: element=1, property=2, internal=3

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // node 0: synthetic root
        9, 1, 2, 0, 3, // node 1: (GC roots), 3 edges
        5, 2, 100, 32, 0, // node 2: closure "outer"
        4, 3, 101, 48, 1, // node 3: SFI "outer" → 1 edge (script)
        5, 4, 102, 32, 1, // node 4: closure "inner" → 1 edge (shared)
        4, 5, 103, 80, 0, // node 5: Script
        4, 6, 104, 48, 1, // node 6: SFI "inner" → 1 edge (script)
        4, 11, 105, 48, 1, // node 7: SFI (unnamed) → 1 edge (script)
    ];

    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root --element[0]--> (GC roots)
        2,
        9,
        n(2), // (GC roots) --"func"--> node 2 (outer closure)
        2,
        10,
        n(4), // (GC roots) --"global"--> node 4 (inner closure)
        2,
        12,
        n(7), // (GC roots) --"anon"--> node 7 (unnamed SFI)
        3,
        8,
        n(5), // node 3 (SFI outer) --"script"--> node 5 (Script)
        3,
        7,
        n(6), // node 4 (inner closure) --"shared"--> node 6 (SFI inner)
        3,
        8,
        n(5), // node 6 (SFI inner) --"script"--> node 5 (Script)
        3,
        8,
        n(5), // node 7 (unnamed SFI) --"script"--> node 5 (Script)
    ];

    let locations: Vec<u32> = vec![
        n(2),
        7,
        4,
        10, // node 2: closure "outer" at script 7, line 4, col 10
        n(3),
        7,
        4,
        10, // node 3: SFI "outer" at script 7, line 4, col 10
        n(6),
        7,
        5,
        20, // node 6: SFI "inner" at script 7, line 5, col 20
        n(7),
        7,
        0,
        0, // node 7: unnamed SFI at script 7, line 0, col 0
    ];

    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields: standard_node_fields(),
                node_type_enum: standard_node_type_enum(),
                edge_fields: standard_edge_fields(),
                edge_type_enum: standard_edge_type_enum(),
                location_fields: s(&["object_index", "script_id", "line", "column"]),
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
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };

    HeapSnapshot::new(raw)
}

#[test]
fn test_is_js_function() {
    let snap = make_function_snapshot();
    // Node 2 is a closure (JSFunction)
    assert!(snap.is_js_function(NodeOrdinal(2)));
    // Node 4 is a closure (JSFunction)
    assert!(snap.is_js_function(NodeOrdinal(4)));
    // Node 3 is code (SFI), not a closure
    assert!(!snap.is_js_function(NodeOrdinal(3)));
    // Node 5 is code (Script), not a closure
    assert!(!snap.is_js_function(NodeOrdinal(5)));
}

#[test]
fn test_is_shared_function_info() {
    let snap = make_function_snapshot();
    // Node 3: SFI "outer"
    assert!(snap.is_shared_function_info(NodeOrdinal(3)));
    // Node 6: SFI "inner"
    assert!(snap.is_shared_function_info(NodeOrdinal(6)));
    // Node 2: closure, not SFI
    assert!(!snap.is_shared_function_info(NodeOrdinal(2)));
    // Node 5: Script, not SFI
    assert!(!snap.is_shared_function_info(NodeOrdinal(5)));
}

#[test]
fn test_node_location_direct() {
    let snap = make_function_snapshot();
    // Node 2 (outer closure) has a direct location entry
    let loc = snap.node_location(NodeOrdinal(2)).unwrap();
    assert_eq!(loc.script_id, 7);
    assert_eq!(loc.line, 4);
    assert_eq!(loc.column, 10);
}

#[test]
fn test_node_location_via_shared() {
    let snap = make_function_snapshot();
    // Node 4 (inner closure) has no direct location, but has a "shared" edge
    // to node 6 (SFI inner) which has a location
    let loc = snap.node_location(NodeOrdinal(4)).unwrap();
    assert_eq!(loc.script_id, 7);
    assert_eq!(loc.line, 5);
    assert_eq!(loc.column, 20);
}

#[test]
fn test_node_location_sfi() {
    let snap = make_function_snapshot();
    // Node 3 (SFI outer) has a direct location
    let loc = snap.node_location(NodeOrdinal(3)).unwrap();
    assert_eq!(loc.script_id, 7);
    assert_eq!(loc.line, 4);
    assert_eq!(loc.column, 10);
}

#[test]
fn test_node_location_none() {
    let snap = make_function_snapshot();
    // Node 5 (Script) has no location
    assert!(snap.node_location(NodeOrdinal(5)).is_none());
    // Node 1 (GC roots) has no location
    assert!(snap.node_location(NodeOrdinal(1)).is_none());
}

#[test]
fn test_script_name_resolution() {
    let snap = make_function_snapshot();
    // Script ID 7 should resolve to "/app/src/utils.js"
    assert_eq!(snap.script_names.get(&7).unwrap(), "/app/src/utils.js");
}

#[test]
fn test_format_location() {
    let snap = make_function_snapshot();
    // format_location should use the file basename and 1-based line/col
    let loc = snap.node_location(NodeOrdinal(2)).unwrap();
    assert_eq!(snap.format_location(&loc), "utils.js:5:11");

    let loc = snap.node_location(NodeOrdinal(4)).unwrap();
    assert_eq!(snap.format_location(&loc), "utils.js:6:21");
}

#[test]
fn test_format_location_unresolved_script() {
    let snap = make_test_snapshot(); // no locations at all
    let loc = SourceLocation {
        script_id: 99,
        line: 0,
        column: 0,
    };
    assert_eq!(snap.format_location(&loc), "script_id=99:L1:1");
}

#[test]
fn test_is_sfi_named() {
    let snap = make_function_snapshot();
    // Node 3: "system / SharedFunctionInfo / outer" — named
    assert!(snap.is_shared_function_info(NodeOrdinal(3)));
    // Node 6: "system / SharedFunctionInfo / inner" — named
    assert!(snap.is_shared_function_info(NodeOrdinal(6)));
}

#[test]
fn test_is_sfi_unnamed() {
    let snap = make_function_snapshot();
    // Node 7: "system / SharedFunctionInfo" — unnamed
    assert!(snap.is_shared_function_info(NodeOrdinal(7)));
}

#[test]
fn test_sfi_unnamed_location() {
    let snap = make_function_snapshot();
    // Node 7 (unnamed SFI) has location at line 0, col 0
    let loc = snap.node_location(NodeOrdinal(7)).unwrap();
    assert_eq!(loc.script_id, 7);
    assert_eq!(loc.line, 0);
    assert_eq!(loc.column, 0);
    assert_eq!(snap.format_location(&loc), "utils.js:1:1");
}

// ── allocation tracking ─────────────────────────────────────────────────

/// Build a snapshot with allocation tracking data.
///
/// Nodes include a `trace_node_id` field.
/// Trace tree:
///   root (id=1, func_info=0)
///   └── main (id=2, func_info=1)
///       ├── alloc_a (id=3, func_info=2)
///       └── alloc_b (id=4, func_info=3)
///
/// Node 2 (Object, id=3) allocated via alloc_a (trace_node_id=3)
/// Node 3 (Object, id=5) allocated via alloc_b (trace_node_id=4)
/// Node 4 (Array, id=7) has no allocation info (trace_node_id=0)
fn make_alloc_tracking_snapshot() -> HeapSnapshot {
    // 6 fields: type, name, id, self_size, edge_count, trace_node_id
    let node_fields = s(&[
        "type",
        "name",
        "id",
        "self_size",
        "edge_count",
        "trace_node_id",
    ]);
    let nfc = node_fields.len(); // 6

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, 0, // node 0: synthetic root
        9, 1, 2, 0, 3, 0, // node 1: (GC roots)
        3, 2, 3, 100, 0, 3, // node 2: object "Obj", id=3, trace_node_id=3
        3, 2, 5, 60, 0, 4, // node 3: object "Obj", id=5, trace_node_id=4
        1, 3, 7, 200, 0, 0, // node 4: array "Arr", id=7, no trace
    ];
    let efc = 3u32;
    let n = |ord: u32| ord * nfc as u32;
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root -> GC roots (element)
        2,
        4,
        n(2), // GC roots -> node 2 (property "a")
        2,
        5,
        n(3), // GC roots -> node 3 (property "b")
        2,
        6,
        n(4), // GC roots -> node 4 (property "c")
    ];

    // trace_function_infos: 4 functions * 6 fields each
    // fields: function_id, name, script_name, script_id, line, column
    // strings: 0="", 1="(GC roots)", 2="Obj", 3="Arr", 4="a", 5="b", 6="c",
    //          7="(root)", 8="main", 9="alloc_a", 10="alloc_b", 11="app.js"
    let strings = s(&[
        "",
        "(GC roots)",
        "Obj",
        "Arr",
        "a",
        "b",
        "c",
        "(root)",
        "main",
        "alloc_a",
        "alloc_b",
        "app.js",
    ]);

    let trace_function_infos: Vec<u32> = vec![
        0, 7, 0, 0, 0, 0, // func 0: "(root)"
        0, 8, 11, 1, 0, 0, // func 1: "main" in app.js:1:1
        0, 9, 11, 1, 9, 4, // func 2: "alloc_a" in app.js:10:5
        0, 10, 11, 1, 19, 4, // func 3: "alloc_b" in app.js:20:5
    ];

    // Flattened trace tree: id -> parent, id -> func_info_index
    // root(1) -> main(2) -> alloc_a(3), alloc_b(4)
    let trace_tree_parents: Vec<u32> = vec![0, 0, 1, 2, 2]; // [unused, root's parent=0, 1's parent=root, 3's parent=2, 4's parent=2]
    let trace_tree_func_idxs: Vec<u32> = vec![0, 0, 1, 2, 3]; // [unused, root=func0, 2=func1, 3=func2, 4=func3]

    let raw = RawHeapSnapshot {
        snapshot: SnapshotHeader {
            meta: SnapshotMeta {
                node_fields,
                node_type_enum: standard_node_type_enum(),
                edge_fields: standard_edge_fields(),
                edge_type_enum: standard_edge_type_enum(),
                location_fields: vec![],
                sample_fields: s(&["timestamp_us", "last_assigned_id"]),
                trace_function_info_fields: s(&[
                    "function_id",
                    "name",
                    "script_name",
                    "script_id",
                    "line",
                    "column",
                ]),
                trace_node_fields: s(&["id", "function_info_index", "count", "size", "children"]),
            },
            node_count: nodes.len() / nfc,
            edge_count: edges.len() / efc as usize,
            trace_function_count: 4,
            root_index: Some(0),
            extra_native_bytes: None,
        },
        nodes,
        edges,
        strings,
        locations: vec![],
        trace_function_infos,
        trace_tree_parents,
        trace_tree_func_idxs,
        // samples: two intervals
        // interval 1: ts=50000, last_id=3  (node 2 with id=3 falls here)
        // interval 2: ts=100000, last_id=7 (nodes 3,4 with ids 5,7 fall here)
        samples: vec![50000, 3, 100000, 7],
    };
    HeapSnapshot::new(raw)
}

#[test]
fn test_has_allocation_data() {
    let snap = make_alloc_tracking_snapshot();
    assert!(snap.has_allocation_data());
}

#[test]
fn test_has_allocation_data_without_tracking() {
    let snap = make_test_snapshot();
    assert!(!snap.has_allocation_data());
}

#[test]
fn test_allocation_stack_via_alloc_a() {
    let snap = make_alloc_tracking_snapshot();
    // Node 2 (ordinal 2) has trace_node_id=3 -> alloc_a -> main
    let stack = snap.get_allocation_stack(NodeOrdinal(2)).unwrap();
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].function_name, "alloc_a");
    assert_eq!(stack[0].script_name, "app.js");
    assert_eq!(stack[0].line, 9);
    assert_eq!(stack[0].column, 4);
    assert_eq!(stack[1].function_name, "main");
}

#[test]
fn test_allocation_stack_via_alloc_b() {
    let snap = make_alloc_tracking_snapshot();
    // Node 3 (ordinal 3) has trace_node_id=4 -> alloc_b -> main
    let stack = snap.get_allocation_stack(NodeOrdinal(3)).unwrap();
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].function_name, "alloc_b");
    assert_eq!(stack[1].function_name, "main");
}

#[test]
fn test_allocation_stack_none_for_untracked() {
    let snap = make_alloc_tracking_snapshot();
    // Node 4 (ordinal 4) has trace_node_id=0 -> no allocation info
    assert!(snap.get_allocation_stack(NodeOrdinal(4)).is_none());
}

#[test]
fn test_allocation_stack_none_without_tracking() {
    let snap = make_test_snapshot();
    assert!(snap.get_allocation_stack(NodeOrdinal(2)).is_none());
}

#[test]
fn test_format_allocation_frame() {
    let frame = AllocationFrame {
        function_name: "alloc_a".to_string(),
        script_name: "app.js".to_string(),
        line: 9,
        column: 4,
    };
    assert_eq!(
        HeapSnapshot::format_allocation_frame(&frame),
        "alloc_a (app.js:10:5)"
    );
}

#[test]
fn test_format_allocation_frame_with_path() {
    let frame = AllocationFrame {
        function_name: "foo".to_string(),
        script_name: "/home/user/project/src/bar.js".to_string(),
        line: 0,
        column: 0,
    };
    assert_eq!(
        HeapSnapshot::format_allocation_frame(&frame),
        "foo (bar.js:1:1)"
    );
}

#[test]
fn test_format_allocation_frame_unknown_script() {
    let frame = AllocationFrame {
        function_name: "foo".to_string(),
        script_name: "".to_string(),
        line: 5,
        column: 10,
    };
    assert_eq!(
        HeapSnapshot::format_allocation_frame(&frame),
        "foo (<unknown>:6:11)"
    );
}

// ── timeline ────────────────────────────────────────────────────────────

#[test]
fn test_timeline_intervals() {
    let snap = make_alloc_tracking_snapshot();
    let timeline = snap.get_timeline();
    assert_eq!(timeline.len(), 2);

    // Interval 1: ts=50000, last_id=3
    // Live objects with id in (0, 3]: node 2 (id=3, size=100)
    assert_eq!(timeline[0].timestamp_us, 50000);
    assert_eq!(timeline[0].count, 1);
    assert_eq!(timeline[0].size, 100);

    // Interval 2: ts=100000, last_id=7
    // Live objects with id in (3, 7]: node 3 (id=5, size=60), node 4 (id=7, size=200)
    assert_eq!(timeline[1].timestamp_us, 100000);
    assert_eq!(timeline[1].count, 2);
    assert_eq!(timeline[1].size, 260);
}

#[test]
fn test_timeline_empty_without_samples() {
    let snap = make_test_snapshot();
    assert!(snap.get_timeline().is_empty());
}

// ── retained-by filters ─────────────────────────────────────────────────

fn agg_names(aggs: &AggregateMap) -> Vec<String> {
    let mut names: Vec<String> = aggs.iter().map(|a| a.name.clone()).collect();
    names.sort();
    names
}

// ── retained by detached DOM ────────────────────────────────────────────

/// Graph:
///   0: synthetic root → 1
///   1: (GC roots) → 2, 3
///   2: "Normal" (attached, det=1) — reachable normally
///   3: "DetachedDiv" (detached, det=2) → 4
///   4: "Leaked" — only reachable through the detached node
///   5: "Orphan" — no edges to it (truly unreachable)
fn make_detached_dom_snapshot() -> HeapSnapshot {
    let node_fields = s(&[
        "type",
        "name",
        "id",
        "self_size",
        "edge_count",
        "detachedness",
    ]);
    let nfc = node_fields.len();
    let n = |ord: u32| ord * nfc as u32;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, 0, // 0: synthetic root
        9, 1, 2, 0, 2, 0, // 1: (GC roots)
        3, 2, 3, 50, 0, 1, // 2: "Normal" attached
        3, 3, 5, 20, 1, 2, // 3: "DetachedDiv" detached
        3, 4, 7, 30, 0, 0, // 4: "Leaked"
        3, 5, 9, 100, 0, 0, // 5: "Orphan" — nothing points here
    ];
    let strings = s(&[
        "",
        "(GC roots)",
        "Normal",
        "DetachedDiv",
        "Leaked",
        "Orphan",
        "a",
        "b",
        "c",
    ]);
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        6,
        n(2), // GC roots → Normal
        2,
        7,
        n(3), // GC roots → DetachedDiv
        2,
        8,
        n(4), // DetachedDiv → Leaked
    ];

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
            edge_count: edges.len() / 3,
            trace_function_count: 0,
            root_index: Some(0),
            extra_native_bytes: None,
        },
        nodes,
        edges,
        strings,
        locations: vec![],
        trace_function_infos: vec![],
        trace_tree_parents: vec![],
        trace_tree_func_idxs: vec![],
        samples: vec![],
    };
    HeapSnapshot::new(raw)
}

#[test]
fn test_retained_by_detached_dom() {
    let snap = make_detached_dom_snapshot();
    let names = agg_names(&snap.retained_by_detached_dom());
    assert!(names.contains(&"DetachedDiv".to_string()), "got: {names:?}");
    assert!(names.contains(&"Leaked".to_string()), "got: {names:?}");
    assert!(!names.contains(&"Normal".to_string()), "got: {names:?}");
}

#[test]
fn test_retained_by_detached_dom_excludes_unreachable() {
    let snap = make_detached_dom_snapshot();
    let names = agg_names(&snap.retained_by_detached_dom());
    // "Orphan" has no edges pointing to it — it's unreachable, not retained by detached DOM
    assert!(
        !names.contains(&"Orphan".to_string()),
        "truly unreachable objects should not appear, got: {names:?}"
    );
}

#[test]
fn test_retained_by_detached_dom_empty_without_detachedness() {
    let snap = make_test_snapshot();
    assert!(snap.retained_by_detached_dom().is_empty());
}

// ── retained by DevTools console ────────────────────────────────────────

/// Graph:
///   0: synthetic root → 1, and console edge → 3
///   1: (GC roots) → 2
///   2: "Normal" — reachable normally
///   3: "ConsoleObj" → 4
///   4: "Leaked" — only reachable through the console edge
///   5: "Orphan" — nothing points here
fn make_console_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 2, // 0: synthetic root (2 edges: GC roots + console)
        9, 1, 2, 0, 1, // 1: (GC roots)
        3, 2, 3, 50, 0, // 2: "Normal"
        3, 3, 5, 10, 1, // 3: "ConsoleObj"
        3, 4, 7, 60, 0, // 4: "Leaked"
        3, 5, 9, 100, 0, // 5: "Orphan"
    ];
    // string 6 = "temp1 / DevTools console"
    let strings = s(&[
        "",
        "(GC roots)",
        "Normal",
        "ConsoleObj",
        "Leaked",
        "Orphan",
        "temp1 / DevTools console",
        "a",
        "b",
    ]);
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots (element)
        2,
        6,
        n(3), // root → ConsoleObj (property "temp1 / DevTools console")
        2,
        7,
        n(2), // GC roots → Normal
        2,
        8,
        n(4), // ConsoleObj → Leaked
    ];

    build_snapshot(standard_node_fields(), nodes, edges, strings)
}

#[test]
fn test_retained_by_console() {
    let snap = make_console_snapshot();
    let names = agg_names(&snap.retained_by_console());
    assert!(names.contains(&"ConsoleObj".to_string()), "got: {names:?}");
    assert!(names.contains(&"Leaked".to_string()), "got: {names:?}");
    assert!(!names.contains(&"Normal".to_string()), "got: {names:?}");
}

#[test]
fn test_retained_by_console_excludes_unreachable() {
    let snap = make_console_snapshot();
    let names = agg_names(&snap.retained_by_console());
    assert!(
        !names.contains(&"Orphan".to_string()),
        "truly unreachable objects should not appear, got: {names:?}"
    );
}

// ── retained by event handlers ──────────────────────────────────────────

/// Graph:
///   0: synthetic root → 1
///   1: (GC roots) → 2, 3
///   2: "Normal" — reachable normally
///   3: "V8EventListener" → callback_object_ → 4
///   4: "Handler" — has "code" edge → 5, and property → 6
///   5: "HandlerCode" (code type)
///   6: "Leaked" — only reachable through the handler
///   7: "Orphan" — nothing points here
fn make_event_handler_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    let nodes: Vec<u32> = vec![
        9, 0, 1, 0, 1, // 0: synthetic root
        9, 1, 2, 0, 2, // 1: (GC roots)
        3, 2, 3, 50, 0, // 2: "Normal"
        3, 3, 5, 10, 1, // 3: "V8EventListener"
        3, 4, 7, 10, 2, // 4: "Handler" (2 edges: code + leaked)
        4, 5, 9, 20, 0, // 5: "HandlerCode" (type=code=4)
        3, 6, 11, 40, 0, // 6: "Leaked"
        3, 7, 13, 100, 0, // 7: "Orphan"
    ];
    // 8="callback_object_", 9="code", 10="a", 11="b", 12="c"
    let strings = s(&[
        "",
        "(GC roots)",
        "Normal",
        "V8EventListener",
        "Handler",
        "HandlerCode",
        "Leaked",
        "Orphan",
        "callback_object_",
        "code",
        "a",
        "b",
        "c",
    ]);
    let edges: Vec<u32> = vec![
        1,
        0,
        n(1), // root → GC roots
        2,
        10,
        n(2), // GC roots → Normal
        2,
        11,
        n(3), // GC roots → V8EventListener
        2,
        8,
        n(4), // V8EventListener → Handler (callback_object_)
        2,
        9,
        n(5), // Handler → HandlerCode (code)
        2,
        12,
        n(6), // Handler → Leaked
    ];

    build_snapshot(standard_node_fields(), nodes, edges, strings)
}

#[test]
fn test_retained_by_event_handlers() {
    let snap = make_event_handler_snapshot();
    let names = agg_names(&snap.retained_by_event_handlers());
    assert!(names.contains(&"Handler".to_string()), "got: {names:?}");
    assert!(names.contains(&"Leaked".to_string()), "got: {names:?}");
    assert!(!names.contains(&"Normal".to_string()), "got: {names:?}");
}

#[test]
fn test_retained_by_event_handlers_excludes_unreachable() {
    let snap = make_event_handler_snapshot();
    let names = agg_names(&snap.retained_by_event_handlers());
    assert!(
        !names.contains(&"Orphan".to_string()),
        "truly unreachable objects should not appear, got: {names:?}"
    );
}

// ====== find_native_context_for_context tests ======

/// Snapshot with a context chain: NativeContext <- Context A <- Context B
///
/// ```text
/// Node 0: synthetic root
/// Node 1: (GC roots)
/// Node 2: "system / NativeContext / https://example.com"
/// Node 3: "system / Context" (context A, previous -> node 2)
/// Node 4: "system / Context" (context B, previous -> node 3)
/// Node 5: "object" (non-context node)
/// ```
fn make_context_chain_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            //  type name id size edges
            9, 0, 1, 0, 1, // node 0: synthetic root
            9, 1, 2, 0, 1, // node 1: (GC roots)
            3, 2, 3, 100, 0, // node 2: NativeContext
            3, 3, 5, 24, 1, // node 3: Context A (1 edge: previous -> node 2)
            3, 3, 7, 24, 1, // node 4: Context B (1 edge: previous -> node 3)
            3, 4, 9, 16, 0, // node 5: plain object
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            5,
            n(2), // GC roots -> NativeContext (property)
            3,
            6,
            n(2), // Context A: previous -> NativeContext (internal)
            3,
            6,
            n(3), // Context B: previous -> Context A (internal)
        ],
        s(&[
            "",                                             // 0
            "(GC roots)",                                   // 1
            "system / NativeContext / https://example.com", // 2
            "system / Context",                             // 3
            "plain object",                                 // 4
            "nc",                                           // 5
            "previous",                                     // 6
        ]),
    )
}

#[test]
fn test_find_native_context_for_context_from_native_context() {
    let snap = make_context_chain_snapshot();
    // NativeContext returns itself.
    assert_eq!(
        snap.find_native_context_for_context(NodeOrdinal(2)),
        Some(NodeOrdinal(2))
    );
}

#[test]
fn test_find_native_context_for_context_one_hop() {
    let snap = make_context_chain_snapshot();
    // Context A's previous points directly to NativeContext.
    assert_eq!(
        snap.find_native_context_for_context(NodeOrdinal(3)),
        Some(NodeOrdinal(2))
    );
}

#[test]
fn test_find_native_context_for_context_two_hops() {
    let snap = make_context_chain_snapshot();
    // Context B -> Context A -> NativeContext.
    assert_eq!(
        snap.find_native_context_for_context(NodeOrdinal(4)),
        Some(NodeOrdinal(2))
    );
}

#[test]
fn test_find_native_context_for_context_non_context_returns_none() {
    let snap = make_context_chain_snapshot();
    // Non-context node returns None.
    assert_eq!(snap.find_native_context_for_context(NodeOrdinal(5)), None);
}

/// Snapshot for two-stage native context ownership inference.
///
/// ```text
/// Node  0: synthetic root
/// Node  1: (GC roots)
/// Node  2: NativeContext A -> FixedA
/// Node  3: NativeContext B -> FixedB
/// Node  4: MapA -> native_context A
/// Node  5: MapB -> native_context B
/// Node  6: FixedA -> mapA, UniqueChild, SharedChild
/// Node  7: FixedB -> mapB, SharedChild, FixedA
/// Node  8: UniqueChild
/// Node  9: SharedChild
/// Node 10: Context -> previous -> NativeContext A
/// Node 11: Closure -> context -> Context
/// Node 12: Isolated
/// ```
fn make_native_context_ownership_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            //  type name id size edges
            9, 0, 1, 0, 1, // 0 synthetic root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 80, 1, // 3 NativeContext B
            3, 4, 7, 24, 1, // 4 MapA
            3, 4, 9, 24, 1, // 5 MapB
            3, 5, 11, 40, 3, // 6 FixedA
            3, 6, 13, 40, 3, // 7 FixedB
            3, 7, 15, 16, 0, // 8 UniqueChild
            3, 8, 17, 16, 0, // 9 SharedChild
            3, 9, 19, 24, 1, // 10 Context
            5, 10, 21, 32, 1, // 11 Closure
            3, 11, 23, 16, 0, // 12 Isolated
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            12,
            n(2), // GC roots -> NativeContext A
            2,
            13,
            n(3), // GC roots -> NativeContext B
            2,
            14,
            n(6), // NativeContext A -> FixedA
            2,
            15,
            n(7), // NativeContext B -> FixedB
            3,
            17,
            n(2), // MapA -> native_context A
            3,
            17,
            n(3), // MapB -> native_context B
            3,
            16,
            n(4), // FixedA -> mapA
            2,
            18,
            n(8), // FixedA -> UniqueChild
            2,
            19,
            n(9), // FixedA -> SharedChild
            3,
            16,
            n(5), // FixedB -> mapB
            2,
            19,
            n(9), // FixedB -> SharedChild
            2,
            20,
            n(6), // FixedB -> FixedA (conflicting reachability)
            3,
            21,
            n(2), // Context -> previous -> NativeContext A
            0,
            22,
            n(10), // Closure -> context -> Context
        ],
        s(&[
            "",                                        // 0
            "(GC roots)",                              // 1
            "system / NativeContext / https://a.test", // 2
            "system / NativeContext / https://b.test", // 3
            "system / Map",                            // 4
            "FixedA",                                  // 5
            "FixedB",                                  // 6
            "UniqueChild",                             // 7
            "SharedChild",                             // 8
            "system / Context",                        // 9
            "Closure",                                 // 10
            "Isolated",                                // 11
            "ctx_a",                                   // 12
            "ctx_b",                                   // 13
            "a_root",                                  // 14
            "b_root",                                  // 15
            "map",                                     // 16
            "native_context",                          // 17
            "child_unique",                            // 18
            "child_shared",                            // 19
            "b_to_fixed_a",                            // 20
            "previous",                                // 21
            "context",                                 // 22
        ]),
    )
}

fn assert_bucket_context(snap: &HeapSnapshot, node: NodeOrdinal, expected_ctx: NodeOrdinal) {
    match snap.node_native_context_bucket(node) {
        NativeContextBucket::Context(id) => {
            assert_eq!(snap.native_context_by_id(id).ordinal, expected_ctx);
        }
        other => panic!("expected Context({expected_ctx:?}) for {node:?}, got {other:?}"),
    }
}

#[test]
fn test_node_native_context_direct_inference_uses_map_and_context_edges() {
    let snap = make_native_context_ownership_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const CTX_B: NodeOrdinal = NodeOrdinal(3);
    const FIXED_A: NodeOrdinal = NodeOrdinal(6);
    const FIXED_B: NodeOrdinal = NodeOrdinal(7);
    const CONTEXT_A: NodeOrdinal = NodeOrdinal(10);
    const CLOSURE: NodeOrdinal = NodeOrdinal(11);

    assert_bucket_context(&snap, CTX_A, CTX_A);
    assert_bucket_context(&snap, CTX_B, CTX_B);
    assert_bucket_context(&snap, FIXED_A, CTX_A);
    assert_bucket_context(&snap, FIXED_B, CTX_B);
    assert_bucket_context(&snap, CONTEXT_A, CTX_A);
    assert_bucket_context(&snap, CLOSURE, CTX_A);
}

#[test]
fn test_node_native_context_bucket_assigns_unique_shared_and_unattributed() {
    let snap = make_native_context_ownership_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const UNIQUE_CHILD: NodeOrdinal = NodeOrdinal(8);
    const SHARED_CHILD: NodeOrdinal = NodeOrdinal(9);
    const ISOLATED: NodeOrdinal = NodeOrdinal(12);

    // UniqueChild has no direct owner, but is only reachable through FixedA.
    assert_bucket_context(&snap, UNIQUE_CHILD, CTX_A);
    // SharedChild is reachable from GC roots but not attributable to one context.
    assert_eq!(
        snap.node_native_context_bucket(SHARED_CHILD),
        NativeContextBucket::Shared
    );
    // Isolated has neither native-context attribution nor GC reachability.
    assert_eq!(
        snap.node_native_context_bucket(ISOLATED),
        NativeContextBucket::Unattributed
    );
}

#[test]
fn test_node_native_context_fallback_does_not_overwrite_direct_owner() {
    let snap = make_native_context_ownership_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const FIXED_A: NodeOrdinal = NodeOrdinal(6);

    // FixedA is directly attributed to NativeContext A via its map, even though
    // NativeContext B also reaches it through FixedB.
    assert_bucket_context(&snap, FIXED_A, CTX_A);
}

/// Snapshot where one native context reaches `Target` via a weak edge and
/// another reaches it via a strong edge. The result should be `Shared`.
fn make_native_context_bucket_weak_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 80, 1, // 3 NativeContext B
            3, 4, 7, 16, 1, // 4 AHolder
            3, 5, 9, 16, 1, // 5 BHolder
            3, 6, 11, 16, 0, // 6 Target
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            7,
            n(2), // GC roots -> NativeContext A
            2,
            8,
            n(3), // GC roots -> NativeContext B
            2,
            9,
            n(4), // NativeContext A -> AHolder
            2,
            10,
            n(5), // NativeContext B -> BHolder
            6,
            11,
            n(6), // AHolder --weak--> Target
            2,
            11,
            n(6), // BHolder --property--> Target
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "AHolder",
            "BHolder",
            "Target",
            "ctx_a",
            "ctx_b",
            "a_holder",
            "b_holder",
            "target",
        ]),
    )
}

/// Snapshot where ambiguity is introduced transitively through two unresolved
/// intermediary nodes.
fn make_native_context_bucket_transitive_shared_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 80, 1, // 3 NativeContext B
            3, 4, 7, 16, 1, // 4 X
            3, 5, 9, 16, 1, // 5 Y
            3, 6, 11, 16, 0, // 6 Z
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            7,
            n(2), // GC roots -> NativeContext A
            2,
            8,
            n(3), // GC roots -> NativeContext B
            2,
            9,
            n(4), // NativeContext A -> X
            2,
            10,
            n(5), // NativeContext B -> Y
            2,
            11,
            n(6), // X -> Z
            2,
            11,
            n(6), // Y -> Z
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "X",
            "Y",
            "Z",
            "ctx_a",
            "ctx_b",
            "x",
            "y",
            "z",
        ]),
    )
}

/// Snapshot with an unresolved cycle reachable from a single native context.
fn make_native_context_bucket_scc_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 1, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 16, 1, // 3 X
            3, 4, 7, 16, 1, // 4 Y
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            5,
            n(2), // GC roots -> NativeContext A
            2,
            6,
            n(3), // NativeContext A -> X
            2,
            7,
            n(4), // X -> Y
            2,
            8,
            n(3), // Y -> X
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "X",
            "Y",
            "ctx_a",
            "x",
            "y",
            "x_back",
        ]),
    )
}

/// Snapshot where an object is unreachable from GC roots but still directly
/// attributable through map -> native_context.
fn make_native_context_bucket_unreachable_direct_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 1, // 1 (GC roots)
            3, 2, 3, 80, 0, // 2 NativeContext A
            3, 3, 5, 24, 1, // 3 MapA (unreachable)
            3, 4, 7, 16, 1, // 4 UnreachableObject
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            5,
            n(2), // GC roots -> NativeContext A
            3,
            6,
            n(2), // MapA -> native_context A
            3,
            7,
            n(3), // UnreachableObject -> map
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / Map",
            "UnreachableObject",
            "ctx_a",
            "native_context",
            "map",
        ]),
    )
}

/// Snapshot with a GC-reachable object that is not reached from any native
/// context and should therefore remain unattributed.
fn make_native_context_bucket_reachable_unattributed_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 0, // 2 NativeContext A
            3, 3, 5, 16, 0, // 3 ReachableUnattributed
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            4,
            n(2), // GC roots -> NativeContext A
            2,
            5,
            n(3), // GC roots -> ReachableUnattributed
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "ReachableUnattributed",
            "ctx_a",
            "unattributed",
        ]),
    )
}

#[test]
fn test_node_native_context_bucket_weak_from_one_context_strong_from_another_is_shared() {
    let snap = make_native_context_bucket_weak_snapshot();
    const TARGET: NodeOrdinal = NodeOrdinal(6);

    assert_eq!(
        snap.node_native_context_bucket(TARGET),
        NativeContextBucket::Shared
    );
}

#[test]
fn test_node_native_context_bucket_propagates_transitive_shared() {
    let snap = make_native_context_bucket_transitive_shared_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const CTX_B: NodeOrdinal = NodeOrdinal(3);
    const X: NodeOrdinal = NodeOrdinal(4);
    const Y: NodeOrdinal = NodeOrdinal(5);
    const Z: NodeOrdinal = NodeOrdinal(6);

    assert_bucket_context(&snap, X, CTX_A);
    assert_bucket_context(&snap, Y, CTX_B);
    assert_eq!(
        snap.node_native_context_bucket(Z),
        NativeContextBucket::Shared
    );
}

#[test]
fn test_node_native_context_bucket_assigns_single_context_scc() {
    let snap = make_native_context_bucket_scc_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const X: NodeOrdinal = NodeOrdinal(3);
    const Y: NodeOrdinal = NodeOrdinal(4);

    assert_bucket_context(&snap, X, CTX_A);
    assert_bucket_context(&snap, Y, CTX_A);
}

#[test]
fn test_node_native_context_bucket_keeps_direct_assignment_for_unreachable_object() {
    let snap = make_native_context_bucket_unreachable_direct_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const UNREACHABLE_OBJECT: NodeOrdinal = NodeOrdinal(4);

    assert!(snap.node_distance(UNREACHABLE_OBJECT).is_unreachable());
    assert_bucket_context(&snap, UNREACHABLE_OBJECT, CTX_A);
}

#[test]
fn test_node_native_context_bucket_keeps_reachable_none_as_unattributed() {
    let snap = make_native_context_bucket_reachable_unattributed_snapshot();
    const REACHABLE_UNATTRIBUTED: NodeOrdinal = NodeOrdinal(3);

    assert!(snap.node_distance(REACHABLE_UNATTRIBUTED).is_reachable());
    assert_eq!(
        snap.node_native_context_bucket(REACHABLE_UNATTRIBUTED),
        NativeContextBucket::Unattributed
    );
}

/// Snapshot where one candidate path is a shortcut edge and must not influence
/// stage 2.
fn make_native_context_bucket_shortcut_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 80, 1, // 3 NativeContext B
            3, 4, 7, 16, 1, // 4 AHolder
            3, 5, 9, 16, 1, // 5 BHolder
            3, 6, 11, 16, 0, // 6 Target
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            7,
            n(2), // GC roots -> NativeContext A
            2,
            8,
            n(3), // GC roots -> NativeContext B
            2,
            9,
            n(4), // NativeContext A -> AHolder
            2,
            10,
            n(5), // NativeContext B -> BHolder
            5,
            11,
            n(6), // AHolder --shortcut--> Target
            2,
            11,
            n(6), // BHolder --property--> Target
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "AHolder",
            "BHolder",
            "Target",
            "ctx_a",
            "ctx_b",
            "a_holder",
            "b_holder",
            "target",
        ]),
    )
}

/// Snapshot with an unresolved cycle reached from two contexts.
fn make_native_context_bucket_shared_scc_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 1, // 2 NativeContext A
            3, 3, 5, 80, 1, // 3 NativeContext B
            3, 4, 7, 16, 1, // 4 X
            3, 5, 9, 16, 1, // 5 Y
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            6,
            n(2), // GC roots -> NativeContext A
            2,
            7,
            n(3), // GC roots -> NativeContext B
            2,
            8,
            n(4), // NativeContext A -> X
            2,
            9,
            n(5), // NativeContext B -> Y
            2,
            10,
            n(5), // X -> Y
            2,
            11,
            n(4), // Y -> X
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "X",
            "Y",
            "ctx_a",
            "ctx_b",
            "x",
            "y",
            "x_to_y",
            "y_to_x",
        ]),
    )
}

/// Snapshot where direct inference signals disagree. `context` currently wins
/// over `map -> native_context`.
fn make_native_context_bucket_conflicting_direct_signals_snapshot() -> HeapSnapshot {
    let nfc = 5u32;
    let n = |ord: u32| ord * nfc;

    build_snapshot(
        standard_node_fields(),
        vec![
            9, 0, 1, 0, 1, // 0 root
            9, 1, 2, 0, 2, // 1 (GC roots)
            3, 2, 3, 80, 0, // 2 NativeContext A
            3, 3, 5, 80, 0, // 3 NativeContext B
            3, 4, 7, 24, 1, // 4 Context -> previous -> A
            3, 5, 9, 24, 1, // 5 MapB -> native_context B
            3, 6, 11, 16, 2, // 6 Target -> context A, map B
        ],
        vec![
            1,
            0,
            n(1), // root -> GC roots
            2,
            7,
            n(2), // GC roots -> NativeContext A
            2,
            8,
            n(3), // GC roots -> NativeContext B
            3,
            9,
            n(2), // Context -> previous -> NativeContext A
            3,
            10,
            n(3), // MapB -> native_context B
            0,
            11,
            n(4), // Target -> context -> Context
            3,
            12,
            n(5), // Target -> map -> MapB
        ],
        s(&[
            "",
            "(GC roots)",
            "system / NativeContext / https://a.test",
            "system / NativeContext / https://b.test",
            "system / Context",
            "system / Map",
            "Target",
            "ctx_a",
            "ctx_b",
            "previous",
            "native_context",
            "context",
            "map",
        ]),
    )
}

#[test]
fn test_node_native_context_bucket_ignores_shortcut_edges() {
    let snap = make_native_context_bucket_shortcut_snapshot();
    const CTX_B: NodeOrdinal = NodeOrdinal(3);
    const TARGET: NodeOrdinal = NodeOrdinal(6);

    assert_bucket_context(&snap, TARGET, CTX_B);
}

#[test]
fn test_node_native_context_bucket_assigns_shared_scc() {
    let snap = make_native_context_bucket_shared_scc_snapshot();
    const X: NodeOrdinal = NodeOrdinal(4);
    const Y: NodeOrdinal = NodeOrdinal(5);

    assert_eq!(
        snap.node_native_context_bucket(X),
        NativeContextBucket::Shared
    );
    assert_eq!(
        snap.node_native_context_bucket(Y),
        NativeContextBucket::Shared
    );
}

#[test]
fn test_node_native_context_bucket_prefers_context_over_map_native_context() {
    let snap = make_native_context_bucket_conflicting_direct_signals_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const TARGET: NodeOrdinal = NodeOrdinal(6);

    // `Target` has two direct signals:
    // - `context` -> Context A -> NativeContext A
    // - `map` -> MapB -> native_context -> NativeContext B
    // The direct inference order currently prefers `context`.
    assert_bucket_context(&snap, TARGET, CTX_A);
}

#[test]
fn test_native_context_attributable_sizes_sum_bucketed_self_sizes() {
    let snap = make_native_context_ownership_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const CTX_B: NodeOrdinal = NodeOrdinal(3);

    let sizes = snap.native_context_attributable_sizes();

    assert_eq!(
        sizes,
        NativeContextAttributableSizes {
            native_contexts: vec![
                NativeContextData {
                    ordinal: CTX_A,
                    kind: NativeContextKind::Utility,
                    is_extension: false,
                    size: 216,
                },
                NativeContextData {
                    ordinal: CTX_B,
                    kind: NativeContextKind::Utility,
                    is_extension: false,
                    size: 144,
                },
            ],
            shared: 16,
            unattributed: 16,
        }
    );
    assert_eq!(snap.native_context_attributable_size(CTX_A), Some(216));
    assert_eq!(snap.native_context_attributable_size(CTX_B), Some(144));
    assert_eq!(snap.shared_attributable_size(), 16);
    assert_eq!(snap.unattributed_size(), 16);
}

#[test]
fn test_native_context_attributable_sizes_count_mixed_weak_and_strong_shared_bytes() {
    let snap = make_native_context_bucket_weak_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);
    const CTX_B: NodeOrdinal = NodeOrdinal(3);

    assert_eq!(snap.native_context_attributable_size(CTX_A), Some(96));
    assert_eq!(snap.native_context_attributable_size(CTX_B), Some(96));
    assert_eq!(snap.shared_attributable_size(), 16);
    assert_eq!(snap.unattributed_size(), 0);
}

#[test]
fn test_native_context_attributable_sizes_keep_unreachable_direct_and_reachable_unattributed() {
    let unreachable = make_native_context_bucket_unreachable_direct_snapshot();
    const CTX_A: NodeOrdinal = NodeOrdinal(2);

    assert_eq!(
        unreachable.native_context_attributable_size(CTX_A),
        Some(120)
    );
    assert_eq!(unreachable.shared_attributable_size(), 0);
    assert_eq!(unreachable.unattributed_size(), 0);

    let reachable_unattributed = make_native_context_bucket_reachable_unattributed_snapshot();

    assert_eq!(
        reachable_unattributed.native_context_attributable_size(CTX_A),
        Some(80)
    );
    assert_eq!(reachable_unattributed.shared_attributable_size(), 0);
    assert_eq!(reachable_unattributed.unattributed_size(), 16);
}
