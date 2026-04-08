use rustc_hash::FxHashMap;

use crate::function_info::ScopeInfo;
use crate::print::closure_leaks::{
    UnusedVarsResult, find_unused_vars, precompute_subtree_closures,
};
use crate::print::format_size;
use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

pub fn print_context_tree(snap: &HeapSnapshot, node_id: NodeId) {
    let ordinal = match snap.node_for_snapshot_object_id(node_id) {
        Some(o) => o,
        None => {
            println!("Error: no node found with id @{node_id}");
            std::process::exit(1);
        }
    };

    if !snap.is_context(ordinal) {
        println!("Error: @{node_id} is not a Context node");
        std::process::exit(1);
    }

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

    print_node(
        snap,
        ordinal,
        0,
        &children_map,
        &closures_map,
        &subtree_closures,
        &empty,
        &mut script_cache,
    );
}

fn print_node(
    snap: &HeapSnapshot,
    ordinal: NodeOrdinal,
    depth: usize,
    children_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
    closures_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
    subtree_closures: &FxHashMap<NodeOrdinal, Vec<(NodeOrdinal, u32)>>,
    empty: &Vec<(NodeOrdinal, u32)>,
    script_cache: &mut FxHashMap<NodeOrdinal, Option<Vec<ScopeInfo>>>,
) {
    let indent = "  ".repeat(depth);
    let id = snap.node_id(ordinal);
    let name = snap.node_display_name(ordinal);
    let self_size = snap.node_self_size(ordinal);
    let retained = snap.node_retained_size(ordinal);

    let vars = snap.context_variable_names(ordinal);
    let vars_str = if vars.is_empty() {
        String::new()
    } else {
        format!(" [{}]", vars.join(", "))
    };

    let scope_info_str = match snap.find_edge_target(ordinal, "scope_info") {
        Some(si) => format!(" scope_info=@{}", snap.node_id(si)),
        None => String::new(),
    };

    println!(
        "{indent}@{id} {name} (self_size: {}, retained: {}){vars_str}{scope_info_str}",
        format_size(self_size as f64),
        format_size(retained),
    );

    if let Some(funcs) = closures_map.get(&ordinal) {
        for &func_ord in funcs {
            let func_id = snap.node_id(func_ord);
            let func_name = snap.node_display_name(func_ord);
            let loc = snap
                .node_location(func_ord)
                .map(|l| snap.format_location(&l));
            let loc_str = loc.as_deref().unwrap_or("?");
            println!("{indent}  -> @{func_id} {func_name} ({loc_str})");
        }
    }

    if !snap.is_native_context(ordinal) && !vars.is_empty() {
        let closure_depths = subtree_closures.get(&ordinal).unwrap_or(empty);
        let result = find_unused_vars(snap, &vars, closure_depths, script_cache);
        match result {
            UnusedVarsResult::Complete(unused) if !unused.is_empty() => {
                println!(
                    "{indent}  ! unused context variables: {}",
                    unused.join(", ")
                );
            }
            UnusedVarsResult::Incomplete(reason) => {
                println!("{indent}  (incomplete: {reason})");
            }
            _ => {}
        }
    }

    if let Some(kids) = children_map.get(&ordinal) {
        for &child in kids {
            print_node(
                snap,
                child,
                depth + 1,
                children_map,
                closures_map,
                subtree_closures,
                empty,
                script_cache,
            );
        }
    }
}
