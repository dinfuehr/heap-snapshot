use crate::snapshot::HeapSnapshot;

use super::{format_count, format_size};

pub fn print_statistics(snap: &HeapSnapshot) {
    let stats = snap.get_statistics();
    println!("Statistics (total {}):", format_size(stats.total));
    println!("  V8 Heap:        {}", format_size(stats.v8heap_total));
    println!("    Code:         {}", format_size(stats.code));
    println!("    Strings:      {}", format_size(stats.strings));
    println!("    JS Arrays:    {}", format_size(stats.js_arrays));
    println!("    System:       {}", format_size(stats.system));
    println!("  Native:         {}", format_size(stats.native_total));
    println!("    Typed Arrays: {}", format_size(stats.typed_arrays));
    println!(
        "    Extra Native: {}",
        format_size(stats.extra_native_bytes)
    );
    println!(
        "  Unreachable:    {} ({} objects)",
        format_size(stats.unreachable_size),
        format_count(stats.unreachable_count),
    );

    let contexts = snap.native_contexts();
    if !contexts.is_empty() {
        println!();
        println!("Native Context Attribution:");
        for ctx in contexts {
            let label = snap.native_context_label(ctx.ordinal);
            println!("  {:<40} {}", label, format_size(ctx.size));
        }
        println!(
            "  {:<40} {}",
            "Shared",
            format_size(snap.shared_attributable_size())
        );
        println!(
            "  {:<40} {}",
            "Unattributed",
            format_size(snap.unattributed_size())
        );
    }
}
