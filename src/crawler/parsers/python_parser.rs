//! PythonParser — tree-sitter for function_definition and call.

use tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use tree_sitter::StreamingIterator;

use super::super::{CallGraph, LanguageParser};

/// Python: function_definition with name, call with function (identifier or attribute).
const PYTHON_QUERY: &str = r#"
(function_definition
  name: (identifier) @fn_name)
(call
  function: (identifier) @call_name)
(call
  function: (attribute
    attribute: (identifier) @call_name))
"#;

pub struct PythonParser {
    language: Language,
    query: Query,
}

impl PythonParser {
    pub fn new() -> Self {
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let query = Query::new(&language, PYTHON_QUERY).expect("Python query must be valid");
        Self { language, query }
    }

    fn get_text(node: Node, source: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        source.get(start..end).unwrap_or("").trim().to_string()
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for PythonParser {
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
