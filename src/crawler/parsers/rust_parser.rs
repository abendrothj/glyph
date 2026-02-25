//! RustParser — tree-sitter recursive walk via GenericWalker.

use tree_sitter::{Language, Parser};

use super::super::{CallGraph, LanguageParser};
use super::builtins;
use super::walker::{walk_tree, WalkerConfig};
use std::collections::HashMap;

const RUST_CONFIG: WalkerConfig = WalkerConfig {
    // fn foo() / pub fn foo() / async fn foo()
    function_kinds: &["function_item"],
    function_name_field: "name",

    // Closures: skipped for now (no stable name from parent without type info).
    anon_function_kinds: &[],
    anon_parent_kinds: &[],
    anon_parent_name_field: "name",

    call_kind: "call_expression",
    call_function_field: "function",
    // self.foo() / items.bar() → field_expression; method name in `field`.
    method_receiver_kind: "field_expression",
    method_name_field: "field",
    // Struct::new() / Module::helper() → scoped_identifier; name in `name`.
    path_call_kind: Some("scoped_identifier"),
    path_name_field: Some("name"),

    if_kind: Some("if_expression"),
    if_condition_field: Some("condition"),
    if_then_field: Some("consequence"),
    if_else_field: Some("alternative"),
    elif_clause_kind: None,
    elif_condition_field: None,
    elif_body_field: None,
    else_clause_kind: None,
    else_body_field: None,

    for_kinds: &["for_expression"],
    while_kinds: &["while_expression"],
    loop_body_field: Some("body"),
    while_condition_field: Some("condition"),

    match_kind: Some("match_expression"),
    match_value_field: Some("value"),
    // match_arm nodes live inside match_block (the `body` field), not as
    // direct children of match_expression.
    match_body_field: Some("body"),
    match_arm_kind: Some("match_arm"),
    match_pattern_kind: Some("match_pattern"),

    // `#[cfg(test)] mod tests { ... }` — skip the whole subtree.
    test_mod_kind: Some("mod_item"),
    test_mod_name_field: "name",
    test_mod_names: &["tests", "test"],

    builtins: &builtins::RUST_BUILTINS,

    // `// @flow` above a fn bypasses the builtins filter for that name.
    comment_kind: Some("line_comment"),
};

pub struct RustParser {
    language: Language,
}

impl RustParser {
    pub fn new() -> Self {
        Self { language: tree_sitter_rust::LANGUAGE.into() }
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for RustParser {
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
        walk_tree(&RUST_CONFIG, tree.root_node(), code, no_flow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_empty() {
        assert!(RustParser::new().parse("").is_empty());
    }

    #[test]
    fn parse_invalid_syntax_returns_empty() {
        assert!(RustParser::new().parse("fn broken {").is_empty());
    }

    #[test]
    fn parse_garbage_input_returns_empty() {
        assert!(RustParser::new().parse("\x00\x01\x02 garbage binary data").is_empty());
    }

    // Logic flow map integration tests: see tests/logic_flow_map.rs
}
