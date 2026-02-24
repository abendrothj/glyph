//! Phase 9: Flow Crawler — extension-router architecture for multi-language call-graph visualization.
//!
//! LanguageParser trait + CrawlerRouter for extension-based dispatch. Tree-sitter Query for Rust MVP.

mod parsers;
mod router;

use bevy::prelude::*;
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

/// Call graph: caller -> list of callees.
pub type CallGraph = HashMap<String, Vec<String>>;

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

/// Compute hierarchy levels: roots (never callees) = 0, callees = 1 + max(caller level).
fn hierarchy_levels(graph: &CallGraph, all_fns: &[String]) -> HashMap<String, usize> {
    let mut callee_to_callers: HashMap<String, Vec<String>> = HashMap::new();
    for (caller, callees) in graph {
        for callee in callees {
            callee_to_callers
                .entry(callee.clone())
                .or_default()
                .push(caller.clone());
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
                let entity = spawn_node_with_color(&mut commands, x, y, name, CRAWL_NODE_COLOR);
                name_to_entity.insert(name.clone(), entity);
            }
        }

        // Spawn edges (dedupe callees per caller). Only link to defined functions.
        let mut edge_count = 0;
        for (caller, callees) in &graph {
            let Some(&source) = name_to_entity.get(caller) else {
                continue;
            };
            let seen: std::collections::HashSet<_> = callees.iter().filter(|c| defined.contains(*c)).collect();
            for callee in seen {
                if let Some(&target) = name_to_entity.get(callee) {
                    if source != target {
                        commands.spawn(Edge {
                            source,
                            target,
                            label: None,
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
