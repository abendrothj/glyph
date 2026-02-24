//! Phase 9: Flow Crawler — extension-router architecture for multi-language call-graph visualization.
//!
//! LanguageParser trait + CrawlerRouter for extension-based dispatch. Tree-sitter Query for Rust MVP.

mod parsers;
mod router;

use bevy::prelude::*;
use std::collections::HashMap;
use crate::components::{CanvasNode, Edge};
use crate::helpers::spawn_node_with_color;
use crate::resources::SpatialIndex;

pub use router::CrawlerRouter;

/// Resource for deferred crawl (set by command palette, processed in Update).
#[derive(Resource, Default)]
pub struct PendingCrawl(pub Option<String>);

/// Call graph: caller -> list of callees.
pub type CallGraph = HashMap<String, Vec<String>>;

/// Trait for language-specific AST parsing. Returns caller -> callees map.
pub trait LanguageParser: Send + Sync {
    /// Parse source code and extract call graph. Returns empty map on parse failure (no panic).
    fn parse(&self, code: &str) -> CallGraph;
}

/// Grid layout constants.
const GRID_START_X: f32 = 0.0;
const GRID_START_Y: f32 = 0.0;
const GRID_STEP_X: f32 = 400.0;
const GRID_STEP_Y: f32 = 300.0;
const GRID_MAX_X: f32 = 4000.0;

/// Color for crawled function nodes.
const CRAWL_NODE_COLOR: Color = Color::srgb(0.35, 0.55, 0.45);

/// Process PendingCrawl resource: route through CrawlerRouter, spawn nodes and edges.
pub fn process_crawl_request_system(
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut pending_crawl: ResMut<PendingCrawl>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    let Some(path) = pending_crawl.0.take() else {
        return;
    };
    let path = path.trim();
    if path.is_empty() {
        return;
    }
    let path = path.to_string();

    let graph = CrawlerRouter::crawl(&path);
    if graph.is_empty() {
        warn!("[CRAWL] No functions found in {}", path);
        return;
    }

    // Collect all unique function names (callers + callees).
    let mut all_fns: std::collections::HashSet<String> = graph.keys().cloned().collect();
    for callees in graph.values() {
        all_fns.extend(callees.iter().cloned());
    }

    // Despawn existing nodes and edges.
    for entity in node_query.iter().collect::<Vec<_>>() {
        commands.entity(entity).despawn();
    }
    for entity in edge_entity_query.iter().collect::<Vec<_>>() {
        commands.entity(entity).despawn();
    }
    spatial_index.clear();

    // Sort for deterministic layout.
    let mut sorted: Vec<_> = all_fns.into_iter().collect();
    sorted.sort();

    // Spawn nodes with grid layout.
    let mut name_to_entity: HashMap<String, Entity> = HashMap::new();
    let mut x = GRID_START_X;
    let mut y = GRID_START_Y;

    for name in &sorted {
        let entity = spawn_node_with_color(&mut commands, x, y, name, CRAWL_NODE_COLOR);
        name_to_entity.insert(name.clone(), entity);
        x += GRID_STEP_X;
        if x > GRID_MAX_X {
            x = GRID_START_X;
            y += GRID_STEP_Y;
        }
    }

    // Spawn edges (dedupe callees per caller).
    let mut edge_count = 0;
    for (caller, callees) in &graph {
        let Some(&source) = name_to_entity.get(caller) else {
            continue;
        };
        let seen: std::collections::HashSet<_> = callees.iter().collect();
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

    info!(
        "[CRAWL] Spawned {} nodes, {} edges from {}",
        sorted.len(),
        edge_count,
        path
    );
}
