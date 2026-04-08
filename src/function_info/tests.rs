use super::*;

/// Helper: extract just the variable names from a scope's context_variables.
fn var_names(scope: &ScopeInfo) -> Vec<&str> {
    scope
        .context_variables
        .iter()
        .map(|v| v.name.as_str())
        .collect()
}

/// Helper: extract (name, depth) pairs from a scope's context_variables.
fn var_depths(scope: &ScopeInfo) -> Vec<(&str, u32)> {
    scope
        .context_variables
        .iter()
        .map(|v| (v.name.as_str(), v.depth))
        .collect()
}

/// Helper: extract context slot names.
fn slot_names(scope: &ScopeInfo) -> Vec<&str> {
    scope.context_slots.iter().map(|s| s.as_str()).collect()
}

// --- basic function tests ---

#[test]
fn test_simple_function_declaration() {
    let source = "function foo() { return 1; }";
    let result = extract_scopes(source).unwrap();
    // No captures, no inner functions → not interesting.
    assert!(result.is_empty());
}

#[test]
fn test_function_with_capture() {
    let source = "const x = 1; function foo() { return x; }";
    let result = extract_scopes(source).unwrap();
    let fns: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(var_names(fns[0]), vec!["x"]);
}

#[test]
fn test_arrow_function() {
    let source = "const f = () => 42;";
    let result = extract_scopes(source).unwrap();
    // No captures → not interesting.
    assert!(result.is_empty());
}

#[test]
fn test_arrow_captures() {
    let source = "const x = 10; const f = (y) => x + y;";
    let result = extract_scopes(source).unwrap();
    let arrows: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 1);
    assert_eq!(var_names(arrows[0]), vec!["x"]);
}

#[test]
fn test_function_expression() {
    let source = "const x = 1; const f = function named() { return x; };";
    let result = extract_scopes(source).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].kind, ScopeKind::Function);
    assert_eq!(var_names(&result[0]), vec!["x"]);
}

#[test]
fn test_no_functions() {
    let source = "const x = 1 + 2;";
    let result = extract_scopes(source).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_no_capture_of_local_vars() {
    let source = "function foo() { const x = 1; return x; }";
    let result = extract_scopes(source).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_parameter_not_captured() {
    let source = "const x = 1; function foo(x) { return x; }";
    let result = extract_scopes(source).unwrap();
    // x is shadowed by the parameter, so no capture.
    assert!(result.is_empty());
}

// --- nested functions and depth ---

#[test]
fn test_nested_functions_depth() {
    let source = "function outer() { const a = 1; function inner() { return a; } }";
    let result = extract_scopes(source).unwrap();

    // outer creates a context (a is captured), inner captures a.
    let outer: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::Function && s.creates_context)
        .collect();
    assert_eq!(outer.len(), 1);
    assert_eq!(slot_names(outer[0]), vec!["a"]);

    let inner: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::Function && !s.context_variables.is_empty())
        .collect();
    assert_eq!(inner.len(), 1);
    assert_eq!(var_depths(inner[0]), vec![("a", 0)]);
}

#[test]
fn test_three_levels_depth() {
    let source = r#"
        function a() {
            const x = 1;
            function b() {
                const y = 2;
                function c() {
                    return x + y;
                }
            }
        }
    "#;
    let result = extract_scopes(source).unwrap();

    // a creates context with x, b creates context with y.
    let a_scope = result
        .iter()
        .find(|s| s.creates_context && s.context_slots.iter().any(|n| n == "x"))
        .expect("a should create context for x");
    assert_eq!(slot_names(a_scope), vec!["x"]);

    let b_scope = result
        .iter()
        .find(|s| s.creates_context && s.context_slots.iter().any(|n| n == "y"))
        .expect("b should create context for y");
    assert_eq!(slot_names(b_scope), vec!["y"]);

    // c captures x (depth 1, through b's context) and y (depth 0, from b).
    let c = result
        .iter()
        .find(|s| {
            s.kind == ScopeKind::Function
                && s.context_variables.iter().any(|v| v.name == "x")
                && s.context_variables.iter().any(|v| v.name == "y")
        })
        .expect("should find function c");

    let x_depth = c
        .context_variables
        .iter()
        .find(|v| v.name == "x")
        .unwrap()
        .depth;
    let y_depth = c
        .context_variables
        .iter()
        .find(|v| v.name == "y")
        .unwrap()
        .depth;

    // y is in b (immediate parent context), x is in a (one more up).
    assert_eq!(y_depth, 0);
    assert_eq!(x_depth, 1);
}

#[test]
fn test_arrow_nested_in_function() {
    let source = "function outer() { const a = 1; const f = () => a; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("should find arrow");
    assert_eq!(var_depths(arrow), vec![("a", 0)]);
}

#[test]
fn test_arrow_nested_in_arrow() {
    let source = "const a = 1; const f = () => { const b = 2; return () => a + b; };";
    let result = extract_scopes(source).unwrap();
    let arrows: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 2);

    // Inner arrow captures b (depth 0) and a (depth 1).
    let inner = arrows
        .iter()
        .find(|s| s.context_variables.len() == 2)
        .expect("inner arrow");
    let a_depth = inner
        .context_variables
        .iter()
        .find(|v| v.name == "a")
        .unwrap()
        .depth;
    let b_depth = inner
        .context_variables
        .iter()
        .find(|v| v.name == "b")
        .unwrap()
        .depth;
    assert_eq!(b_depth, 0);
    assert_eq!(a_depth, 1);
}

// --- arrow function variants ---

#[test]
fn test_arrow_expression_body() {
    let source = "const x = 1; const f = y => x * y;";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["x"]);
}

#[test]
fn test_arrow_block_body() {
    let source = "const x = 1; const f = (y) => { return x * y; };";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["x"]);
}

#[test]
fn test_arrow_parameter_shadowing() {
    let source = "const x = 1; const f = (x) => x + 1;";
    let result = extract_scopes(source).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_arrow_multiple_params() {
    let source = "const z = 1; const f = (x, y) => x + y + z;";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["z"]);
}

#[test]
fn test_arrow_in_array() {
    let source = "const x = 1; const arr = [() => x, () => x + 1];";
    let result = extract_scopes(source).unwrap();
    let arrows: Vec<_> = result
        .iter()
        .filter(|s| s.kind == ScopeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 2);
    assert_eq!(var_names(arrows[0]), vec!["x"]);
    assert_eq!(var_names(arrows[1]), vec!["x"]);
}

#[test]
fn test_async_arrow() {
    let source = "const x = 1; const f = async () => x;";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["x"]);
}

// --- for-of / for-in / for / block / catch ---

#[test]
fn test_for_of_capture() {
    let source = "for (const item of [1,2,3]) { const f = () => item; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert!(var_names(arrow).contains(&"item"));
}

#[test]
fn test_for_of_creates_context() {
    let source = "for (const item of [1,2,3]) { const f = () => item; }";
    let result = extract_scopes(source).unwrap();
    let for_of = result
        .iter()
        .find(|s| s.kind == ScopeKind::ForOf)
        .expect("for-of scope");
    assert!(for_of.creates_context);
    assert_eq!(slot_names(for_of), vec!["item"]);
}

#[test]
fn test_for_in_capture() {
    let source = "for (const key in {a:1}) { const f = () => key; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert!(var_names(arrow).contains(&"key"));
}

#[test]
fn test_for_let_capture() {
    let source = "for (let i = 0; i < 3; i++) { const f = () => i; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert!(var_names(arrow).contains(&"i"));
}

#[test]
fn test_for_var_capture() {
    let source = "for (var i = 0; i < 3; i++) { const f = () => i; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert!(var_names(arrow).contains(&"i"));
}

#[test]
fn test_block_scope_capture() {
    let source = "let f; { let x = 1; f = () => x; }";
    let result = extract_scopes(source).unwrap();
    let block = result
        .iter()
        .find(|s| s.kind == ScopeKind::Block)
        .expect("block scope");
    assert!(block.creates_context);
    assert_eq!(slot_names(block), vec!["x"]);

    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["x"]);
}

#[test]
fn test_catch_capture() {
    let source = "let f; try { throw 1; } catch (e) { f = () => e; }";
    let result = extract_scopes(source).unwrap();

    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert_eq!(var_names(arrow), vec!["e"]);

    // The catch clause scope should create a context since `e` is captured.
    let catch = result
        .iter()
        .find(|s| s.kind == ScopeKind::Catch)
        .expect("catch scope");
    assert!(catch.creates_context);
    assert_eq!(slot_names(catch), vec!["e"]);
}

#[test]
fn test_for_of_no_capture() {
    let source = "for (const item of [1,2,3]) { console.log(item); }";
    let result = extract_scopes(source).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_for_of_nested_captures_outer_and_loop() {
    let source = "const x = 1; for (const item of [1,2,3]) { const f = () => x + item; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");
    assert!(var_names(arrow).contains(&"x"));
    assert!(var_names(arrow).contains(&"item"));
}

// --- depth through block scopes ---

#[test]
fn test_depth_through_for_of() {
    // x is in script scope, item is in for-of scope.
    // The arrow is inside the for-of body.
    // If for-of creates a context (it does, because arrow captures item),
    // then x is at depth 1 (through the for-of context) and item at depth 0.
    let source = "const x = 1; for (const item of [1,2,3]) { const f = () => x + item; }";
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");

    let item_depth = arrow
        .context_variables
        .iter()
        .find(|v| v.name == "item")
        .unwrap()
        .depth;
    let x_depth = arrow
        .context_variables
        .iter()
        .find(|v| v.name == "x")
        .unwrap()
        .depth;

    assert_eq!(item_depth, 0);
    assert!(x_depth > item_depth);
}

#[test]
fn test_depth_nested_blocks() {
    let source = r#"
        const a = 1;
        {
            let b = 2;
            {
                let c = 3;
                const f = () => a + b + c;
            }
        }
    "#;
    let result = extract_scopes(source).unwrap();
    let arrow = result
        .iter()
        .find(|s| s.kind == ScopeKind::ArrowFunction)
        .expect("arrow");

    let c_depth = arrow
        .context_variables
        .iter()
        .find(|v| v.name == "c")
        .unwrap()
        .depth;
    let b_depth = arrow
        .context_variables
        .iter()
        .find(|v| v.name == "b")
        .unwrap()
        .depth;
    let a_depth = arrow
        .context_variables
        .iter()
        .find(|v| v.name == "a")
        .unwrap()
        .depth;

    // c is in immediate parent block, b in grandparent, a in script scope.
    assert_eq!(c_depth, 0);
    assert_eq!(b_depth, 1);
    assert!(a_depth > b_depth);
}

// --- creates_context flag ---

#[test]
fn test_creates_context_for_captured_function() {
    let source = "function outer() { const x = 1; function inner() { return x; } }";
    let result = extract_scopes(source).unwrap();
    let outer = result
        .iter()
        .find(|s| s.kind == ScopeKind::Function && s.creates_context)
        .expect("outer should create context");
    assert!(outer.creates_context);
    assert_eq!(slot_names(outer), vec!["x"]);
}

#[test]
fn test_no_context_when_not_captured() {
    let source = "function foo() { const x = 1; return x; }";
    let result = extract_scopes(source).unwrap();
    // foo doesn't have captured variables, so it's not emitted at all.
    assert!(result.is_empty());
}

// --- UTF-16 offset tests ---

#[test]
fn test_utf16_offsets_ascii() {
    let source = "const x = 1; function f() { return x; }";
    let result = extract_scopes(source).unwrap();
    let f = result
        .iter()
        .find(|s| s.kind == ScopeKind::Function)
        .expect("function");
    assert_eq!(f.span.utf8_start, f.span.utf16_start);
    assert_eq!(f.span.utf8_end, f.span.utf16_end);
}

#[test]
fn test_utf16_offsets_with_emoji() {
    let source = "const x = '😀'; function f() { return x; }";
    let result = extract_scopes(source).unwrap();
    let f = result
        .iter()
        .find(|s| s.kind == ScopeKind::Function)
        .expect("function");
    // '😀' is 4 bytes UTF-8 but 2 code units UTF-16: difference of 2.
    assert_eq!(f.span.utf8_start - f.span.utf16_start, 2);
    assert_eq!(f.span.utf8_end - f.span.utf16_end, 2);
}

// --- build_utf16_offset_table ---

#[test]
fn test_build_utf16_offset_table_ascii() {
    let table = build_utf16_offset_table("abc");
    assert_eq!(table, vec![0, 1, 2, 3]);
}

#[test]
fn test_build_utf16_offset_table_emoji() {
    let table = build_utf16_offset_table("a😀b");
    assert_eq!(table, vec![0, 1, 1, 1, 1, 3, 4]);
}
