use rustc_hash::FxHashMap;

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

    // Build parent -> children map by scanning all nodes for Context nodes
    // and following their "previous" edge.
    let mut children_map: FxHashMap<NodeOrdinal, Vec<NodeOrdinal>> = FxHashMap::default();
    for ord_idx in 0..snap.node_count() {
        let ord = NodeOrdinal(ord_idx);
        if !snap.is_context(ord) {
            continue;
        }
        if let Some(parent) = snap.find_edge_target(ord, "previous") {
            children_map.entry(parent).or_default().push(ord);
        }
    }

    print_node(snap, ordinal, 0, &children_map);
}

fn print_node(
    snap: &HeapSnapshot,
    ordinal: NodeOrdinal,
    depth: usize,
    children_map: &FxHashMap<NodeOrdinal, Vec<NodeOrdinal>>,
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

    println!(
        "{indent}@{id} {name} (self_size: {}, retained: {}){vars_str}",
        format_size(self_size as f64),
        format_size(retained),
    );

    if let Some(kids) = children_map.get(&ordinal) {
        for &child in kids {
            print_node(snap, child, depth + 1, children_map);
        }
    }
}
