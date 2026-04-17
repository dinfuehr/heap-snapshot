use super::format_size;
use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

pub fn print_show(snap: &HeapSnapshot, node_id: NodeId, depth: usize, offset: usize, limit: usize) {
    let ordinal = match snap.node_for_snapshot_object_id(node_id) {
        Some(o) => o,
        None => {
            println!("Error: no node found with id @{node_id}");
            std::process::exit(1);
        }
    };

    let self_size = snap.node_self_size(ordinal) as u64;
    let retained = snap.node_retained_size(ordinal);
    println!("id:           @{node_id}");
    println!("ordinal:      {}", ordinal.0);
    println!("type:         {}", snap.node_type_name(ordinal));
    println!("name:         {}", snap.node_display_name(ordinal));
    println!("class:        {}", snap.node_class_name(ordinal));
    println!("self size:    {} ({self_size})", format_size(self_size));
    println!("retained:     {} ({retained})", format_size(retained));
    println!("distance:     {}", snap.node_distance(ordinal));
    println!("detachedness: {:?}", snap.node_detachedness(ordinal));
    println!("edge count:   {}", snap.node_edge_count(ordinal));

    if let Some(stack) = snap.get_allocation_stack(ordinal) {
        println!("  Allocated at:");
        for frame in &stack {
            println!("    {}", HeapSnapshot::format_allocation_frame(frame));
        }
    }

    if let Some(source) = snap.shared_function_info_source(ordinal) {
        println!("  Source:");
        for line in source.lines() {
            println!("    {line}");
        }
    }

    show_edges(snap, ordinal, 1, depth, offset, limit);
}

fn show_edges(
    snap: &HeapSnapshot,
    ordinal: NodeOrdinal,
    cur_depth: usize,
    max_depth: usize,
    offset: usize,
    limit: usize,
) {
    let indent = "  ".repeat(cur_depth);
    let edges: Vec<_> = snap.iter_edges(ordinal).collect();
    let total = edges.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);

    for &(edge_idx, child_ord) in &edges[start..end] {
        let edge_type = snap.edge_type_name(edge_idx);
        let edge_name = snap.edge_name(edge_idx);
        let child_id = snap.node_id(child_ord);
        let child_name = snap.node_display_name(child_ord);
        let child_type = snap.node_type_name(child_ord);
        let child_size = snap.node_self_size(child_ord);

        println!(
            "{indent}--[{edge_type} \"{edge_name}\"]--> @{} {child_name} (type: {child_type}, self_size: {child_size})",
            child_id.0
        );

        if cur_depth < max_depth {
            show_edges(snap, child_ord, cur_depth + 1, max_depth, 0, limit);
        }
    }

    if end < total {
        println!("{indent}({}-{} of {total} children shown)", start + 1, end);
    }
}
