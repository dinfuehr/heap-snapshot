#[cfg(test)]
macro_rules! parsed_heap_snapshot {
    (
        options: $options:expr,
        snapshot: $snapshot:expr,
        $nodes:ident,
        $edges:ident,
        $strings:ident,
        locations: $locations:expr,
        trace_function_infos: $trace_function_infos:expr,
        trace_tree_parents: $trace_tree_parents:expr,
        trace_tree_func_idxs: $trace_tree_func_idxs:expr,
        samples: $samples:expr $(,)?
    ) => {
        $crate::snapshot::HeapSnapshot::from_raw_parts_with_options_for_test(
            $snapshot,
            $nodes,
            $edges,
            $strings,
            $locations,
            $trace_function_infos,
            $trace_tree_parents,
            $trace_tree_func_idxs,
            $samples,
            $options,
        )
    };
    (
        options: $options:expr,
        snapshot: $snapshot:expr,
        nodes: $nodes:expr,
        edges: $edges:expr,
        strings: $strings:expr,
        locations: $locations:expr,
        trace_function_infos: $trace_function_infos:expr,
        trace_tree_parents: $trace_tree_parents:expr,
        trace_tree_func_idxs: $trace_tree_func_idxs:expr,
        samples: $samples:expr $(,)?
    ) => {
        $crate::snapshot::HeapSnapshot::from_raw_parts_with_options_for_test(
            $snapshot,
            $nodes,
            $edges,
            $strings,
            $locations,
            $trace_function_infos,
            $trace_tree_parents,
            $trace_tree_func_idxs,
            $samples,
            $options,
        )
    };
    (
        snapshot: $snapshot:expr,
        $nodes:ident,
        $edges:ident,
        $strings:ident,
        locations: $locations:expr,
        trace_function_infos: $trace_function_infos:expr,
        trace_tree_parents: $trace_tree_parents:expr,
        trace_tree_func_idxs: $trace_tree_func_idxs:expr,
        samples: $samples:expr $(,)?
    ) => {
        $crate::snapshot::HeapSnapshot::from_raw_parts_with_options_for_test(
            $snapshot,
            $nodes,
            $edges,
            $strings,
            $locations,
            $trace_function_infos,
            $trace_tree_parents,
            $trace_tree_func_idxs,
            $samples,
            Default::default(),
        )
    };
    (
        snapshot: $snapshot:expr,
        nodes: $nodes:expr,
        edges: $edges:expr,
        strings: $strings:expr,
        locations: $locations:expr,
        trace_function_infos: $trace_function_infos:expr,
        trace_tree_parents: $trace_tree_parents:expr,
        trace_tree_func_idxs: $trace_tree_func_idxs:expr,
        samples: $samples:expr $(,)?
    ) => {
        $crate::snapshot::HeapSnapshot::from_raw_parts_with_options_for_test(
            $snapshot,
            $nodes,
            $edges,
            $strings,
            $locations,
            $trace_function_infos,
            $trace_tree_parents,
            $trace_tree_func_idxs,
            $samples,
            Default::default(),
        )
    };
}

pub mod diff;
#[cfg(feature = "cli")]
pub mod display;
pub mod function_info;
#[cfg(feature = "cli")]
pub mod mcp;
#[cfg(feature = "cli")]
pub mod print;
pub mod retaining_path;
pub mod snapshot;
#[cfg(feature = "cli")]
pub mod tui;
pub mod types;
pub mod utils;

/// Fetch the name of a Chrome extension from the Chrome Web Store.
#[cfg(feature = "cli")]
pub fn resolve_chrome_extension_name(extension_id: &str) -> Option<String> {
    use std::time::Duration;
    let url = format!("https://chromewebstore.google.com/detail/{extension_id}");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .new_agent();
    let body: String = agent
        .get(&url)
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let title_start = body.find("<title>")? + "<title>".len();
    let title_end = body[title_start..].find("</title>")? + title_start;
    let title = &body[title_start..title_end];
    let name = title.strip_suffix(" - Chrome Web Store")?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
