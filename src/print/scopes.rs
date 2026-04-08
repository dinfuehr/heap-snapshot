use crate::function_info::{ScopeInfo, ScopeKind, extract_scopes};
use crate::snapshot::HeapSnapshot;
use crate::types::NodeId;

pub fn print_scopes(snap: &HeapSnapshot, node_id: NodeId) {
    let ordinal = match snap.node_for_snapshot_object_id(node_id) {
        Some(o) => o,
        None => {
            println!("Error: no node found with id @{node_id}");
            std::process::exit(1);
        }
    };

    let source = match snap.script_source(ordinal) {
        Some(s) => s,
        None => {
            println!("Error: @{node_id} is not a Script or SharedFunctionInfo, or has no source");
            std::process::exit(1);
        }
    };

    let scopes = match extract_scopes(source) {
        Ok(s) => s,
        Err(e) => {
            println!("Error parsing source: {e}");
            std::process::exit(1);
        }
    };

    if scopes.is_empty() {
        println!("No context-creating scopes found.");
        return;
    }

    // Build a tree by nesting scopes based on their spans.
    let tree = build_scope_tree(&scopes);
    for node in &tree {
        print_scope_node(source, node, 0);
    }
}

struct ScopeNode<'a> {
    scope: &'a ScopeInfo,
    children: Vec<ScopeNode<'a>>,
}

fn build_scope_tree<'a>(scopes: &'a [ScopeInfo]) -> Vec<ScopeNode<'a>> {
    let mut roots: Vec<ScopeNode<'a>> = Vec::new();
    for scope in scopes {
        insert_into(&mut roots, scope);
    }
    roots
}

fn insert_into<'a>(nodes: &mut Vec<ScopeNode<'a>>, scope: &'a ScopeInfo) {
    for node in nodes.iter_mut() {
        if contains(node.scope, scope) {
            insert_into(&mut node.children, scope);
            return;
        }
    }
    nodes.push(ScopeNode {
        scope,
        children: Vec::new(),
    });
}

fn contains(outer: &ScopeInfo, inner: &ScopeInfo) -> bool {
    outer.span.utf8_start <= inner.span.utf8_start && inner.span.utf8_end <= outer.span.utf8_end
}

fn print_scope_node(source: &str, node: &ScopeNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let scope = node.scope;
    let kind = match scope.kind {
        ScopeKind::Function => "Function",
        ScopeKind::ArrowFunction => "ArrowFunction",
        ScopeKind::Block => "Block",
        ScopeKind::For => "For",
        ScopeKind::ForIn => "ForIn",
        ScopeKind::ForOf => "ForOf",
        ScopeKind::Catch => "Catch",
    };

    let (line, col) = offset_to_line_col(source, scope.span.utf8_start as usize);
    let (end_line, end_col) = offset_to_line_col(source, scope.span.utf8_end as usize);

    let ctx_info = if scope.creates_context {
        format!(" (context: {})", scope.context_slots.join(", "))
    } else {
        String::new()
    };

    println!(
        "{indent}{kind} [{line}:{col}-{end_line}:{end_col}]{ctx_info} creates_context: {}",
        scope.creates_context,
    );

    if !scope.context_variables.is_empty() {
        let vars: Vec<String> = scope
            .context_variables
            .iter()
            .map(|v| {
                if v.depth == 0 {
                    v.name.clone()
                } else {
                    format!("{} (depth {})", v.name, v.depth)
                }
            })
            .collect();
        println!("{indent}  captures: {}", vars.join(", "));
    }

    for child in &node.children {
        print_scope_node(source, child, depth + 1);
    }
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
