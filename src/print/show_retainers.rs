use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

pub fn print_show_retainers(
    snap: &HeapSnapshot,
    node_id: NodeId,
    depth: usize,
    offset: usize,
    limit: usize,
) {
    let ordinal = match snap.node_for_snapshot_object_id(node_id) {
        Some(o) => o,
        None => {
            println!("Error: no node found with id @{node_id}");
            std::process::exit(1);
        }
    };

    println!(
        "Object @{node_id}: {} (type: {}, self_size: {}, retained_size: {:.0})",
        snap.node_display_name(ordinal),
        snap.node_type_name(ordinal),
        snap.node_self_size(ordinal),
        snap.node_retained_size(ordinal),
    );

    show_retainers(snap, ordinal, 1, depth, offset, limit);
}

fn show_retainers(
    snap: &HeapSnapshot,
    ordinal: NodeOrdinal,
    cur_depth: usize,
    max_depth: usize,
    offset: usize,
    limit: usize,
) {
    let indent = "  ".repeat(cur_depth);
    let retainers = snap.get_retainers(ordinal);
    let total = retainers.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);

    for &(edge_idx, ret_ord) in &retainers[start..end] {
        let edge_type = snap.edge_type_name(edge_idx);
        let edge_name = snap.edge_name(edge_idx);
        let ret_id = snap.node_id(ret_ord);
        let ret_name = snap.node_display_name(ret_ord);
        let ret_type = snap.node_type_name(ret_ord);
        let ret_size = snap.node_self_size(ret_ord);

        println!(
            "{indent}<--[{edge_type} \"{edge_name}\"]-- @{} {ret_name} (type: {ret_type}, self_size: {ret_size})",
            ret_id.0
        );

        if cur_depth < max_depth {
            show_retainers(snap, ret_ord, cur_depth + 1, max_depth, 0, limit);
        }
    }

    if end < total {
        println!("{indent}({}-{} of {total} retainers shown)", start + 1, end);
    }
}
