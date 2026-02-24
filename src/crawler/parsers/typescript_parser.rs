//! TypeScriptParser — tree-sitter for function declarations and call expressions.

use tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use tree_sitter::StreamingIterator;

use super::super::{CallGraph, LanguageParser};

/// TypeScript: function_declaration, method_definition; call_expression.
const TYPESCRIPT_QUERY: &str = r#"
(function_declaration
  name: (identifier) @fn_name)
(method_definition
  name: (property_identifier) @fn_name)
(call_expression
  function: (identifier) @call_name)
(call_expression
  function: (member_expression
    property: (property_identifier) @call_name))
"#;

pub struct TypeScriptParser {
    language: Language,
    query: Query,
}

impl TypeScriptParser {
    pub fn new() -> Self {
        let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let query = Query::new(&language, TYPESCRIPT_QUERY).expect("TypeScript query must be valid");
        Self { language, query }
    }

    fn get_text(node: Node, source: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        source.get(start..end).unwrap_or("").trim().to_string()
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for TypeScriptParser {
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
                    let (start, end) = node
                        .parent()
                        .map(|p: Node| (p.start_byte(), p.end_byte()))
                        .unwrap_or((node.start_byte(), node.end_byte()));
                    functions.push((start, end, text));
                } else if cap.index == call_name_idx {
                    calls.push((node.start_byte(), node.end_byte(), text));
                }
            }
        }

        functions.sort_by_key(|(s, _, _)| *s);

        for (call_start, call_end, callee) in calls {
            if let Some((_fs, _fe, caller)) = functions
                .iter()
                .rev()
                .find(|(fs, fe, _)| *fs <= call_start && *fe >= call_end)
            {
                graph.entry(caller.clone()).or_default().push(callee);
            }
        }

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
        let p = TypeScriptParser::new();
        assert!(p.parse("").is_empty());
    }

    #[test]
    fn parse_single_function_no_calls() {
        let p = TypeScriptParser::new();
        let code = "function foo() {}";
        let g = p.parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.get("foo").unwrap().is_empty());
    }

    #[test]
    fn parse_function_calling_another() {
        let p = TypeScriptParser::new();
        let code = r#"
function bar() {}
function foo() { bar(); }
"#;
        let g = p.parse(code);
        assert!(g.contains_key("foo"));
        assert!(g.contains_key("bar"));
        assert_eq!(g.get("foo").unwrap(), &vec!["bar".to_string()]);
    }
}
