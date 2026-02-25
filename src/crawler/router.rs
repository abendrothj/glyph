//! CrawlerRouter — walkdir-based directory crawler with extension dispatch.

use std::path::Path;
use walkdir::WalkDir;

use super::parsers::python_parser::PythonParser;
use super::parsers::rust_parser::RustParser;
use super::parsers::typescript_parser::TypeScriptParser;
use super::parsers::walker::DECISION_SEP;
use super::{CallGraph, FlowEdge, LanguageParser, SourceMap};

/// Routes files by extension to the appropriate LanguageParser.
/// Uses walkdir to iterate; parse failures are logged and skipped (no panic).
pub struct CrawlerRouter;

impl CrawlerRouter {
    /// Crawl a directory and aggregate call graphs from all supported files.
    ///
    /// Node IDs are namespaced as `relative/path.rs::function_name` so that
    /// identically-named functions in different files remain distinct.  Decision
    /// nodes follow the same prefix convention:
    /// `relative/path.rs::_decision_N\x1FDISPLAY_TEXT`.
    ///
    /// Cross-file calls are resolved in a two-pass approach: first collect every
    /// defined name and its declaring file(s), then rewrite edge targets to the
    /// namespaced form.  When a name is defined in more than one file an edge is
    /// emitted to each definition so ambiguity is visible in the graph.
    pub fn crawl(root: &str) -> (CallGraph, SourceMap) {
        let root_path = Path::new(root);
        if !root_path.exists() || !root_path.is_dir() {
            return (CallGraph::new(), SourceMap::new());
        }

        let rust_parser = RustParser::new();
        let python_parser = PythonParser::new();
        let typescript_parser = TypeScriptParser::new();

        // ── Phase 1: per-file parse ───────────────────────────────────────────
        // Collect (rel_path, abs_path, bare_call_graph, line_numbers).
        let mut per_file: Vec<(String, String, CallGraph, std::collections::HashMap<String, u32>)> = Vec::new();
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
                    let rel = path
                        .strip_prefix(root_path)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned();

                    // Skip test directories (tests/, test/) and test-named files
                    // (test_*.py, *_test.rs, *.test.ts, *.spec.ts).
                    let rel_norm = rel.replace('\\', "/");
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if rel_norm.starts_with("tests/")
                        || rel_norm.contains("/tests/")
                        || rel_norm.starts_with("test/")
                        || rel_norm.contains("/test/")
                        || stem.starts_with("test_")
                        || stem.ends_with("_test")
                        || rel_norm.contains(".test.")
                        || rel_norm.contains(".spec.")
                    {
                        continue;
                    }

                    let abs = path.to_string_lossy().into_owned();
                    let (file_graph, line_numbers) = p.parse_with_lines(&content);
                    if !file_graph.is_empty() {
                        per_file.push((rel, abs, file_graph, line_numbers));
                    }
                }
            }
        }

        // ── Phase 2: bare-name → [declaring files] index ─────────────────────
        // Only regular function keys are indexed (decision nodes are file-local).
        let mut defined_in: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (path, _abs, file_graph, _lines) in &per_file {
            for key in file_graph.keys() {
                if !key.contains(DECISION_SEP) {
                    defined_in.entry(key.clone()).or_default().push(path.clone());
                }
            }
        }

        // ── Phase 3: build namespaced graph + source map ─────────────────────
        // Key format:  `relative/path.rs::bare_name`
        // Decision IDs keep their full form after the `::` separator:
        //   `relative/path.rs::_decision_1\x1Fif x > 0`
        // Call targets are resolved via the index built in Phase 2.
        // source_map: ns_key → (abs_path, 1-indexed line) for each function node.
        let mut graph = CallGraph::new();
        let mut source_map = SourceMap::new();
        for (path, abs, file_graph, line_numbers) in per_file {
            for (bare_key, bare_edges) in file_graph {
                let ns_key = format!("{}::{}", path, bare_key);
                let ns_edges: Vec<FlowEdge> = bare_edges
                    .into_iter()
                    .flat_map(|edge| {
                        let target = &edge.target;
                        if target.contains(DECISION_SEP) {
                            // Decision nodes are always file-local — prefix with same path.
                            vec![FlowEdge {
                                target: format!("{}::{}", path, target),
                                label: edge.label,
                            }]
                        } else {
                            // Regular call — resolve to all declaring files.
                            // Undefined names (external libs) produce no edges here;
                            // the post-processing pass below is a safety net.
                            match defined_in.get(target) {
                                None => vec![],
                                Some(files) => files
                                    .iter()
                                    .map(|f| FlowEdge {
                                        target: format!("{}::{}", f, target),
                                        label: edge.label.clone(),
                                    })
                                    .collect(),
                            }
                        }
                    })
                    .collect();
                // Record source location for regular function nodes.
                if !bare_key.contains(DECISION_SEP) {
                    if let Some(&line) = line_numbers.get(&bare_key) {
                        source_map.insert(ns_key.clone(), (abs.clone(), line));
                    }
                }
                graph.insert(ns_key, ns_edges);
            }
        }

        // ── Post-processing ───────────────────────────────────────────────────
        // Decision nodes are identified by the presence of DECISION_SEP in the key
        // (they follow the pattern `file::_decision_N\x1FTEXT`).

        // Pass 1: collect defined function keys (non-decision).
        let defined: std::collections::HashSet<String> = graph
            .keys()
            .filter(|k| !k.contains(DECISION_SEP))
            .cloned()
            .collect();

        // Pass 2: drop any edges that somehow still point outside the defined set.
        for edges in graph.values_mut() {
            edges.retain(|e| e.target.contains(DECISION_SEP) || defined.contains(&e.target));
        }

        // Pass 3: fixpoint-prune decision nodes with no outgoing edges.
        loop {
            let empty: std::collections::HashSet<String> = graph
                .iter()
                .filter(|(k, v)| k.contains(DECISION_SEP) && v.is_empty())
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

        (graph, source_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn crawl_nonexistent_returns_empty() {
        let (g, _) = CrawlerRouter::crawl("/nonexistent/path/12345");
        assert!(g.is_empty());
    }

    #[test]
    fn crawl_empty_string_returns_empty() {
        let (g, _) = CrawlerRouter::crawl("");
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

        let (g, src) = CrawlerRouter::crawl(dir_path.to_str().unwrap());
        // Keys are now namespaced as `relative_path::function_name`.
        assert!(g.contains_key("mod.rs::public_api"), "expected mod.rs::public_api in {:?}", g.keys().collect::<Vec<_>>());
        assert!(g.contains_key("mod.rs::helper"));
        let public_api_edges = g.get("mod.rs::public_api").unwrap();
        assert_eq!(public_api_edges.len(), 1);
        assert_eq!(public_api_edges[0].target, "mod.rs::helper");
        // Source map should contain a line number for public_api.
        assert!(src.contains_key("mod.rs::public_api"), "source_map missing public_api");
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

        let (g, _src) = CrawlerRouter::crawl(dir_path.to_str().unwrap());
        assert!(g.contains_key("main.py::foo"), "expected main.py::foo in {:?}", g.keys().collect::<Vec<_>>());
        assert!(g.contains_key("main.py::bar"));
        let foo_edges = g.get("main.py::foo").unwrap();
        assert_eq!(foo_edges.len(), 1);
        assert_eq!(foo_edges[0].target, "main.py::bar");
    }
}
