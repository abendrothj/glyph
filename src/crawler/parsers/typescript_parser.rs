//! TypeScriptParser — tree-sitter recursive walk via GenericWalker.
//!
//! Handles function declarations, method definitions, and arrow functions /
//! function expressions (name resolved from the variable declarator parent).

use tree_sitter::{Language, Parser};

use super::super::{CallGraph, LanguageParser};
use super::builtins;
use super::walker::{walk_tree, WalkerConfig};
use std::collections::HashMap;

const TYPESCRIPT_CONFIG: WalkerConfig = WalkerConfig {
    // Named function nodes whose name is an inline field.
    // `function_declaration`  → function foo() {}
    // `method_definition`     → class methods
    // `generator_function_declaration` → function* foo() {}
    function_kinds: &["function_declaration", "method_definition", "generator_function_declaration"],
    function_name_field: "name",

    // Anonymous functions whose name comes from the parent variable declarator.
    // `arrow_function`     → const foo = () => {}
    // `function_expression`→ const foo = function() {}
    anon_function_kinds: &["arrow_function", "function_expression"],
    // Name is on the parent `variable_declarator` (top-level / local const/let/var)
    // or `public_field_definition` (class property arrow functions).
    anon_parent_kinds: &["variable_declarator", "public_field_definition"],
    anon_parent_name_field: "name",

    call_kind: "call_expression",
    call_function_field: "function",
    // obj.method() → member_expression; the method name is in the `property` field.
    method_receiver_kind: "member_expression",
    method_name_field: "property",

    path_call_kind: None,
    path_name_field: None,

    // if / else  — tree-sitter-typescript uses field-based alternative (same as Rust)
    // condition is a `parenthesized_expression`; walking it reaches the inner expression.
    if_kind: Some("if_statement"),
    if_condition_field: Some("condition"),
    if_then_field: Some("consequence"),
    if_else_field: Some("alternative"),
    elif_clause_kind: None, // TypeScript uses nested if_statement inside alternative
    elif_condition_field: None,
    elif_body_field: None,
    else_clause_kind: None,
    else_body_field: None,

    // for / while loops (includes for...of and for...in variants)
    for_kinds: &["for_statement", "for_in_statement", "for_of_statement"],
    while_kinds: &["while_statement", "do_statement"],
    loop_body_field: Some("body"),
    while_condition_field: Some("condition"),

    // No match expression in TypeScript (switch is a separate construct; not yet supported).
    match_kind: None,
    match_value_field: None,
    match_body_field: None,
    match_arm_kind: None,
    match_pattern_kind: None,

    // TypeScript test filtering is file-level (*.test.ts, *.spec.ts).
    test_mod_kind: None,
    test_mod_name_field: "",
    test_mod_names: &[],

    builtins: &builtins::TYPESCRIPT_BUILTINS,

    // `// @flow` above a function bypasses the builtins filter for that name.
    comment_kind: Some("comment"),
};

pub struct TypeScriptParser {
    language: Language,
}

impl TypeScriptParser {
    pub fn new() -> Self {
        Self { language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() }
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for TypeScriptParser {
    fn parse(&self, code: &str) -> CallGraph {
        self.parse_with_lines(code, false).0
    }

    fn parse_with_lines(&self, code: &str, no_flow: bool) -> (CallGraph, HashMap<String, u32>) {
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return (CallGraph::new(), HashMap::new());
        }
        let Some(tree) = parser.parse(code, None) else {
            return (CallGraph::new(), HashMap::new());
        };
        if tree.root_node().has_error() {
            return (CallGraph::new(), HashMap::new());
        }
        walk_tree(&TYPESCRIPT_CONFIG, tree.root_node(), code, no_flow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_empty() {
        assert!(TypeScriptParser::new().parse("").is_empty());
    }

    #[test]
    fn parse_single_function_no_calls() {
        let g = TypeScriptParser::new().parse("function foo() {}");
        assert!(g.contains_key("foo"));
        assert!(g["foo"].is_empty());
    }

    #[test]
    fn parse_function_calling_another() {
        let code = "function bar() {}\nfunction foo() { bar(); }\n";
        let g = TypeScriptParser::new().parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.contains_key("bar"));
        assert_eq!(g["foo"].len(), 1);
        assert_eq!(g["foo"][0].target, "bar");
    }

    #[test]
    fn parse_arrow_function_calling_another() {
        let code = "const bar = () => {};\nconst foo = () => { bar(); };\n";
        let g = TypeScriptParser::new().parse(code);
        assert!(g.contains_key("foo"), "arrow function 'foo' not found; keys: {:?}", g.keys().collect::<Vec<_>>());
        assert!(g.contains_key("bar"));
        assert!(g["foo"].iter().any(|e| e.target == "bar"));
    }

    #[test]
    fn parse_if_branch_creates_decision_node() {
        let code = r#"
function foo(x: number) {
    if (x > 0) {
        bar();
    } else {
        baz();
    }
}
function bar() {}
function baz() {}
"#;
        let g = TypeScriptParser::new().parse(code);
        let foo_edges = &g["foo"];
        assert!(foo_edges.iter().any(|e| e.target.starts_with("_decision_")));
        let dec_id = foo_edges
            .iter()
            .find(|e| e.target.starts_with("_decision_"))
            .unwrap()
            .target
            .clone();
        let dec_edges = &g[&dec_id];
        assert!(dec_edges.iter().any(|e| e.target == "bar" && e.label.as_deref() == Some("True")));
        assert!(dec_edges.iter().any(|e| e.target == "baz" && e.label.as_deref() == Some("False")));
    }

    #[test]
    fn parse_for_loop_creates_decision_node() {
        let code = "function foo() {\n    for (const x of items) {\n        bar();\n    }\n}\nfunction bar() {}\n";
        let g = TypeScriptParser::new().parse(code);
        let foo_edges = &g["foo"];
        assert!(foo_edges.iter().any(|e| e.target.starts_with("_decision_")));
        let dec_id = foo_edges
            .iter()
            .find(|e| e.target.starts_with("_decision_"))
            .unwrap()
            .target
            .clone();
        let dec_edges = &g[&dec_id];
        assert!(dec_edges.iter().any(|e| e.target == "bar" && e.label.as_deref() == Some("Loop")));
    }
}
