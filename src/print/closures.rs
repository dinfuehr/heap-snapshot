use crate::print::format_size;
use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

use crate::types::NodeId;

pub fn print_closures(
    snap: &HeapSnapshot,
    offset: usize,
    limit: usize,
    show_builtins: bool,
    show_function_template_info: bool,
    show_extensions: bool,
    filter_native_context: Option<NodeId>,
) {
    let mut entries: Vec<ClosureEntry> = Vec::new();

    for idx in 0..snap.node_count() {
        let ord = NodeOrdinal(idx);
        if !snap.is_js_function(ord) {
            continue;
        }

        let name = snap.node_display_name(ord);
        let id = snap.node_id(ord);
        let retained = snap.node_retained_size(ord);

        let location = snap
            .node_location(ord)
            .map(|loc| snap.format_location(&loc));

        let sfi_ord = snap.find_edge_target(ord, "shared");

        let has_builtin_id = sfi_ord
            .and_then(|o| snap.int_edge_value(o, "builtin_id"))
            .is_some();
        let has_function_template_info = sfi_ord.is_some_and(|sfi| {
            snap.find_edge_target(sfi, "untrusted_function_data")
                .is_some_and(|t| snap.node_raw_name(t).contains("FunctionTemplateInfo"))
        });
        let is_extension_script = sfi_ord.is_some_and(|sfi| {
            snap.find_edge_target(sfi, "script").is_some_and(|script| {
                snap.find_edge_target(script, "script_type_name")
                    .is_some_and(|t| snap.node_raw_name(t) == "extension")
            })
        });

        let sfi_id = sfi_ord.map(|o| snap.node_id(o));

        let context_ord = snap.find_edge_target(ord, "context");
        let context_id = context_ord.map(|o| snap.node_id(o));
        let context_vars = context_ord
            .map(|o| snap.context_variable_names(o))
            .unwrap_or_default();

        let native_context_ord =
            context_ord.and_then(|ctx| snap.find_native_context_for_context(ctx));

        let is_extension_native_context = native_context_ord
            .and_then(|nc| snap.native_context_url(nc))
            .is_some_and(|url| url.starts_with("chrome-extension://"));

        let native_context_id = native_context_ord.map(|o| snap.node_id(o));

        entries.push(ClosureEntry {
            name,
            id: id.0,
            retained,
            location,
            sfi_id: sfi_id.map(|n| n.0),
            context_id: context_id.map(|n| n.0),
            context_vars,
            has_builtin_id,
            has_function_template_info,
            is_extension_script,
            is_extension_native_context,
            native_context_id,
        });
    }

    if !show_builtins {
        entries.retain(|e| !e.has_builtin_id);
    }
    if !show_function_template_info {
        entries.retain(|e| !e.has_function_template_info);
    }
    if !show_extensions {
        entries.retain(|e| !e.is_extension_script && !e.is_extension_native_context);
    }
    if let Some(nc_id) = filter_native_context {
        entries.retain(|e| e.native_context_id == Some(nc_id));
    }

    entries.sort_by(|a, b| b.retained.partial_cmp(&a.retained).unwrap());

    let total = entries.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);

    for entry in &entries[start..end] {
        let loc = entry.location.as_deref().unwrap_or("?");

        print!(
            "@{} {} ({}, retained: {})",
            entry.id,
            entry.name,
            loc,
            format_size(entry.retained),
        );

        if let Some(sfi) = entry.sfi_id {
            print!("  sfi=@{sfi}");
        }
        if let Some(ctx) = entry.context_id {
            print!("  ctx=@{ctx}");
        }

        if !entry.context_vars.is_empty() {
            print!("  [{}]", entry.context_vars.join(", "));
        }

        println!();
    }

    println!("\nShowing {}-{} of {total} closures", start + 1, end);
}

struct ClosureEntry {
    name: String,
    id: u64,
    retained: f64,
    location: Option<String>,
    sfi_id: Option<u64>,
    context_id: Option<u64>,
    context_vars: Vec<String>,
    has_builtin_id: bool,
    has_function_template_info: bool,
    is_extension_script: bool,
    is_extension_native_context: bool,
    native_context_id: Option<NodeId>,
}
