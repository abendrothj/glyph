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

/// Grid layout constants: start at (0,0), x += 500, wrap every 10 nodes (y += 400).
const GRID_START_X: f32 = 0.0;
const GRID_START_Y: f32 = 0.0;
const GRID_STEP_X: f32 = 500.0;
const GRID_STEP_Y: f32 = 400.0;
const NODES_PER_ROW: usize = 10;

/// Color for crawled function nodes.
const CRAWL_NODE_COLOR: Color = Color::srgb(0.35, 0.55, 0.45);

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

        // Spawn nodes: grid layout — start (0,0), x += 500, wrap every 10 nodes (y += 400).
        let mut name_to_entity: HashMap<String, Entity> = HashMap::new();
        for (i, name) in sorted.iter().enumerate() {
            let row = i / NODES_PER_ROW;
            let col = i % NODES_PER_ROW;
            let x = GRID_START_X + col as f32 * GRID_STEP_X;
            let y = GRID_START_Y + row as f32 * GRID_STEP_Y;
            let entity = spawn_node_with_color(&mut commands, x, y, name, CRAWL_NODE_COLOR);
            name_to_entity.insert(name.clone(), entity);
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

        force_layout.0 = true;

        info!(
            "[CRAWL] Spawned {} nodes, {} edges from {}",
            sorted.len(),
            edge_count,
            path
        );
    }
}
