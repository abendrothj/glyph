use glyph::crawler::LanguageParser;
/// Tests for the Rust logic flow map parser (Phase 9 Crawler).

use glyph::crawler::parsers::rust_parser::RustParser;
use glyph::crawler::{FlowEdge, CallGraph};

fn get_edges(graph: &CallGraph, node: &str) -> Vec<(String, Option<String>)> {
    graph.get(node)
        .map(|edges: &Vec<FlowEdge>| edges.iter().map(|e| (e.target.clone(), e.label.clone())).collect())
        .unwrap_or_default()
}

#[test]
fn test_simple_if_branch() {
    let code = r#"
fn foo(x: i32) {
    if x > 0 {
        bar();
    } else {
        baz();
    }
}
fn bar() {}
fn baz() {}
"#;
    let parser = RustParser::new();
    let graph = parser.parse(code);
    // foo should connect to a decision node
    let foo_edges = get_edges(&graph, "foo");
    assert!(foo_edges.iter().any(|(t, _)| t.starts_with("_decision_")));
    // The decision node should have two labeled edges: True->bar, False->baz
    let decision_id = &foo_edges[0].0;
    let dec_edges = get_edges(&graph, decision_id);
    assert!(dec_edges.iter().any(|(t, l)| t == "bar" && l.as_deref() == Some("True")));
    assert!(dec_edges.iter().any(|(t, l)| t == "baz" && l.as_deref() == Some("False")));
}

#[test]
fn test_match_branch() {
    let code = r#"
fn foo(x: i32) {
    match x {
        1 => bar(),
        2 => baz(),
        _ => qux(),
    }
}
fn bar() {}
fn baz() {}
fn qux() {}
"#;
    let parser = RustParser::new();
    let graph = parser.parse(code);
    let foo_edges = get_edges(&graph, "foo");
    assert!(foo_edges.iter().any(|(t, _)| t.starts_with("_decision_")));
    let decision_id = &foo_edges[0].0;
    let dec_edges = get_edges(&graph, decision_id);
    assert!(dec_edges.iter().any(|(t, l)| t == "bar" && l.as_deref() == Some("1")));
    assert!(dec_edges.iter().any(|(t, l)| t == "baz" && l.as_deref() == Some("2")));
    assert!(dec_edges.iter().any(|(t, l)| t == "qux" && l.as_deref() == Some("_")));
}

#[test]
fn test_loop_branch() {
    let code = r#"
fn foo() {
    for i in 0..3 {
        bar();
    }
}
fn bar() {}
"#;
    let parser = RustParser::new();
    let graph = parser.parse(code);
    let foo_edges = get_edges(&graph, "foo");
    assert!(foo_edges.iter().any(|(t, _)| t.starts_with("_decision_")));
    let decision_id = &foo_edges[0].0;
    let dec_edges = get_edges(&graph, decision_id);
    assert!(dec_edges.iter().any(|(t, l)| t == "bar" && l.as_deref() == Some("Loop")));
}
