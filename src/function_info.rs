use rustc_hash::FxHashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, BlockStatement, CatchClause, ForInStatement, ForOfStatement,
    ForStatement, Function, Program,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{ScopeId, Scoping, SemanticBuilder};
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;

#[cfg(test)]
mod tests;

/// A span expressed in both UTF-8 byte offsets and UTF-16 code-unit offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualSpan {
    pub utf8_start: u32,
    pub utf8_end: u32,
    pub utf16_start: u32,
    pub utf16_end: u32,
}

/// What kind of scope created this entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Function,
    ArrowFunction,
    Block,
    For,
    ForIn,
    ForOf,
    Catch,
}

/// A captured variable with its name and the number of context-creating scopes
/// between the capturing scope and the declaring scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextVariable {
    pub name: String,
    /// Number of intervening context-creating scopes to traverse.
    /// 0 = declared in the immediate parent context, 1 = grandparent, etc.
    pub depth: u32,
}

/// Information about a scope that participates in context allocation.
#[derive(Clone, Debug)]
pub struct ScopeInfo {
    pub kind: ScopeKind,
    pub span: DualSpan,
    /// Variables captured from outer scopes.
    pub context_variables: Vec<ContextVariable>,
    /// Whether this scope creates a context (has variables captured by inner scopes).
    pub creates_context: bool,
    /// Variables declared in this scope that are stored in the context
    /// (i.e. captured by inner functions). Only populated when `creates_context` is true.
    pub context_slots: Vec<String>,
}

/// Parse JavaScript source and extract scope/context information.
///
/// Returns all functions and block scopes that either capture variables
/// from outer scopes or have their own variables captured by inner scopes.
pub fn extract_scopes(source: &str) -> Result<Vec<ScopeInfo>, String> {
    let utf16_table = build_utf16_offset_table(source);
    let allocator = Allocator::default();
    let source_type = SourceType::default();
    let parser_ret = Parser::new(&allocator, source, source_type).parse();

    if !parser_ret.errors.is_empty() {
        return Err(parser_ret
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    let semantic_ret = SemanticBuilder::new().build(&parser_ret.program);
    let scoping = semantic_ret.semantic.scoping();

    // Pass 1: determine which scopes create contexts and their captured bindings.
    let context_scope_slots = find_context_creating_scopes(scoping);

    // Pass 2: walk AST, collect scope info with depth-aware context variables.
    let mut collector = ScopeCollector {
        utf16_table: &utf16_table,
        scoping,
        context_scope_slots: &context_scope_slots,
        scopes: Vec::new(),
    };
    collector.visit_program(&parser_ret.program);

    Ok(collector.scopes)
}

/// Build a table mapping each UTF-8 byte offset to its UTF-16 code-unit offset.
fn build_utf16_offset_table(source: &str) -> Vec<u32> {
    let mut table = Vec::with_capacity(source.len() + 1);
    let mut utf16_offset: u32 = 0;
    for ch in source.chars() {
        let utf16_len = ch.len_utf16() as u32;
        for _ in 0..ch.len_utf8() {
            table.push(utf16_offset);
        }
        utf16_offset += utf16_len;
    }
    table.push(utf16_offset);
    table
}

fn make_dual_span(span: oxc_span::Span, utf16_table: &[u32]) -> DualSpan {
    DualSpan {
        utf8_start: span.start,
        utf8_end: span.end,
        utf16_start: utf16_table[span.start as usize],
        utf16_end: utf16_table[span.end as usize],
    }
}

/// Determine which scopes create V8 contexts and which of their bindings are
/// captured across a function boundary.
fn find_context_creating_scopes(scoping: &Scoping) -> FxHashMap<ScopeId, Vec<String>> {
    let mut result: FxHashMap<ScopeId, Vec<String>> = FxHashMap::default();

    for scope_id in scoping.scope_descendants_from_root() {
        for symbol_id in scoping.iter_bindings_in(scope_id) {
            let captured_across_function = scoping.get_resolved_references(symbol_id).any(
                |r: &oxc_syntax::reference::Reference| {
                    has_function_boundary_between(r.scope_id(), scope_id, scoping)
                },
            );
            if captured_across_function {
                result
                    .entry(scope_id)
                    .or_default()
                    .push(scoping.symbol_name(symbol_id).to_string());
            }
        }
    }

    for slots in result.values_mut() {
        slots.sort();
    }

    result
}

/// Returns true if there is a function/arrow scope boundary between `inner`
/// and `outer` (walking from inner to outer, exclusive of outer).
fn has_function_boundary_between(inner: ScopeId, outer: ScopeId, scoping: &Scoping) -> bool {
    let mut current = Some(inner);
    while let Some(s) = current {
        if s == outer {
            return false;
        }
        let flags = scoping.scope_flags(s);
        if flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow) {
            return true;
        }
        current = scoping.scope_parent_id(s);
    }
    false
}

/// For a given scope, find all variables it captures from outer scopes.
/// `depth` counts only intervening context-creating scopes.
fn collect_context_variables(
    scope_id: ScopeId,
    scoping: &Scoping,
    context_scope_slots: &FxHashMap<ScopeId, Vec<String>>,
) -> Vec<ContextVariable> {
    // Build a map from declaring scope → context depth.
    // Walk from scope_id upward, incrementing depth at each context-creating scope.
    let mut scope_depth: FxHashMap<ScopeId, u32> = FxHashMap::default();
    let mut depth: u32 = 0;
    for ancestor in scoping.scope_ancestors(scope_id) {
        if ancestor == scope_id {
            continue;
        }
        if context_scope_slots.contains_key(&ancestor) {
            scope_depth.insert(ancestor, depth);
            depth += 1;
        }
    }

    let mut captured = Vec::new();
    for (&declaring_scope, &d) in &scope_depth {
        for symbol_id in scoping.iter_bindings_in(declaring_scope) {
            let is_referenced = scoping.get_resolved_references(symbol_id).any(
                |r: &oxc_syntax::reference::Reference| {
                    is_scope_descendant(r.scope_id(), scope_id, scoping)
                },
            );
            if is_referenced {
                captured.push(ContextVariable {
                    name: scoping.symbol_name(symbol_id).to_string(),
                    depth: d,
                });
            }
        }
    }

    captured.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name)));
    captured
}

/// Returns true if `scope` is equal to or a descendant of `ancestor`.
fn is_scope_descendant(scope: ScopeId, ancestor: ScopeId, scoping: &Scoping) -> bool {
    let mut current = Some(scope);
    while let Some(s) = current {
        if s == ancestor {
            return true;
        }
        current = scoping.scope_parent_id(s);
    }
    false
}

struct ScopeCollector<'a> {
    utf16_table: &'a [u32],
    scoping: &'a Scoping,
    context_scope_slots: &'a FxHashMap<ScopeId, Vec<String>>,
    scopes: Vec<ScopeInfo>,
}

impl ScopeCollector<'_> {
    fn push_scope(&mut self, kind: ScopeKind, span: oxc_span::Span, scope_id: ScopeId) {
        let context_variables =
            collect_context_variables(scope_id, self.scoping, self.context_scope_slots);
        let context_slots = self
            .context_scope_slots
            .get(&scope_id)
            .cloned()
            .unwrap_or_default();
        let creates_context = !context_slots.is_empty();

        // Always emit function/arrow scopes so they can be matched by position.
        // Only emit block scopes when they are interesting (capture or are captured).
        let is_function = matches!(kind, ScopeKind::Function | ScopeKind::ArrowFunction);
        if is_function || !context_variables.is_empty() || creates_context {
            self.scopes.push(ScopeInfo {
                kind,
                span: make_dual_span(span, self.utf16_table),
                context_variables,
                creates_context,
                context_slots,
            });
        }
    }
}

impl<'a> Visit<'a> for ScopeCollector<'a> {
    fn visit_program(&mut self, program: &Program<'a>) {
        walk::walk_program(self, program);
    }

    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        self.push_scope(ScopeKind::Function, func.span, func.scope_id());
        walk::walk_function(self, func, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.push_scope(ScopeKind::ArrowFunction, arrow.span, arrow.scope_id());
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        let scope_id = block.scope_id();
        let flags = self.scoping.scope_flags(scope_id);
        // Skip blocks that are function/arrow bodies (handled by their own visitors).
        if !flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow) {
            // oxc places catch parameters in the catch body block scope.
            // Detect this by checking if the parent scope has the CatchClause flag.
            let kind = match self.scoping.scope_parent_id(scope_id) {
                Some(parent)
                    if self
                        .scoping
                        .scope_flags(parent)
                        .contains(ScopeFlags::CatchClause) =>
                {
                    ScopeKind::Catch
                }
                _ => ScopeKind::Block,
            };
            self.push_scope(kind, block.span, scope_id);
        }
        walk::walk_block_statement(self, block);
    }

    fn visit_for_statement(&mut self, stmt: &ForStatement<'a>) {
        self.push_scope(ScopeKind::For, stmt.span, stmt.scope_id());
        walk::walk_for_statement(self, stmt);
    }

    fn visit_for_in_statement(&mut self, stmt: &ForInStatement<'a>) {
        self.push_scope(ScopeKind::ForIn, stmt.span, stmt.scope_id());
        walk::walk_for_in_statement(self, stmt);
    }

    fn visit_for_of_statement(&mut self, stmt: &ForOfStatement<'a>) {
        self.push_scope(ScopeKind::ForOf, stmt.span, stmt.scope_id());
        walk::walk_for_of_statement(self, stmt);
    }

    fn visit_catch_clause(&mut self, clause: &CatchClause<'a>) {
        walk::walk_catch_clause(self, clause);
    }
}
