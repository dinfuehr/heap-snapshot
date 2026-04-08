use rustc_hash::{FxHashMap, FxHashSet};

use crate::function_info::{ScopeInfo, extract_scopes};
use crate::print::format_size;
use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

pub struct ContextLeak {
    pub context_ord: NodeOrdinal,
    pub result: UnusedVarsResult,
}

/// Analyze the given contexts for unused variables.
pub fn find_closure_leaks(snap: &HeapSnapshot, contexts: &[NodeOrdinal]) -> Vec<ContextLeak> {
    let mut children_map: FxHashMap<NodeOrdinal, Vec<NodeOrdinal>> = FxHashMap::default();
    let mut closures_map: FxHashMap<NodeOrdinal, Vec<NodeOrdinal>> = FxHashMap::default();

    for ord_idx in 0..snap.node_count() {
        let ord = NodeOrdinal(ord_idx);
        if snap.is_context(ord) {
            if let Some(parent) = snap.find_edge_target(ord, "previous") {
                children_map.entry(parent).or_default().push(ord);
            }
        } else if snap.is_js_function(ord) {
            if let Some(ctx) = snap.find_edge_target(ord, "context") {
                closures_map.entry(ctx).or_default().push(ord);
            }
        }
    }

    let subtree_closures = precompute_subtree_closures(&children_map, &closures_map);
    let mut script_cache: FxHashMap<NodeOrdinal, Option<Vec<ScopeInfo>>> = FxHashMap::default();
    let empty = Vec::new();
    let mut leaks = Vec::new();

    for &ord in contexts {
        let vars = snap.context_variable_names(ord);
        if vars.is_empty() {
            continue;
        }

        let closure_depths = subtree_closures.get(&ord).unwrap_or(&empty);
        let result = find_unused_vars(snap, &vars, closure_depths, &mut script_cache);

        if !matches!(&result, UnusedVarsResult::Complete(u) if u.is_empty()) {
            leaks.push(ContextLeak {
                context_ord: ord,
                result,
            });
        }
    }

    leaks
}

pub enum UnusedVarsResult {
    /// Analysis completed: list of unused variable names.
    Complete(Vec<String>),
    /// Analysis could not be completed for the given reason.
    Incomplete(String),
}

/// Find unused context variables for a single context.
/// `closure_depths` is the precomputed list of (closure_ordinal, depth) pairs
/// for this context's subtree.
pub fn find_unused_vars(
    snap: &HeapSnapshot,
    vars: &[String],
    closure_depths: &[(NodeOrdinal, u32)],
    script_cache: &mut FxHashMap<NodeOrdinal, Option<Vec<ScopeInfo>>>,
) -> UnusedVarsResult {
    let mut accessed: FxHashSet<String> = FxHashSet::default();

    for &(func_ord, ctx_depth) in closure_depths {
        let func_id = snap.node_id(func_ord);

        let Some(sfi) = snap.find_edge_target(func_ord, "shared") else {
            return UnusedVarsResult::Incomplete(format!(
                "closure @{func_id} has no SharedFunctionInfo"
            ));
        };
        let sfi_id = snap.node_id(sfi);

        let Some(script_ord) = snap.find_edge_target(sfi, "script") else {
            return UnusedVarsResult::Incomplete(format!("SFI @{sfi_id} has no script"));
        };
        let script_id = snap.node_id(script_ord);

        let scopes = script_cache.entry(script_ord).or_insert_with(|| {
            snap.script_source(script_ord)
                .and_then(|src| extract_scopes(src).ok())
        });

        let Some(scopes) = scopes else {
            return UnusedVarsResult::Incomplete(format!(
                "script @{script_id} source is missing or could not be parsed"
            ));
        };

        let Some(start_pos) = snap.int_edge_value(sfi, "start_position") else {
            return UnusedVarsResult::Incomplete(format!("SFI @{sfi_id} has no start_position"));
        };
        if start_pos < 0 {
            return UnusedVarsResult::Incomplete(format!(
                "SFI @{sfi_id} has negative start_position ({start_pos})"
            ));
        }

        // If the closure isn't in any scope returned by extract_scopes, it doesn't
        // capture any outer variables — safe to skip without marking incomplete.
        let Some(scope) = find_scope_for_position(scopes, start_pos as u32) else {
            continue;
        };

        for cv in &scope.context_variables {
            if cv.depth == ctx_depth && vars.contains(&cv.name) {
                accessed.insert(cv.name.clone());
            }
        }
    }

    UnusedVarsResult::Complete(
        vars.iter()
            .filter(|v| !accessed.contains(v.as_str()))
            .cloned()
            .collect(),
    )
}

/// Precompute for every context its full list of (closure, depth) pairs
/// by traversing the context tree bottom-up. Each context is processed
/// exactly once: its list is its direct closures (depth 0) plus all
/// children's lists with depth incremented by 1.
pub fn precompute_subtree_closures(
    children_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
    closures_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
) -> FxHashMap<NodeOrdinal, Vec<(NodeOrdinal, u32)>> {
    // Collect all context nodes that appear in either map.
    let mut all_contexts: FxHashSet<NodeOrdinal> = FxHashSet::default();
    for (&parent, children) in children_map {
        all_contexts.insert(parent);
        for &child in children {
            all_contexts.insert(child);
        }
    }
    for &ctx in closures_map.keys() {
        all_contexts.insert(ctx);
    }

    let mut result: FxHashMap<NodeOrdinal, Vec<(NodeOrdinal, u32)>> = FxHashMap::default();

    // Iterative post-order: push each context twice — first unprocessed,
    // then processed. On the processed visit all children are already in
    // the result map.
    for ctx in all_contexts {
        if result.contains_key(&ctx) {
            continue;
        }
        let mut stack: Vec<(NodeOrdinal, bool)> = vec![(ctx, false)];
        while let Some((node, processed)) = stack.pop() {
            if result.contains_key(&node) {
                continue;
            }
            if !processed {
                stack.push((node, true));
                if let Some(kids) = children_map.get(&node) {
                    for &kid in kids {
                        if !result.contains_key(&kid) {
                            stack.push((kid, false));
                        }
                    }
                }
                continue;
            }

            let mut depths = Vec::new();
            if let Some(funcs) = closures_map.get(&node) {
                depths.extend(funcs.iter().map(|&f| (f, 0u32)));
            }
            if let Some(kids) = children_map.get(&node) {
                for &kid in kids {
                    let kid_depths = &result[&kid];
                    depths.extend(kid_depths.iter().map(|&(f, d)| (f, d + 1)));
                }
            }
            result.insert(node, depths);
        }
    }

    result
}

fn find_scope_for_position(scopes: &[ScopeInfo], pos: u32) -> Option<&ScopeInfo> {
    scopes
        .iter()
        .filter(|s| s.span.utf16_start <= pos && pos < s.span.utf16_end)
        .min_by_key(|s| s.span.utf16_end - s.span.utf16_start)
}

/// Classify a context: check if it belongs to builtins, FunctionTemplateInfo,
/// extension scripts, or extension NativeContexts, by inspecting closures that
/// reference it.
fn classify_context(
    snap: &HeapSnapshot,
    context_ord: NodeOrdinal,
    closures_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
) -> ContextClass {
    if let Some(funcs) = closures_map.get(&context_ord) {
        for &func_ord in funcs {
            let Some(sfi) = snap.find_edge_target(func_ord, "shared") else {
                continue;
            };

            let has_builtin_id = snap.int_edge_value(sfi, "builtin_id").is_some();
            let has_fti = snap
                .find_edge_target(sfi, "untrusted_function_data")
                .is_some_and(|t| snap.node_raw_name(t).contains("FunctionTemplateInfo"));
            let is_ext_script = snap.find_edge_target(sfi, "script").is_some_and(|script| {
                snap.find_edge_target(script, "script_type_name")
                    .is_some_and(|t| snap.node_raw_name(t) == "extension")
            });

            let is_ext_nc = snap
                .find_native_context_for_context(context_ord)
                .and_then(|nc| snap.native_context_url(nc))
                .is_some_and(|url| url.starts_with("chrome-extension://"));

            return ContextClass {
                has_builtin_id,
                has_function_template_info: has_fti,
                is_extension_script: is_ext_script,
                is_extension_native_context: is_ext_nc,
            };
        }
    }

    // No closure found — check NativeContext URL anyway.
    let is_ext_nc = snap
        .find_native_context_for_context(context_ord)
        .and_then(|nc| snap.native_context_url(nc))
        .is_some_and(|url| url.starts_with("chrome-extension://"));

    ContextClass {
        has_builtin_id: false,
        has_function_template_info: false,
        is_extension_script: false,
        is_extension_native_context: is_ext_nc,
    }
}

struct ContextClass {
    has_builtin_id: bool,
    has_function_template_info: bool,
    is_extension_script: bool,
    is_extension_native_context: bool,
}

/// Collect all non-NativeContext contexts with variables, applying the given filters.
pub fn collect_contexts(
    snap: &HeapSnapshot,
    show_builtins: bool,
    show_function_template_info: bool,
    show_extensions: bool,
) -> Vec<NodeOrdinal> {
    let mut closures_map: FxHashMap<NodeOrdinal, Vec<NodeOrdinal>> = FxHashMap::default();
    for ord_idx in 0..snap.node_count() {
        let ord = NodeOrdinal(ord_idx);
        if snap.is_js_function(ord) {
            if let Some(ctx) = snap.find_edge_target(ord, "context") {
                closures_map.entry(ctx).or_default().push(ord);
            }
        }
    }

    let mut contexts = Vec::new();
    for ord_idx in 0..snap.node_count() {
        let ord = NodeOrdinal(ord_idx);
        if !snap.is_context(ord) || snap.is_native_context(ord) {
            continue;
        }
        if snap.context_variable_names(ord).is_empty() {
            continue;
        }
        let cls = classify_context(snap, ord, &closures_map);
        if !show_builtins && cls.has_builtin_id {
            continue;
        }
        if !show_function_template_info && cls.has_function_template_info {
            continue;
        }
        if !show_extensions && (cls.is_extension_script || cls.is_extension_native_context) {
            continue;
        }
        contexts.push(ord);
    }
    contexts
}

pub fn print_closure_leaks(
    snap: &HeapSnapshot,
    show_builtins: bool,
    show_function_template_info: bool,
    show_extensions: bool,
    show_incomplete: bool,
) {
    let contexts = collect_contexts(
        snap,
        show_builtins,
        show_function_template_info,
        show_extensions,
    );
    let mut leaks = find_closure_leaks(snap, &contexts);

    if !show_incomplete {
        leaks.retain(|l| matches!(&l.result, UnusedVarsResult::Complete(_)));
    }

    if leaks.is_empty() {
        println!("No closure leaks detected.");
        return;
    }

    // Sort by retained size descending.
    leaks.sort_by(|a, b| {
        snap.node_retained_size(b.context_ord)
            .partial_cmp(&snap.node_retained_size(a.context_ord))
            .unwrap()
    });

    for leak in &leaks {
        let id = snap.node_id(leak.context_ord);
        let retained = snap.node_retained_size(leak.context_ord);
        let all_vars = snap.context_variable_names(leak.context_ord);

        let loc = find_context_location(snap, leak.context_ord);
        let loc_str = loc.as_deref().unwrap_or("?");

        println!(
            "@{id} ({loc_str}, retained: {})  vars: [{}]",
            format_size(retained),
            all_vars.join(", "),
        );

        match &leak.result {
            UnusedVarsResult::Incomplete(reason) => {
                println!("  (incomplete: {reason})");
            }
            UnusedVarsResult::Complete(unused) => {
                let var_targets = context_variable_targets(snap, leak.context_ord);
                for name in unused {
                    if let Some(&target_ord) = var_targets.get(name) {
                        let target_id = snap.node_id(target_ord);
                        let target_name = snap.node_display_name(target_ord);
                        let target_retained = snap.node_retained_size(target_ord);
                        println!(
                            "  unused: {name}: @{target_id} {target_name} (retained: {})",
                            format_size(target_retained),
                        );
                    } else {
                        println!("  unused: {name}");
                    }
                }
            }
        }
    }

    println!("\n{} contexts with unused variables", leaks.len());
}

/// Get a map from context variable name to the target node ordinal for a Context node.
fn context_variable_targets(
    snap: &HeapSnapshot,
    context_ord: NodeOrdinal,
) -> FxHashMap<String, NodeOrdinal> {
    let mut map = FxHashMap::default();
    for (edge_idx, child_ord) in snap.iter_edges(context_ord) {
        if snap.edge_type_name(edge_idx) == "context" {
            let name = snap.edge_name(edge_idx);
            if name != "this" {
                map.insert(name, child_ord);
            }
        }
    }
    map
}

/// Try to find a source location for a context by looking at closures that reference it.
fn find_context_location(snap: &HeapSnapshot, context_ord: NodeOrdinal) -> Option<String> {
    for ord_idx in 0..snap.node_count() {
        let ord = NodeOrdinal(ord_idx);
        if !snap.is_js_function(ord) {
            continue;
        }
        if snap.find_edge_target(ord, "context") != Some(context_ord) {
            continue;
        }
        if let Some(loc) = snap.node_location(ord) {
            return Some(snap.format_location(&loc));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RawHeapSnapshot, SnapshotHeader, SnapshotMeta};

    // JS source used by tests. `outer` creates a context with [a, b].
    // `f` captures `a`, `g` captures `b`.
    const SOURCE: &str =
        "function outer(){var a=1;var b=2;function f(){return a}function g(){return b}}";
    // Position inside f's body (the `{` at offset 45)
    const F_START: &str = "45";
    const F_END: &str = "55";
    // Position inside g's body (the `{` at offset 67)
    const G_START: &str = "67";
    const G_END: &str = "77";

    fn s(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn build_leak_snapshot(closures: &[(&str, &str)]) -> HeapSnapshot {
        build_leak_snapshot_with_source(SOURCE, closures)
    }

    /// Build a snapshot with a Context holding variables [a, b] and the given
    /// list of live closures. Each closure is (start_position, end_position)
    /// pointing into the source.
    fn build_leak_snapshot_with_source(source: &str, closures: &[(&str, &str)]) -> HeapSnapshot {
        let nfc = 5u32;
        let n = |ord: u32| ord * nfc;

        // --- strings ---
        //  0: ""
        //  1: "(GC roots)"
        //  2: "system / NativeContext / test"
        //  3: "system / Context"
        //  4: "f"
        //  5: "system / SharedFunctionInfo"
        //  6: "system / Script / test.js"
        //  7: SOURCE
        //  8: "a"
        //  9: "b"
        // 10: "shared"
        // 11: "context"
        // 12: "script"
        // 13: "start_position"
        // 14: "end_position"
        // 15: "value"
        // 16: "previous"
        // 17: "scope_info"
        // 18: "system / ScopeInfo"
        // 19: "int"
        // 20: "nc"
        // 21: "func"
        // 22: "obj"
        // 23: "source"
        // 24+: start/end position value strings for each closure

        let mut strings = s(&[
            "",
            "(GC roots)",
            "system / NativeContext / test",
            "system / Context",
            "f",
            "system / SharedFunctionInfo",
            "system / Script / test.js",
            source,
            "a",
            "b",
            "shared",
            "context",
            "script",
            "start_position",
            "end_position",
            "value",
            "previous",
            "scope_info",
            "system / ScopeInfo",
            "int",
            "nc",
            "func",
            "obj",
            "source",
        ]);

        // Add position value strings for each closure
        let pos_str_base = strings.len() as u32;
        for (start, end) in closures {
            strings.push(start.to_string());
            strings.push(end.to_string());
        }

        // --- fixed nodes (0..7) ---
        // Node 0: synthetic root (1 edge → GC roots)
        // Node 1: (GC roots) (2 + closures.len() edges)
        // Node 2: NativeContext (0 edges)
        // Node 3: Context (4 edges: a, b, previous, scope_info)
        // Node 4: Script (1 edge: source)
        // Node 5: source string (0 edges)
        // Node 6: ScopeInfo (2 edges: hidden→"a", hidden→"b")
        // Node 7: string "a" (0 edges)
        // Node 8: string "b" (0 edges)
        // Node 9: obj_a (0 edges)
        // Node 10: obj_b (0 edges)
        let fixed_nodes = 11u32;

        // Per closure: JSFunction (2 edges), SFI (3 edges),
        //              int_start (1 edge), str_start (0), int_end (1 edge), str_end (0)
        // = 6 nodes per closure
        let gc_roots_edges = 1 + closures.len() as u32; // → NativeContext + each JSFunction

        let mut nodes = vec![
            9,
            0,
            1,
            0,
            1, // node 0: synthetic root
            9,
            1,
            2,
            0,
            gc_roots_edges, // node 1: GC roots
            8,
            2,
            3,
            100,
            0, // node 2: NativeContext
            3,
            3,
            5,
            24,
            4, // node 3: Context
            4,
            6,
            7,
            80,
            1, // node 4: Script
            2,
            7,
            9,
            100,
            0, // node 5: source string
            4,
            18,
            11,
            56,
            2, // node 6: ScopeInfo
            2,
            8,
            13,
            0,
            0, // node 7: string "a"
            2,
            9,
            15,
            0,
            0, // node 8: string "b"
            3,
            22,
            17,
            16,
            0, // node 9: obj_a
            3,
            22,
            19,
            16,
            0, // node 10: obj_b
        ];

        // Per-closure nodes
        let mut next_id = 21u32;
        for _ in closures {
            // JSFunction (closure type=5, 2 edges: shared, context)
            nodes.extend_from_slice(&[5, 4, next_id, 32, 2]);
            next_id += 2;
            // SFI (code type=4, 3 edges: script, start_position, end_position)
            nodes.extend_from_slice(&[4, 5, next_id, 48, 3]);
            next_id += 2;
            // int start_pos (number type=7, 1 edge: value)
            nodes.extend_from_slice(&[7, 19, next_id, 0, 1]);
            next_id += 2;
            // str start_pos value (string type=2, 0 edges)
            nodes.extend_from_slice(&[2, 0, next_id, 0, 0]); // name filled via edge
            next_id += 2;
            // int end_pos (number type=7, 1 edge: value)
            nodes.extend_from_slice(&[7, 19, next_id, 0, 1]);
            next_id += 2;
            // str end_pos value (string type=2, 0 edges)
            nodes.extend_from_slice(&[2, 0, next_id, 0, 0]);
            next_id += 2;
        }

        // Fix string node names for position value strings
        for (i, _) in closures.iter().enumerate() {
            let base_node = fixed_nodes + i as u32 * 6;
            let start_str_node = base_node + 3; // str start_pos value
            let end_str_node = base_node + 5; // str end_pos value
            let start_str_idx = pos_str_base + i as u32 * 2;
            let end_str_idx = pos_str_base + i as u32 * 2 + 1;
            // Set name field (offset 1 in node fields)
            nodes[(start_str_node * nfc + 1) as usize] = start_str_idx;
            nodes[(end_str_node * nfc + 1) as usize] = end_str_idx;
        }

        // --- edges ---
        let mut edges: Vec<u32> = vec![
            1,
            0,
            n(1), // root → GC roots (element)
            2,
            20,
            n(2), // GC roots → NativeContext (property "nc")
        ];

        // GC roots → each JSFunction
        for i in 0..closures.len() as u32 {
            let func_node = fixed_nodes + i * 6;
            edges.extend_from_slice(&[2, 21, n(func_node)]); // property "func"
        }

        // Context edges
        edges.extend_from_slice(&[
            0,
            8,
            n(9), // context "a" → obj_a
            0,
            9,
            n(10), // context "b" → obj_b
            3,
            16,
            n(2), // internal "previous" → NativeContext
            3,
            17,
            n(6), // internal "scope_info" → ScopeInfo
        ]);

        // Script edge
        edges.extend_from_slice(&[
            3,
            23,
            n(5), // internal "source" → source string
        ]);

        // ScopeInfo hidden edges
        edges.extend_from_slice(&[
            4,
            0,
            n(7), // hidden 0 → string "a"
            4,
            1,
            n(8), // hidden 1 → string "b"
        ]);

        // Per-closure edges
        for i in 0..closures.len() as u32 {
            let base = fixed_nodes + i * 6;
            let sfi = base + 1;
            let int_start = base + 2;
            let str_start = base + 3;
            let int_end = base + 4;
            let str_end = base + 5;

            // JSFunction edges
            edges.extend_from_slice(&[
                3,
                10,
                n(sfi), // internal "shared" → SFI
                3,
                11,
                n(3), // internal "context" → Context
            ]);

            // SFI edges
            edges.extend_from_slice(&[
                3,
                12,
                n(4), // internal "script" → Script
                3,
                13,
                n(int_start), // internal "start_position" → int
                3,
                14,
                n(int_end), // internal "end_position" → int
            ]);

            // int start_pos → value
            edges.extend_from_slice(&[
                3,
                15,
                n(str_start), // internal "value"
            ]);

            // int end_pos → value
            edges.extend_from_slice(&[
                3,
                15,
                n(str_end), // internal "value"
            ]);
        }

        let raw = RawHeapSnapshot {
            snapshot: SnapshotHeader {
                meta: SnapshotMeta {
                    node_fields: s(&["type", "name", "id", "self_size", "edge_count"]),
                    node_type_enum: s(&[
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
                    ]),
                    edge_fields: s(&["type", "name_or_index", "to_node"]),
                    edge_type_enum: s(&[
                        "context", "element", "property", "internal", "hidden", "shortcut", "weak",
                    ]),
                    location_fields: vec![],
                    sample_fields: vec![],
                    trace_function_info_fields: vec![],
                    trace_node_fields: vec![],
                },
                node_count: nodes.len() / nfc as usize,
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

    fn all_contexts(snap: &HeapSnapshot) -> Vec<NodeOrdinal> {
        (0..snap.node_count())
            .map(NodeOrdinal)
            .filter(|&ord| snap.is_context(ord) && !snap.is_native_context(ord))
            .collect()
    }

    fn leak_var_names(leaks: &[ContextLeak]) -> Vec<Vec<&str>> {
        leaks
            .iter()
            .map(|l| match &l.result {
                UnusedVarsResult::Complete(unused) => unused.iter().map(|s| s.as_str()).collect(),
                UnusedVarsResult::Incomplete(_) => vec![],
            })
            .collect()
    }

    fn is_incomplete(leak: &ContextLeak) -> bool {
        matches!(&leak.result, UnusedVarsResult::Incomplete(_))
    }

    #[test]
    fn test_simple_leak() {
        // Only f is alive (captures a), b is unused.
        let snap = build_leak_snapshot(&[(F_START, F_END)]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        assert_eq!(leak_var_names(&leaks), vec![vec!["b"]]);
    }

    #[test]
    fn test_no_leak_both_closures_alive() {
        // Both f (captures a) and g (captures b) are alive.
        let snap = build_leak_snapshot(&[(F_START, F_END), (G_START, G_END)]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert!(
            leaks.is_empty(),
            "expected no leaks, got: {leaks:?}",
            leaks = leak_var_names(&leaks)
        );
    }

    #[test]
    fn test_no_closures_all_unused() {
        // No closures reference the context → all variables unused.
        let snap = build_leak_snapshot(&[]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        let names = leak_var_names(&leaks);
        assert!(names[0].contains(&"a"));
        assert!(names[0].contains(&"b"));
    }

    #[test]
    fn test_only_g_alive() {
        // Only g is alive (captures b), a is unused.
        let snap = build_leak_snapshot(&[(G_START, G_END)]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        assert_eq!(leak_var_names(&leaks), vec![vec!["a"]]);
    }

    #[test]
    fn test_unparseable_source_marked_incomplete() {
        // Source contains V8-only syntax that the JS parser can't handle.
        // Should be marked incomplete rather than reporting false unused vars.
        let snap = build_leak_snapshot_with_source(
            "function outer(){native function Apply();function f(){return Apply}}",
            &[("45", "55")],
        );
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        assert!(is_incomplete(&leaks[0]), "should be marked incomplete");
    }

    #[test]
    fn test_no_source_marked_incomplete() {
        // Script has no source (empty string). Should be marked incomplete.
        let snap = build_leak_snapshot_with_source("", &[("0", "0")]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        assert!(is_incomplete(&leaks[0]), "should be marked incomplete");
    }

    #[test]
    fn test_non_capturing_closure_does_not_cause_incomplete() {
        // `noop` is at the top level, outside `outer`'s scope span [0, 56).
        // extract_scopes won't return any scope covering noop's position,
        // so find_scope_for_position returns None for it. This should NOT
        // mark the context as incomplete — noop simply doesn't capture anything.
        let source =
            "function outer(){var a=1;var b=2;function f(){return a}}function noop(){return 1}";
        // f body at 45 (inside outer), noop body at 71 (outside outer)
        let snap = build_leak_snapshot_with_source(source, &[("45", "55"), ("71", "81")]);
        let leaks = find_closure_leaks(&snap, &all_contexts(&snap));
        assert_eq!(leaks.len(), 1);
        assert!(
            !is_incomplete(&leaks[0]),
            "non-capturing closure should not cause incomplete"
        );
        assert_eq!(leak_var_names(&leaks), vec![vec!["b"]]);
    }
}
