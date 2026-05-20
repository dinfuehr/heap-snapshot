use super::format_size;
use crate::snapshot::HeapSnapshot;

pub fn print_roots(snap: &HeapSnapshot) {
    // System roots
    println!("System roots ({}):", snap.system_roots().len());
    for &child_ord in snap.system_roots() {
        let label = snap.format_node_label(child_ord);
        let self_size = snap.node_self_size(child_ord) as u64;
        let retained = snap.node_retained_size(child_ord);
        let child_count = snap.node_edge_count(child_ord);

        println!(
            "  {label}  (self: {}, retained: {}, {} children)",
            format_size(self_size),
            format_size(retained),
            child_count,
        );

        // For (GC roots), list its children (root categories).
        if snap.is_root(child_ord) {
            for (_ei, cat_ord) in snap.iter_edges(child_ord) {
                let cat_label = snap.format_node_label(cat_ord);
                let cat_self = snap.node_self_size(cat_ord) as u64;
                let cat_retained = snap.node_retained_size(cat_ord);
                let cat_children = snap.node_edge_count(cat_ord);
                println!(
                    "    {cat_label}  (self: {}, retained: {}, {} children)",
                    format_size(cat_self),
                    format_size(cat_retained),
                    cat_children,
                );
            }
        }
    }

    // User roots
    println!("\nUser roots ({}):", snap.user_roots().len());
    for &child_ord in snap.user_roots() {
        let label = snap.format_node_label(child_ord);
        let self_size = snap.node_self_size(child_ord) as u64;
        let retained = snap.node_retained_size(child_ord);
        let child_count = snap.node_edge_count(child_ord);

        // If this is a native context, show the full context label.
        let ctx_info = if snap.is_native_context(child_ord) {
            format!("  {}", snap.native_context_label(child_ord))
        } else {
            String::new()
        };

        println!(
            "  {label}{ctx_info}  (self: {}, retained: {}, {} children)",
            format_size(self_size),
            format_size(retained),
            child_count,
        );
    }
}
