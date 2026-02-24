//! CrawlerRouter — walkdir-based directory crawler with extension dispatch.

use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

use super::parsers::python_parser::PythonParser;
use super::parsers::rust_parser::RustParser;
use super::parsers::typescript_parser::TypeScriptParser;
use super::LanguageParser;

/// Call graph: caller -> list of callees.
pub type CallGraph = HashMap<String, Vec<String>>;

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
            .follow_links(true)
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
                    for (caller, callees) in file_graph {
                        graph.entry(caller).or_default().extend(callees);
                    }
                }
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
        assert_eq!(g.get("public_api").unwrap(), &vec!["helper".to_string()]);
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
        assert_eq!(g.get("foo").unwrap(), &vec!["bar".to_string()]);
    }
}
