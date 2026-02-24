//! Phase 9: Flow Crawler — extension-router architecture for multi-language call-graph visualization.
//!
//! LanguageParser trait + CrawlerRouter for extension-based dispatch. Tree-sitter Query for Rust MVP.

pub mod parsers;
mod router;

use bevy::prelude::*;
use parsers::walker::DECISION_SEP;
use std::collections::HashMap;
use crate::components::{CanvasNode, Edge};
use crate::helpers::spawn_node_with_color;
use crate::layout::ForceLayoutActive;
use crate::resources::SpatialIndex;

pub use router::CrawlerRouter;

/// Message sent when user requests a crawl (e.g. from Command Palette).
#[derive(Message)]
pub struct CrawlRequest {
    pub path: String,
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

/// Trait for language-specific AST parsing. Returns caller -> callees map.
pub trait LanguageParser: Send + Sync {
    /// Parse source code and extract call graph. Returns empty map on parse failure (no panic).
    fn parse(&self, code: &str) -> CallGraph;
}

/// Hierarchical flow layout: roots at top, callees below. No force layout — stays spread.
const FLOW_ROW_HEIGHT: f32 = 380.0;
const FLOW_NODE_SPACING: f32 = 320.0;


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
        level.insert(name.clone(), if callee_to_callers.contains_key(name) { usize::MAX } else { 0 });
    }

    let mut changed = true;
    for _ in 0..all_fns.len() + 2 {
        if !changed {
            break;
        }
        changed = false;
        for name in all_fns {
            let Some(callers) = callee_to_callers.get(name) else {
                continue;
            };
            let max_caller = callers.iter().filter_map(|c| level.get(c)).filter(|&l| *l != usize::MAX).max().copied();
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

    let max_lvl = level.values().copied().filter(|&l| l != usize::MAX).max().unwrap_or(0);
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
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    for ev in crawl_events.read() {
        let path = ev.path.trim();
        if path.is_empty() {
            continue;
        }

        let graph = CrawlerRouter::crawl(path);
        if graph.is_empty() {
            warn!("[CRAWL] No functions found in {}", path);
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
            let y = -(lvl as f32) * FLOW_ROW_HEIGHT;
            for (i, name) in names.iter().enumerate() {
                let x = (i as f32 - row_len as f32 * 0.5) * FLOW_NODE_SPACING;
                // Decision nodes have special prefix
                let is_decision = name.starts_with("_decision_");
                let color = if is_decision { DECISION_NODE_COLOR } else { CRAWL_NODE_COLOR };
                // Decision node keys encode the display label after DECISION_SEP.
                // e.g. `_decision_1\x1Fif x > 0` → display `if x > 0`.
                let display_name =
                    name.splitn(2, DECISION_SEP).nth(1).unwrap_or(name.as_str());
                let entity = spawn_node_with_color(&mut commands, x, y, display_name, color);
                name_to_entity.insert(name.clone(), entity);
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

        force_layout.0 = false; // hierarchy layout — no force collapse

        info!(
            "[CRAWL] Spawned {} nodes, {} edges from {}",
            sorted.len(),
            edge_count,
            path
        );
    }
}
