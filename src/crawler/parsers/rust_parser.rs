//! RustParser — tree-sitter + Query for function_item and call_expression.

use tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use tree_sitter::StreamingIterator;

use super::super::{CallGraph, LanguageParser};

/// S-expression query: (1) function definitions, (2) direct calls, (3) method calls.
/// Calls are associated with the function definition they reside within (containment).
const RUST_QUERY: &str = r#"
(function_item
  name: (identifier) @fn_name)
(call_expression
  function: (identifier) @call_name)
(call_expression
  function: (field_expression
    field: (field_identifier) @call_name))
"#;

pub struct RustParser {
    language: Language,
    query: Query,
}

impl RustParser {
    pub fn new() -> Self {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let query = Query::new(&language, RUST_QUERY).expect("Rust query must be valid");
        Self { language, query }
    }

    fn get_text(node: Node, source: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        source.get(start..end).unwrap_or("").trim().to_string()
    }

    fn callee_text(node: Node, source: &str) -> String {
        Self::get_text(node, source)
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for RustParser {
    fn parse(&self, code: &str) -> CallGraph {
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return CallGraph::new();
        }
        let Some(tree) = parser.parse(code, None) else {
            return CallGraph::new();
        };
        if tree.root_node().has_error() {
            return CallGraph::new();
        }

        let mut graph = CallGraph::new();
        let root = tree.root_node();

        // Collect function ranges (start_byte, end_byte) and names.
        let mut functions: Vec<(usize, usize, String)> = Vec::new();
        let mut calls: Vec<(usize, usize, String)> = Vec::new();

        let name_idx = self.query.capture_index_for_name("fn_name").unwrap_or(0);
        let call_name_idx = self.query.capture_index_for_name("call_name").unwrap_or(1);

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, root, code.as_bytes());

        while let Some(qm) = matches.next() {
            for cap in qm.captures {
                let node = cap.node;
                let text = Self::get_text(node, code);
                if text.is_empty() {
                    continue;
                }
                if cap.index == name_idx {
                    // Use parent function_item range for containment
                    let (start, end) = node
                        .parent()
                        .map(|p: Node| (p.start_byte(), p.end_byte()))
                        .unwrap_or((node.start_byte(), node.end_byte()));
                    functions.push((start, end, text));
                } else if cap.index == call_name_idx {
                    let callee = Self::callee_text(node, code);
                    if !callee.is_empty() {
                        calls.push((node.start_byte(), node.end_byte(), callee));
                    }
                }
            }
        }

        // Sort functions by start_byte for containment lookup.
        functions.sort_by_key(|(s, _, _)| *s);

        for (call_start, call_end, callee) in calls {
            // Find the innermost function that contains this call.
            if let Some((_fs, _fe, caller)) = functions
                .iter()
                .rev()
                .find(|(fs, fe, _)| *fs <= call_start && *fe >= call_end)
            {
                graph.entry(caller.clone()).or_default().push(callee);
            }
        }

        // Ensure all functions appear as nodes (even if they don't call anything).
        for (_, _, name) in &functions {
            graph.entry(name.clone()).or_default();
        }

        graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_empty() {
        let p = RustParser::new();
        let g = p.parse("");
        assert!(g.is_empty());
    }

    #[test]
    fn parse_invalid_syntax_returns_empty() {
        let p = RustParser::new();
        let g = p.parse("fn broken {");
        assert!(g.is_empty());
    }

    #[test]
    fn parse_single_function_no_calls() {
        let p = RustParser::new();
        let code = r#"
fn foo() {
    let x = 1;
}
"#;
        let g = p.parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.get("foo").unwrap().is_empty());
    }

    #[test]
    fn parse_function_calling_another() {
        let p = RustParser::new();
        let code = r#"
fn bar() -> i32 { 42 }

fn foo() {
    bar();
}
"#;
        let g = p.parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.contains_key("bar"));
        assert_eq!(g.get("foo").unwrap(), &vec!["bar".to_string()]);
    }

    #[test]
    fn parse_nested_calls() {
        let p = RustParser::new();
        let code = r#"
fn baz() {}
fn bar() { baz(); }
fn foo() { bar(); }
"#;
        let g = p.parse(code);
        assert_eq!(g.get("foo").unwrap(), &vec!["bar".to_string()]);
        assert_eq!(g.get("bar").unwrap(), &vec!["baz".to_string()]);
        assert!(g.get("baz").unwrap().is_empty());
    }

    #[test]
    fn parse_method_call() {
        let p = RustParser::new();
        let code = r#"
fn foo() {
    let v = vec![1];
    v.len();
}
"#;
        let g = p.parse(code);
        assert!(g.contains_key("foo"));
        let callees = g.get("foo").unwrap();
        assert!(callees.iter().any(|c| c.contains("len")));
    }

    #[test]
    fn parse_multiple_calls_same_callee() {
        let p = RustParser::new();
        let code = r#"
fn bar() {}
fn foo() {
    bar();
    bar();
}
"#;
        let g = p.parse(code);
        let callees = g.get("foo").unwrap();
        assert!(callees.len() >= 1);
        assert!(callees.contains(&"bar".to_string()));
    }
}
