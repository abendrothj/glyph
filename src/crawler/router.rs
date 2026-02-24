//! CrawlerRouter — walkdir-based directory crawler with extension dispatch.

use std::path::Path;
use walkdir::WalkDir;

use super::parsers::python_parser::PythonParser;
use super::parsers::rust_parser::RustParser;
use super::parsers::typescript_parser::TypeScriptParser;
use super::{CallGraph, LanguageParser};

/// Routes files by extension to the appropriate LanguageParser.
/// Uses walkdir to iterate; parse failures are logged and skipped (no panic).
pub struct CrawlerRouter;

impl CrawlerRouter {
    /// Crawl a directory and aggregate call graphs from all supported files.
    pub fn crawl(root: &str) -> CallGraph {
        let mut graph = CallGraph::new();
        let root_path = Path::new(root);
        if !root_path.exists() || !root_path.is_dir() {
            return graph;
        }

        let rust_parser = RustParser::new();
        let python_parser = PythonParser::new();
        let typescript_parser = TypeScriptParser::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let parser: Option<&dyn LanguageParser> = match ext.as_str() {
                "rs" => Some(&rust_parser),
                "py" => Some(&python_parser),
                "ts" | "tsx" => Some(&typescript_parser),
                _ => continue,
            };

            if let Some(p) = parser {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let file_graph = p.parse(&content);
                    for (caller, edges) in file_graph {
                        graph.entry(caller).or_default().extend(edges);
                    }
                }
            }
        }

        // ── Two-pass post-processing ─────────────────────────────────────────
        // Pass 1: collect user-defined function names (non-synthetic keys).
        let defined: std::collections::HashSet<String> = graph
            .keys()
            .filter(|k| !k.starts_with("_decision_"))
            .cloned()
            .collect();

        // Pass 2: filter all edge lists — keep only edges to defined functions
        // or to other decision nodes.  Drops library/external calls that were
        // never parsed (std, third-party crates, etc.).
        for edges in graph.values_mut() {
            edges.retain(|e| {
                e.target.starts_with("_decision_") || defined.contains(&e.target)
            });
        }

        // Pass 3: fixpoint-prune decision nodes that have no remaining outgoing
        // edges.  After removing one empty decision node, its parent may also
        // become empty, so we repeat until stable.
        loop {
            let empty: std::collections::HashSet<String> = graph
                .iter()
                .filter(|(k, v)| k.starts_with("_decision_") && v.is_empty())
                .map(|(k, _)| k.clone())
                .collect();
            if empty.is_empty() {
                break;
            }
            for id in &empty {
                graph.remove(id);
            }
            for edges in graph.values_mut() {
                edges.retain(|e| !empty.contains(&e.target));
            }
        }

        graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn crawl_nonexistent_returns_empty() {
        let g = CrawlerRouter::crawl("/nonexistent/path/12345");
        assert!(g.is_empty());
    }

    #[test]
    fn crawl_empty_string_returns_empty() {
        let g = CrawlerRouter::crawl("");
        assert!(g.is_empty());
    }

    #[test]
    fn crawl_directory_with_rust_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        fs::write(
            dir_path.join("mod.rs"),
            r#"
fn helper() {}
pub fn public_api() { helper(); }
"#,
        )
        .unwrap();
        fs::write(dir_path.join("other.py"), "def foo(): pass").unwrap();

        let g = CrawlerRouter::crawl(dir_path.to_str().unwrap());
        assert!(g.contains_key("public_api"));
        assert!(g.contains_key("helper"));
        let public_api_edges = g.get("public_api").unwrap();
        assert_eq!(public_api_edges.len(), 1);
        assert_eq!(public_api_edges[0].target, "helper");
    }

    #[test]
    fn crawl_directory_with_python_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        fs::write(
            dir_path.join("main.py"),
            r#"
def bar():
    pass

def foo():
    bar()
"#,
        )
        .unwrap();

        let g = CrawlerRouter::crawl(dir_path.to_str().unwrap());
        assert!(g.contains_key("foo"));
        assert!(g.contains_key("bar"));
        let foo_edges = g.get("foo").unwrap();
        assert_eq!(foo_edges.len(), 1);
        assert_eq!(foo_edges[0].target, "bar");
    }
}
