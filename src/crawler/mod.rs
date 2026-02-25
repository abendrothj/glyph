//! Phase 9: Flow Crawler — extension-router architecture for multi-language call-graph visualization.
//!
//! LanguageParser trait + CrawlerRouter for extension-based dispatch. Tree-sitter Query for Rust MVP.

pub mod parsers;
mod router;
pub mod tracing;

use crate::core::components::{CanvasNode, Edge, FileLabel, SourceLocation};
use crate::core::helpers::spawn_node_with_color;
use crate::render::layout::ForceLayoutActive;
use crate::core::resources::SpatialIndex;
use bevy::prelude::*;
use parsers::walker::DECISION_SEP;
use std::collections::HashMap;
use std::path::Path;

pub use router::CrawlerRouter;
pub use tracing::TraceRequest;

// ── File-system watcher ────────────────────────────────────────────────────────

/// Holds the active `notify` watcher and its event channel.
/// Wrapped in `Mutex` so the resource satisfies Bevy's `Send + Sync` requirement;
/// systems always hold `ResMut<WatchState>` (exclusive access) so lock contention
/// is impossible in practice.
#[derive(Resource)]
pub struct WatchState {
    /// Keeps the watcher alive (dropping it stops watching).
    _watcher: std::sync::Mutex<Option<notify::RecommendedWatcher>>,
    /// Receives raw file-system events.
    rx: std::sync::Mutex<Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>>,
    /// Path currently being watched (passed to re-crawl).
    pub watch_path: Option<String>,
    /// Preserve the `no_flow` setting used for the last crawl.
    pub no_flow: bool,
    /// Time of the most recent relevant file-change event (for debouncing).
    last_event: Option<std::time::Instant>,
}

impl Default for WatchState {
    fn default() -> Self {
        Self {
            _watcher: std::sync::Mutex::new(None),
            rx: std::sync::Mutex::new(None),
            watch_path: None,
            no_flow: false,
            last_event: None,
        }
    }
}

/// Checks the watcher channel for source-file changes and fires a re-crawl
/// after a 500 ms debounce window.
pub fn watch_trigger_system(
    mut watch: ResMut<WatchState>,
    mut crawl_events: MessageWriter<CrawlRequest>,
) {
    // Drain pending events.
    let has_event = if let Ok(rx_guard) = watch.rx.try_lock() {
        if let Some(rx) = rx_guard.as_ref() {
            let mut found = false;
            loop {
                match rx.try_recv() {
                    Ok(Ok(ev)) => {
                        let is_source = ev.paths.iter().any(|p| {
                            p.extension()
                                .and_then(|e| e.to_str())
                                .map_or(false, |e| matches!(e, "rs" | "py" | "ts" | "tsx"))
                        });
                        if is_source {
                            found = true;
                        }
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }
            found
        } else {
            false
        }
    } else {
        false
    };

    if has_event {
        watch.last_event = Some(std::time::Instant::now());
    }

    // Fire re-crawl once the debounce window (500 ms) has elapsed.
    if let Some(last) = watch.last_event {
        if last.elapsed() >= std::time::Duration::from_millis(500) {
            watch.last_event = None;
            if let Some(path) = watch.watch_path.clone() {
                info!("[WATCH] Re-crawling {} (file changed)", path);
                crawl_events.write(CrawlRequest {
                    path,
                    no_flow: watch.no_flow,
                });
            }
        }
    }
}

/// Message sent when user requests a crawl (e.g. from Command Palette).
#[derive(Message)]
pub struct CrawlRequest {
    pub path: String,
    /// When `true`, decision nodes (if/for/while/match) are suppressed and the
    /// resulting graph contains only function nodes. Pass `--no-flow` to `:crawl`.
    pub no_flow: bool,
}

/// FlowEdge: labeled edge in the flow map.
#[derive(Clone, Debug)]
pub struct FlowEdge {
    pub target: String,
    pub label: Option<String>,
}

/// Flow map: node_id -> list of outgoing edges (with labels for branches).
pub type FlowMap = HashMap<String, Vec<FlowEdge>>;

/// CallGraph is now an alias for FlowMap for compatibility.
pub type CallGraph = FlowMap;

/// Maps a namespaced node ID → (absolute file path, 1-indexed line number).
/// Built by the router from tree-sitter line information; used to power `gd`.
pub type SourceMap = HashMap<String, (String, u32)>;

/// Trait for language-specific AST parsing. Returns caller -> callees map.
pub trait LanguageParser: Send + Sync {
    /// Parse source code and extract call graph. Returns empty map on parse failure (no panic).
    fn parse(&self, code: &str) -> CallGraph;

    /// Like `parse` but also returns a bare-name → 1-indexed line-number map for each
    /// defined function.  Default delegates to `parse` with an empty line map; override
    /// for accuracy.
    ///
    /// `no_flow` suppresses control-flow decision nodes; see [`walk_tree`].
    fn parse_with_lines(&self, code: &str, no_flow: bool) -> (CallGraph, HashMap<String, u32>) {
        let _ = no_flow;
        (self.parse(code), HashMap::new())
    }
}

/// Color for crawled function nodes.
const CRAWL_NODE_COLOR: Color = Color::srgb(0.35, 0.55, 0.45);
/// Color for decision (branch) nodes.
const DECISION_NODE_COLOR: Color = Color::srgb(0.85, 0.65, 0.15); // gold/amber

/// Compute hierarchy levels: roots (never callees) = 0, callees = 1 + max(caller level).
fn hierarchy_levels(graph: &CallGraph, all_fns: &[String]) -> HashMap<String, usize> {
    let mut callee_to_callers: HashMap<String, Vec<String>> = HashMap::new();
    for (caller, edges) in graph {
        for edge in edges {
            // Exclude self-calls: a self-recursive function with no external callers
            // should still be treated as a root (level 0), not sink to the bottom.
            if edge.target != *caller {
                callee_to_callers
                    .entry(edge.target.clone())
                    .or_default()
                    .push(caller.clone());
            }
        }
    }

    let mut level: HashMap<String, usize> = HashMap::new();
    for name in all_fns {
        level.insert(
            name.clone(),
            if callee_to_callers.contains_key(name) {
                usize::MAX
            } else {
                0
            },
        );
    }

    let mut changed = true;
    let max_iterations = all_fns.len() + 2;
    let mut iteration = 0;
    for _ in 0..max_iterations {
        if !changed {
            break;
        }
        changed = false;
        iteration += 1;
        for name in all_fns {
            let Some(callers) = callee_to_callers.get(name) else {
                continue;
            };
            let max_caller = callers
                .iter()
                .filter_map(|c| level.get(c))
                .filter(|&l| *l != usize::MAX)
                .max()
                .copied();
            let Some(mc) = max_caller else {
                continue;
            };
            let new_lvl = mc + 1;
            if level.get(name).copied().unwrap_or(0) > new_lvl {
                level.insert(name.clone(), new_lvl);
                changed = true;
            }
        }
    }

    if iteration >= max_iterations && changed {
        warn!("[CRAWL] Cycle detected in call graph during hierarchy layout");
    }

    let max_lvl = level
        .values()
        .copied()
        .filter(|&l| l != usize::MAX)
        .max()
        .unwrap_or(0);
    for (_name, lvl) in level.iter_mut() {
        if *lvl == usize::MAX {
            *lvl = max_lvl + 1;
        }
    }
    level
}

/// Ingestion system: listen for CrawlRequest, use CrawlerRouter, spawn nodes and edges.
pub fn handle_crawl_requests(
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut force_layout: ResMut<ForceLayoutActive>,
    mut crawl_events: MessageReader<CrawlRequest>,
    mut watch_state: ResMut<WatchState>,
    mut status: ResMut<crate::core::resources::StatusMessage>,
    config: Res<crate::core::config::GlyphConfig>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    for ev in crawl_events.read() {
        let path = ev.path.trim();
        if path.is_empty() {
            continue;
        }

        // Resolve relative paths (../.. , ./src, etc.) to absolute so that
        // SourceMap entries are always absolute and `gd` works correctly.
        let abs_root = std::path::Path::new(path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(path));

        if !abs_root.exists() || !abs_root.is_dir() {
            let msg = format!("crawl: path not found: {}", path);
            warn!("[CRAWL] {}", msg);
            status.set(msg);
            continue;
        }

        let abs_root_str = abs_root.to_string_lossy().into_owned();
        let (graph, source_map) = CrawlerRouter::crawl(&abs_root_str, ev.no_flow);
        if graph.is_empty() {
            let msg = format!("crawl: no functions found in {}", path);
            warn!("[CRAWL] No functions found in {}", path);
            status.set(msg);
            continue;
        }

        // Only include functions defined in the codebase (graph.keys()). Filter out std/method
        // calls like as_mut, unwrap, iter, etc. that the parser picks up.
        let defined: std::collections::HashSet<String> = graph.keys().cloned().collect();

        // Despawn existing nodes and edges.
        for entity in node_query.iter().collect::<Vec<_>>() {
            commands.entity(entity).despawn();
        }
        for entity in edge_entity_query.iter().collect::<Vec<_>>() {
            commands.entity(entity).despawn();
        }
        spatial_index.clear();

        // Sort for deterministic layout.
        let mut sorted: Vec<_> = defined.iter().cloned().collect();
        sorted.sort();

        // Hierarchical flow layout: roots at top, callees below.
        let levels = hierarchy_levels(&graph, &sorted);
        let mut by_level: HashMap<usize, Vec<String>> = HashMap::new();
        for name in &sorted {
            let lvl = levels.get(name).copied().unwrap_or(0);
            by_level.entry(lvl).or_default().push(name.clone());
        }
        let mut level_order: Vec<_> = by_level.keys().copied().collect();
        level_order.sort();

        let mut name_to_entity: HashMap<String, Entity> = HashMap::new();
        for lvl in level_order {
            let mut names = by_level.get(&lvl).cloned().unwrap_or_default();
            names.sort();
            let row_len = names.len();
            let y = -(lvl as f32) * config.flow_row_height;
            for (i, name) in names.iter().enumerate() {
                let x = (i as f32 - row_len as f32 * 0.5) * config.flow_node_spacing;
                // Node IDs are namespaced: `relative/path.rs::function_name`
                // Decision nodes: `relative/path.rs::_decision_N\x1FDISPLAY_TEXT`
                // Detect by DECISION_SEP presence (only decision nodes contain it).
                let is_decision = name.contains(DECISION_SEP);
                let color = if is_decision {
                    DECISION_NODE_COLOR
                } else {
                    CRAWL_NODE_COLOR
                };
                // Strip the namespace prefix (split at first "::"), then strip the
                // decision-node ID prefix (split at DECISION_SEP) to get display text.
                let after_ns = name.splitn(2, "::").nth(1).unwrap_or(name.as_str());
                let display_name = after_ns.splitn(2, DECISION_SEP).nth(1).unwrap_or(after_ns);
                let entity = spawn_node_with_color(&mut commands, x, y, display_name, color);
                name_to_entity.insert(name.clone(), entity);

                // Attach source location (for gd) and file label only on function nodes.
                if !is_decision {
                    if let Some((abs_file, line)) = source_map.get(name) {
                        commands.entity(entity).insert(SourceLocation {
                            file: abs_file.clone(),
                            line: *line,
                        });
                    }
                    // Small filename label at the bottom of the node.
                    let rel_path = name.splitn(2, "::").next().unwrap_or("");
                    let basename = Path::new(rel_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(rel_path);
                    let basename = basename.to_string();
                    commands.entity(entity).with_children(|parent| {
                        parent.spawn((
                            Text2d::new(basename),
                            TextFont {
                                font_size: 9.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.65, 0.70, 0.75, 0.65)),
                            Transform::from_xyz(0.0, -48.0, 1.0),
                            FileLabel,
                        ));
                    });
                }
            }
        }

        // Spawn edges with labels (for flow map). Only link to defined or decision nodes.
        let mut edge_count = 0;
        for (caller, edges) in &graph {
            let Some(&source) = name_to_entity.get(caller) else {
                continue;
            };
            for edge in edges {
                let target_name = &edge.target;
                if let Some(&target) = name_to_entity.get(target_name) {
                    if source != target {
                        commands.spawn(Edge {
                            source,
                            target,
                            label: edge.label.clone(),
                        });
                        edge_count += 1;
                    }
                }
            }
        }

        force_layout.active = false; // hierarchy layout — no force collapse

        let node_count = sorted.len();
        info!(
            "[CRAWL] Spawned {} nodes, {} edges from {}",
            node_count, edge_count, abs_root_str
        );
        status.set(format!(
            "Crawled: {} nodes, {} edges",
            node_count, edge_count
        ));

        // ── Start/restart the file-system watcher ────────────────────────────
        watch_state.no_flow = ev.no_flow;
        watch_state.watch_path = Some(abs_root_str.clone());
        watch_state.last_event = None;

        use notify::{RecommendedWatcher, RecursiveMode, Watcher};
        let abs = abs_root.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(mut watcher) => {
                if watcher.watch(&abs, RecursiveMode::Recursive).is_ok() {
                    if let (Ok(mut w), Ok(mut r)) =
                        (watch_state._watcher.lock(), watch_state.rx.lock())
                    {
                        *w = Some(watcher);
                        *r = Some(rx);
                    }
                    info!("[WATCH] Watching {} for changes", abs.display());
                }
            }
            Err(e) => warn!("[WATCH] Could not create watcher: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_levels_simple_dag() {
        let mut graph = CallGraph::new();
        graph.insert("a".into(), vec![FlowEdge { target: "b".into(), label: None }]);
        graph.insert("b".into(), vec![]);
        let levels = hierarchy_levels(&graph, &["a".into(), "b".into()]);
        assert_eq!(levels["a"], 0);
        assert_eq!(levels["b"], 1);
    }

    #[test]
    fn hierarchy_levels_direct_cycle() {
        let mut graph = CallGraph::new();
        graph.insert("a".into(), vec![FlowEdge { target: "b".into(), label: None }]);
        graph.insert("b".into(), vec![FlowEdge { target: "a".into(), label: None }]);
        let levels = hierarchy_levels(&graph, &["a".into(), "b".into()]);
        // Both should get valid levels (not usize::MAX) — cycle is handled gracefully
        assert!(levels["a"] < usize::MAX);
        assert!(levels["b"] < usize::MAX);
    }

    #[test]
    fn hierarchy_levels_three_node_cycle() {
        let mut graph = CallGraph::new();
        graph.insert("a".into(), vec![FlowEdge { target: "b".into(), label: None }]);
        graph.insert("b".into(), vec![FlowEdge { target: "c".into(), label: None }]);
        graph.insert("c".into(), vec![FlowEdge { target: "a".into(), label: None }]);
        let levels = hierarchy_levels(&graph, &["a".into(), "b".into(), "c".into()]);
        for lvl in levels.values() {
            assert!(*lvl < usize::MAX);
        }
    }

    #[test]
    fn hierarchy_levels_mixed_dag_and_cycle() {
        // root -> a -> b -> a (cycle), root -> c (no cycle)
        let mut graph = CallGraph::new();
        graph.insert("root".into(), vec![
            FlowEdge { target: "a".into(), label: None },
            FlowEdge { target: "c".into(), label: None },
        ]);
        graph.insert("a".into(), vec![FlowEdge { target: "b".into(), label: None }]);
        graph.insert("b".into(), vec![FlowEdge { target: "a".into(), label: None }]);
        graph.insert("c".into(), vec![]);
        let fns: Vec<String> = vec!["root".into(), "a".into(), "b".into(), "c".into()];
        let levels = hierarchy_levels(&graph, &fns);
        assert_eq!(levels["root"], 0);
        assert_eq!(levels["c"], 1);
        // a and b are in a cycle but reachable from root — should get valid levels
        assert!(levels["a"] < usize::MAX);
        assert!(levels["b"] < usize::MAX);
    }
}
