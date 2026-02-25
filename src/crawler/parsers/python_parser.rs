//! PythonParser — tree-sitter recursive walk via GenericWalker.

use tree_sitter::{Language, Parser};

use super::super::{CallGraph, LanguageParser};
use super::builtins;
use super::walker::{walk_tree, WalkerConfig};
use std::collections::HashMap;

const PYTHON_CONFIG: WalkerConfig = WalkerConfig {
    // def foo() / async def foo()  — both produce `function_definition`
    function_kinds: &["function_definition"],
    function_name_field: "name",

    // Python has no anonymous function syntax with a separate node kind.
    // (Lambdas are `lambda` nodes but have no name; skip for now.)
    anon_function_kinds: &[],
    anon_parent_kinds: &[],
    anon_parent_name_field: "name",

    call_kind: "call",
    call_function_field: "function",
    // obj.method() → attribute node; the method name is in the `attribute` field.
    method_receiver_kind: "attribute",
    method_name_field: "attribute",

    path_call_kind: None,
    path_name_field: None,

    // if / elif / else
    // tree-sitter-python: if_statement has `condition` and `consequence` fields;
    // elif_clause and else_clause are sibling child nodes, not named fields.
    if_kind: Some("if_statement"),
    if_condition_field: Some("condition"),
    if_then_field: Some("consequence"),
    if_else_field: None, // no alternative field; use child-node-based handling below
    elif_clause_kind: Some("elif_clause"),
    elif_condition_field: Some("condition"),
    elif_body_field: Some("consequence"),
    else_clause_kind: Some("else_clause"),
    else_body_field: Some("body"),

    // for x in ...: / while ...:
    for_kinds: &["for_statement"],
    while_kinds: &["while_statement"],
    loop_body_field: Some("body"),
    while_condition_field: Some("condition"),

    // Python 3.10+ match_statement is structurally different from Rust match;
    // not yet supported — treated as a generic node (children walked normally).
    match_kind: None,
    match_value_field: None,
    match_body_field: None,
    match_arm_kind: None,
    match_pattern_kind: None,

    // Python has no inline test-module syntax; test filtering is file-level.
    test_mod_kind: None,
    test_mod_name_field: "",
    test_mod_names: &[],

    builtins: &builtins::PYTHON_BUILTINS,

    // `# @flow` above a def bypasses the builtins filter for that name.
    comment_kind: Some("comment"),
};

pub struct PythonParser {
    language: Language,
}

impl PythonParser {
    pub fn new() -> Self {
        Self { language: tree_sitter_python::LANGUAGE.into() }
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for PythonParser {
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
        walk_tree(&PYTHON_CONFIG, tree.root_node(), code, no_flow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_empty() {
        assert!(PythonParser::new().parse("").is_empty());
    }

    #[test]
    fn parse_single_function_no_calls() {
        let g = PythonParser::new().parse("def foo():\n    pass\n");
        assert!(g.contains_key("foo"));
        assert!(g["foo"].is_empty());
    }

    #[test]
    fn parse_function_calling_another() {
        let code = "def bar():\n    pass\n\ndef foo():\n    bar()\n";
        let g = PythonParser::new().parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.contains_key("bar"));
        assert_eq!(g["foo"].len(), 1);
        assert_eq!(g["foo"][0].target, "bar");
    }

    #[test]
    fn parse_if_branch_creates_decision_node() {
        let code = r#"
def foo(x):
    if x > 0:
        bar()
    else:
        baz()

def bar():
    pass

def baz():
    pass
"#;
        let g = PythonParser::new().parse(code);
        let foo_edges = &g["foo"];
        assert!(foo_edges.iter().any(|e| e.target.starts_with("_decision_")));
        let dec_id = foo_edges.iter().find(|e| e.target.starts_with("_decision_")).unwrap().target.clone();
        let dec_edges = &g[&dec_id];
        assert!(dec_edges.iter().any(|e| e.target == "bar" && e.label.as_deref() == Some("True")));
        assert!(dec_edges.iter().any(|e| e.target == "baz" && e.label.as_deref() == Some("False")));
    }

    #[test]
    fn parse_for_loop_creates_decision_node() {
        let code = "def foo():\n    for i in range(10):\n        bar()\n\ndef bar():\n    pass\n";
        let g = PythonParser::new().parse(code);
        let foo_edges = &g["foo"];
        assert!(foo_edges.iter().any(|e| e.target.starts_with("_decision_")));
        let dec_id = foo_edges.iter().find(|e| e.target.starts_with("_decision_")).unwrap().target.clone();
        let dec_edges = &g[&dec_id];
        assert!(dec_edges.iter().any(|e| e.target == "bar" && e.label.as_deref() == Some("Loop")));
    }

    #[test]
    fn parse_malformed_no_panic() {
        // Incomplete function def
        let result = PythonParser::new().parse("def broken(");
        // Should not panic — may return empty or partial
        let _ = result;
    }

    #[test]
    fn parse_garbage_no_panic() {
        let result = PythonParser::new().parse("\x00\x01 garbage");
        let _ = result;
    }
}
