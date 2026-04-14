use rustc_hash::{FxHashMap, FxHashSet};

use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

#[derive(Clone)]
pub struct ClassDiff {
    pub name: String,
    pub new_count: u32,
    pub deleted_count: u32,
    pub alloc_size: u64,
    pub freed_size: u64,
    pub new_objects: Vec<(NodeId, u32)>,
    pub deleted_objects: Vec<(NodeId, u32)>,
}

impl ClassDiff {
    pub fn delta_count(&self) -> i64 {
        self.new_count as i64 - self.deleted_count as i64
    }

    pub fn size_delta(&self) -> i64 {
        self.alloc_size as i64 - self.freed_size as i64
    }
}

fn build_node_id_set(snap: &HeapSnapshot) -> FxHashSet<NodeId> {
    let mut set = FxHashSet::default();
    for i in 0..snap.node_count() {
        let ordinal = NodeOrdinal(i);
        if snap.node_self_size(ordinal) == 0 {
            continue;
        }
        set.insert(snap.node_id(ordinal));
    }
    set
}

pub fn compute_diff(snap1: &HeapSnapshot, snap2: &HeapSnapshot) -> Vec<ClassDiff> {
    let snap1_ids = build_node_id_set(snap1);
    let snap2_ids = build_node_id_set(snap2);

    let mut diffs: FxHashMap<String, ClassDiff> = FxHashMap::default();

    // New objects: in snap1 (main) but not snap2 (baseline)
    for i in 0..snap1.node_count() {
        let ordinal = NodeOrdinal(i);
        let self_size = snap1.node_self_size(ordinal);
        if self_size == 0 {
            continue;
        }
        let node_id = snap1.node_id(ordinal);
        if !snap2_ids.contains(&node_id) {
            let class_name = snap1.node_class_name(ordinal);
            let entry = diffs.entry(class_name.clone()).or_insert(ClassDiff {
                name: class_name,
                new_count: 0,
                deleted_count: 0,
                alloc_size: 0,
                freed_size: 0,
                new_objects: Vec::new(),
                deleted_objects: Vec::new(),
            });
            entry.new_count += 1;
            entry.alloc_size += self_size as u64;
            entry.new_objects.push((node_id, self_size));
        }
    }

    // Deleted objects: in snap2 (baseline) but not snap1 (main)
    for i in 0..snap2.node_count() {
        let ordinal = NodeOrdinal(i);
        let self_size = snap2.node_self_size(ordinal);
        if self_size == 0 {
            continue;
        }
        let node_id = snap2.node_id(ordinal);
        if !snap1_ids.contains(&node_id) {
            let class_name = snap2.node_class_name(ordinal);
            let entry = diffs.entry(class_name.clone()).or_insert(ClassDiff {
                name: class_name,
                new_count: 0,
                deleted_count: 0,
                alloc_size: 0,
                freed_size: 0,
                new_objects: Vec::new(),
                deleted_objects: Vec::new(),
            });
            entry.deleted_count += 1;
            entry.freed_size += self_size as u64;
            entry.deleted_objects.push((node_id, self_size));
        }
    }

    // Sort by alloc size descending
    let mut entries: Vec<_> = diffs.into_values().collect();
    entries.sort_by(|a, b| b.alloc_size.cmp(&a.alloc_size));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::HeapSnapshot;
    use crate::types::{RawHeapSnapshot, SnapshotHeader, SnapshotMeta};

    fn make_meta() -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        let node_fields: Vec<String> = ["type", "name", "id", "self_size", "edge_count"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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
        let edge_type_enum: Vec<String> = [
            "context", "element", "property", "internal", "hidden", "shortcut", "weak",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        (node_fields, node_type_enum, edge_fields, edge_type_enum)
    }

    fn make_snapshot(nodes: Vec<u32>, edges: Vec<u32>, strings: Vec<String>) -> HeapSnapshot {
        let (node_fields, node_type_enum, edge_fields, edge_type_enum) = make_meta();
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
            trace_function_infos: vec![],
            trace_tree_parents: vec![],
            trace_tree_func_idxs: vec![],
            samples: vec![],
        };
        HeapSnapshot::new(raw)
    }

    // strings: 0:"", 1:"(GC roots)", 2:"Object", 3:"hello", 4:"Array", 5:"global", 6:"arr", 7:"str"
    fn base_strings() -> Vec<String> {
        [
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
        .collect()
    }

    /// snap1: synthetic_root -> (GC roots) -> Object(id=3,100) -> string(id=5,50)
    ///                           (GC roots) -> Array(id=7,200)
    fn make_snap1() -> HeapSnapshot {
        let nfc = 5u32;
        make_snapshot(
            vec![
                9, 0, 1, 0, 1, // node 0: synthetic root, id=1, size=0, 1 edge
                9, 1, 2, 0, 2, // node 1: (GC roots), id=2, size=0, 2 edges
                3, 2, 3, 100, 1, // node 2: Object, id=3, size=100, 1 edge
                2, 3, 5, 50, 0, // node 3: string "hello", id=5, size=50
                1, 4, 7, 200, 0, // node 4: Array, id=7, size=200
            ],
            vec![
                1,
                0,
                1 * nfc, // root -> (GC roots)
                2,
                5,
                2 * nfc, // (GC roots) -> Object
                2,
                6,
                4 * nfc, // (GC roots) -> Array
                2,
                7,
                3 * nfc, // Object -> string
            ],
            base_strings(),
        )
    }

    /// snap2: same structure but Array(id=7) deleted, new Foo(id=9,150) + new string(id=11,60) added
    fn make_snap2() -> HeapSnapshot {
        let nfc = 5u32;
        let mut strings = base_strings();
        // 8: "Foo", 9: "world", 10: "foo", 11: "ref"
        strings.push("Foo".to_string());
        strings.push("world".to_string());
        strings.push("foo".to_string());
        strings.push("ref".to_string());
        make_snapshot(
            vec![
                9, 0, 1, 0, 1, // node 0: synthetic root, id=1, size=0, 1 edge
                9, 1, 2, 0, 4, // node 1: (GC roots), id=2, size=0, 4 edges
                3, 2, 3, 100, 1, // node 2: Object, id=3, size=100 (same)
                2, 3, 5, 50, 0, // node 3: string "hello", id=5, size=50 (same)
                3, 8, 9, 150, 0, // node 4: Foo, id=9, size=150 (new)
                2, 9, 11, 60, 0, // node 5: string "world", id=11, size=60 (new)
            ],
            vec![
                1,
                0,
                1 * nfc, // root -> (GC roots)
                2,
                5,
                2 * nfc, // (GC roots) -> Object
                2,
                10,
                4 * nfc, // (GC roots) -> Foo
                2,
                7,
                3 * nfc, // (GC roots) -> string "hello"
                2,
                11,
                5 * nfc, // (GC roots) -> string "world"
                2,
                7,
                3 * nfc, // Object -> string "hello"
            ],
            strings,
        )
    }

    #[test]
    fn test_diff_new_objects() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        // snap1 is main, snap2 is baseline
        // Array(id=7) is in snap1 but not snap2 → new
        let diffs = compute_diff(&snap1, &snap2);

        let arr = diffs.iter().find(|d| d.name == "(array)").unwrap();
        assert_eq!(arr.new_count, 1);
        assert_eq!(arr.deleted_count, 0);
        assert_eq!(arr.alloc_size, 200);
        assert_eq!(arr.freed_size, 0);
        assert_eq!(arr.new_objects.len(), 1);
        assert_eq!(arr.new_objects[0], (NodeId(7), 200));
    }

    #[test]
    fn test_diff_deleted_objects() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        // Foo(id=9) is in snap2 but not snap1 → deleted (existed in baseline only)
        let diffs = compute_diff(&snap1, &snap2);

        let foo = diffs.iter().find(|d| d.name == "Foo").unwrap();
        assert_eq!(foo.new_count, 0);
        assert_eq!(foo.deleted_count, 1);
        assert_eq!(foo.alloc_size, 0);
        assert_eq!(foo.freed_size, 150);
        assert_eq!(foo.deleted_objects.len(), 1);
        assert_eq!(foo.deleted_objects[0], (NodeId(9), 150));
    }

    #[test]
    fn test_diff_new_string() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        // string "world"(id=11) is in snap2 but not snap1 → deleted
        let diffs = compute_diff(&snap1, &snap2);

        let s = diffs.iter().find(|d| d.name == "(string)").unwrap();
        assert_eq!(s.new_count, 0);
        assert_eq!(s.deleted_count, 1);
        assert_eq!(s.alloc_size, 0);
        assert_eq!(s.freed_size, 60);
    }

    #[test]
    fn test_diff_excludes_unchanged() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        let diffs = compute_diff(&snap1, &snap2);

        // Object id=3 exists in both -> should not appear as "Object" class
        assert!(
            diffs.iter().all(|d| d.name != "Object"),
            "unchanged objects should not appear in diff"
        );
    }

    #[test]
    fn test_diff_delta_count() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        let diffs = compute_diff(&snap1, &snap2);

        // Foo is only in snap2 (baseline) → deleted, delta = -1
        let foo = diffs.iter().find(|d| d.name == "Foo").unwrap();
        assert_eq!(foo.delta_count(), -1);

        // Array is only in snap1 (main) → new, delta = +1
        let arr = diffs.iter().find(|d| d.name == "(array)").unwrap();
        assert_eq!(arr.delta_count(), 1);
    }

    #[test]
    fn test_diff_size_delta() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        let diffs = compute_diff(&snap1, &snap2);

        let foo = diffs.iter().find(|d| d.name == "Foo").unwrap();
        assert_eq!(foo.size_delta(), -150);

        let arr = diffs.iter().find(|d| d.name == "(array)").unwrap();
        assert_eq!(arr.size_delta(), 200);
    }

    #[test]
    fn test_diff_sorted_by_alloc_size() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();
        let diffs = compute_diff(&snap1, &snap2);

        let alloc_sizes: Vec<u64> = diffs.iter().map(|d| d.alloc_size).collect();
        for w in alloc_sizes.windows(2) {
            assert!(w[0] >= w[1], "expected descending alloc_size order");
        }
    }

    #[test]
    fn test_diff_identical_snapshots() {
        let snap1 = make_snap1();
        let snap1b = make_snap1();
        let diffs = compute_diff(&snap1, &snap1b);
        assert!(
            diffs.is_empty(),
            "identical snapshots should produce no diff"
        );
    }

    #[test]
    fn test_diff_reversed() {
        let snap1 = make_snap1();
        let snap2 = make_snap2();

        let forward = compute_diff(&snap1, &snap2);
        let reverse = compute_diff(&snap2, &snap1);

        let fwd_foo = forward.iter().find(|d| d.name == "Foo").unwrap();
        let rev_foo = reverse.iter().find(|d| d.name == "Foo").unwrap();
        assert_eq!(fwd_foo.new_count, rev_foo.deleted_count);
        assert_eq!(fwd_foo.deleted_count, rev_foo.new_count);
        assert_eq!(fwd_foo.alloc_size, rev_foo.freed_size);
        assert_eq!(fwd_foo.freed_size, rev_foo.alloc_size);
    }
}
