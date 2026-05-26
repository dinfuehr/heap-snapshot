use rustc_hash::FxHashMap;

use crate::print::format_size;
use crate::snapshot::{Detachedness, HeapSnapshot};
use crate::types::NodeOrdinal;

struct ContextEntry {
    ordinal: NodeOrdinal,
    retained: u64,
}

struct ScopeGroup {
    scope: Option<NodeOrdinal>,
    retained: u64,
    contexts: Vec<ContextEntry>,
}

struct ContextVarEntry {
    name: String,
    target: NodeOrdinal,
    retained: u64,
}

struct ContextVars {
    visible: Vec<ContextVarEntry>,
    omitted_below_threshold: usize,
}

#[derive(Clone)]
struct ScriptInfo {
    ordinal: NodeOrdinal,
    name: String,
}

#[derive(Clone, Default)]
struct ScopeMeta {
    name: Option<String>,
    script: Option<ScriptInfo>,
}

pub fn print_contexts(
    snap: &HeapSnapshot,
    minimum_retained_size: u64,
    minimum_var_retained_size: u64,
) {
    let mut groups_by_scope: FxHashMap<Option<NodeOrdinal>, Vec<ContextEntry>> =
        FxHashMap::default();
    let scope_meta = build_scope_meta(snap);

    for ord_idx in 0..snap.node_count() {
        let ordinal = NodeOrdinal(ord_idx);
        if !snap.is_context_object(ordinal) {
            continue;
        }
        let scope = snap.find_edge_target(ordinal, "scope_info");
        groups_by_scope
            .entry(scope)
            .or_default()
            .push(ContextEntry {
                ordinal,
                retained: snap.node_retained_size(ordinal),
            });
    }

    let mut groups: Vec<ScopeGroup> = groups_by_scope
        .into_iter()
        .map(|(scope, contexts)| {
            let retained = contexts.iter().map(|c| c.retained).sum();
            ScopeGroup {
                scope,
                retained,
                contexts,
            }
        })
        .collect();

    let total_context_count: usize = groups.iter().map(|g| g.contexts.len()).sum();
    if total_context_count == 0 {
        println!("No Context objects found.");
        return;
    }

    let excluded_scope_group_count = groups
        .iter()
        .filter(|group| group.retained < minimum_retained_size)
        .count();
    let excluded_context_count: usize = groups
        .iter()
        .filter(|group| group.retained < minimum_retained_size)
        .map(|group| group.contexts.len())
        .sum();

    groups.retain(|group| group.retained >= minimum_retained_size);

    groups.sort_by(|a, b| {
        b.retained
            .cmp(&a.retained)
            .then_with(|| scope_sort_key(snap, a.scope).cmp(&scope_sort_key(snap, b.scope)))
    });

    let context_count: usize = groups.iter().map(|g| g.contexts.len()).sum();
    println!(
        "{} Context object{} in {} scope group{} (NativeContexts excluded)",
        context_count,
        if context_count == 1 { "" } else { "s" },
        groups.len(),
        if groups.len() == 1 { "" } else { "s" },
    );
    println!(
        "  Scope group retained size >= {} (excluded: {} scope group{}, {} context{})",
        format_size(minimum_retained_size),
        excluded_scope_group_count,
        if excluded_scope_group_count == 1 {
            ""
        } else {
            "s"
        },
        excluded_context_count,
        if excluded_context_count == 1 { "" } else { "s" },
    );
    println!(
        "  Context var target retained size >= {}",
        format_size(minimum_var_retained_size)
    );

    if context_count == 0 {
        println!(
            "No Context scope groups with retained size >= {}.",
            format_size(minimum_retained_size)
        );
        return;
    }

    for group in &mut groups {
        group.contexts.sort_by(|a, b| {
            b.retained
                .cmp(&a.retained)
                .then_with(|| snap.node_id(a.ordinal).0.cmp(&snap.node_id(b.ordinal).0))
        });
    }

    for group in &groups {
        let scope_label = match group.scope {
            Some(scope) if is_scope_info(snap, scope) => format!("@{}", snap.node_id(scope).0),
            Some(scope) => format!(
                "{} @{}",
                snap.node_display_name(scope),
                snap.node_id(scope).0
            ),
            None => "(no scope_info)".to_string(),
        };
        let meta = group.scope.and_then(|scope| scope_meta.get(&scope));
        let scope_name = group
            .scope
            .map(|scope| scope_name(snap, scope, meta))
            .unwrap_or_else(|| "-".to_string());
        let script = meta
            .and_then(|meta| meta.script.as_ref())
            .map(|script| format!("{} @{}", script.name, snap.node_id(script.ordinal).0))
            .unwrap_or_else(|| "-".to_string());

        println!(
            "\nScope: {scope_label} (name: {scope_name}, script: {script}, contexts: {}, retained: {})",
            group.contexts.len(),
            format_size(group.retained)
        );
        println!(
            "  {:<10}  {:<32}  {:>3}  {:>12}  {:>14}",
            "Context", "Name", "Det", "Self Size", "Retained Size"
        );
        println!("  {}", "-".repeat(78));

        for entry in &group.contexts {
            let ordinal = entry.ordinal;
            println!(
                "  @{:<9}  {:<32}  {:>3}  {:>12}  {:>14}",
                snap.node_id(ordinal).0,
                snap.node_display_name(ordinal),
                det_label(snap.node_detachedness(ordinal)),
                format_size(snap.node_self_size(ordinal) as u64),
                format_size(entry.retained)
            );

            let vars = retained_context_vars(snap, ordinal, minimum_var_retained_size);
            for var in vars.visible {
                println!(
                    "    {} -> @{} {} (retained: {})",
                    var.name,
                    snap.node_id(var.target).0,
                    snap.node_display_name(var.target),
                    format_size(var.retained)
                );
            }
            if vars.omitted_below_threshold > 0 {
                println!(
                    "    {} context variable{} omitted below {}",
                    vars.omitted_below_threshold,
                    if vars.omitted_below_threshold == 1 {
                        ""
                    } else {
                        "s"
                    },
                    format_size(minimum_var_retained_size)
                );
            }
        }
    }
}

fn retained_context_vars(
    snap: &HeapSnapshot,
    context: NodeOrdinal,
    minimum_var_retained_size: u64,
) -> ContextVars {
    let mut vars = Vec::new();
    let mut omitted_below_threshold = 0;

    for (edge_idx, target) in snap.iter_edges(context) {
        if snap.edge_type_name(edge_idx) != "context" {
            continue;
        }

        let name = snap.edge_name(edge_idx);
        if name == "this" {
            continue;
        }

        let retained = snap.node_retained_size(target);
        if retained < minimum_var_retained_size {
            omitted_below_threshold += 1;
            continue;
        }

        vars.push(ContextVarEntry {
            name,
            target,
            retained,
        });
    }

    vars.sort_by(|a, b| {
        b.retained
            .cmp(&a.retained)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| snap.node_id(a.target).0.cmp(&snap.node_id(b.target).0))
    });

    ContextVars {
        visible: vars,
        omitted_below_threshold,
    }
}

fn build_scope_meta(snap: &HeapSnapshot) -> FxHashMap<NodeOrdinal, ScopeMeta> {
    let mut meta = FxHashMap::default();
    let mut outer_scopes = Vec::new();

    for ord_idx in 0..snap.node_count() {
        let scope = NodeOrdinal(ord_idx);
        if !is_scope_info(snap, scope) {
            continue;
        }
        if let Some(outer) = snap
            .find_edge_target(scope, "outer_scope_info")
            .filter(|&target| is_scope_info(snap, target))
        {
            outer_scopes.push((scope, outer));
        }
    }

    for ord_idx in 0..snap.node_count() {
        let sfi = NodeOrdinal(ord_idx);
        if !snap.is_shared_function_info(sfi) {
            continue;
        }

        let script = sfi_script(snap, sfi);

        if let Some(scope) = snap
            .find_edge_target(sfi, "name_or_scope_info")
            .filter(|&target| is_scope_info(snap, target))
        {
            let name = sfi_name(snap, sfi);
            let entry: &mut ScopeMeta = meta.entry(scope).or_default();
            if entry.name.is_none() {
                entry.name = name;
            }
            if entry.script.is_none() {
                entry.script = script.clone();
            }
        }

        if let Some(scope) = snap
            .find_edge_target(sfi, "raw_outer_scope_info_or_feedback_metadata")
            .filter(|&target| is_scope_info(snap, target))
        {
            let entry: &mut ScopeMeta = meta.entry(scope).or_default();
            if entry.script.is_none() {
                entry.script = script;
            }
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for &(scope, outer) in &outer_scopes {
            let Some(script) = meta.get(&outer).and_then(|m| m.script.clone()) else {
                continue;
            };
            let entry: &mut ScopeMeta = meta.entry(scope).or_default();
            if entry.script.is_none() {
                entry.script = Some(script);
                changed = true;
            }
        }
    }

    meta
}

fn sfi_script(snap: &HeapSnapshot, sfi: NodeOrdinal) -> Option<ScriptInfo> {
    let script = snap.find_edge_target(sfi, "script")?;
    Some(ScriptInfo {
        ordinal: script,
        name: script_name(snap, script),
    })
}

fn script_name(snap: &HeapSnapshot, script: NodeOrdinal) -> String {
    let raw = snap.node_raw_name(script);
    raw.strip_prefix("system / Script / ")
        .unwrap_or(raw)
        .to_string()
}

fn sfi_name(snap: &HeapSnapshot, sfi: NodeOrdinal) -> Option<String> {
    snap.node_raw_name(sfi)
        .strip_prefix("system / SharedFunctionInfo / ")
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
}

fn scope_name(snap: &HeapSnapshot, scope: NodeOrdinal, meta: Option<&ScopeMeta>) -> String {
    if let Some(name) = meta.and_then(|m| m.name.as_ref()) {
        return name.clone();
    }

    if scope_type_name(snap, scope).as_deref() == Some("SCRIPT_SCOPE")
        && let Some(script) = meta.and_then(|m| m.script.as_ref())
    {
        return script.name.clone();
    }

    scope_type_name(snap, scope).unwrap_or_else(|| "-".to_string())
}

fn scope_type_name(snap: &HeapSnapshot, scope: NodeOrdinal) -> Option<String> {
    snap.find_edge_target(scope, "scope_type_name")
        .map(|name| snap.node_raw_name(name).to_string())
}

fn is_scope_info(snap: &HeapSnapshot, ordinal: NodeOrdinal) -> bool {
    snap.node_raw_name(ordinal) == "system / ScopeInfo"
}

fn scope_sort_key(snap: &HeapSnapshot, scope: Option<NodeOrdinal>) -> u64 {
    scope.map(|ord| snap.node_id(ord).0).unwrap_or(u64::MAX)
}

fn det_label(detachedness: Detachedness) -> &'static str {
    match detachedness {
        Detachedness::Attached => "no",
        Detachedness::Detached => "yes",
        Detachedness::Unknown => "",
    }
}
