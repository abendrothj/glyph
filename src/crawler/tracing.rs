use crate::components::{CanvasNode, Edge, TracedPath};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// Message sent to request tracing a path between two nodes.
#[derive(Message)]
pub struct TraceRequest {
    pub source: String,
    pub sink: String,
}

/// A graph traversal algorithm to find paths between a source and a sink entity.
pub fn find_traced_paths(
    source_entity: Entity,
    sink_entity: Entity,
    edges: &[(Entity, Entity, Entity)], // (edge_entity, source_ent, target_ent)
) -> HashSet<Entity> {
    // Build adjacency list
    let mut adj: HashMap<Entity, Vec<(Entity, Entity)>> = HashMap::new(); // node -> [(edge, next_node)]
    for (edge_ent, src, trg) in edges {
        adj.entry(*src).or_default().push((*edge_ent, *trg));
    }

    // BFS to find all nodes/edges on ANY path from source to sink.
    // Since we just want to highlight reachable paths, we can just do a standard BFS keeping track of paths
    // Or a simple DFS to find paths.
    let mut result_set = HashSet::new();
    let mut visited = HashSet::new();
    let mut path = Vec::new(); // stores (node, edge_used_to_get_here)

    fn dfs(
        current: Entity,
        target: Entity,
        adj: &HashMap<Entity, Vec<(Entity, Entity)>>,
        visited: &mut HashSet<Entity>,
        path: &mut Vec<(Entity, Option<Entity>)>,
        result_set: &mut HashSet<Entity>,
    ) {
        if current == target {
            // Path found, add all nodes and edges in path to result
            for (n, e) in path.iter() {
                result_set.insert(*n);
                if let Some(edge) = e {
                    result_set.insert(*edge);
                }
            }
            result_set.insert(current);
            return;
        }

        visited.insert(current);
        if let Some(neighbors) = adj.get(&current) {
            for (edge, next_node) in neighbors {
                if !visited.contains(next_node) {
                    path.push((*next_node, Some(*edge)));
                    dfs(*next_node, target, adj, visited, path, result_set);
                    path.pop();
                }
            }
        }
        visited.remove(&current);
    }

    path.push((source_entity, None));
    dfs(
        source_entity,
        sink_entity,
        &adj,
        &mut visited,
        &mut path,
        &mut result_set,
    );

    result_set
}

/// System to handle `:trace flow` commands.
pub fn handle_trace_requests(
    mut commands: Commands,
    mut trace_events: MessageReader<TraceRequest>,
    mut status: ResMut<crate::resources::StatusMessage>,
    node_query: Query<(Entity, &crate::components::TextData), With<CanvasNode>>,
    edge_query: Query<(Entity, &Edge)>,
    traced_query: Query<Entity, With<TracedPath>>,
) {
    for ev in trace_events.read() {
        let source_text = ev.source.trim();
        let sink_text = ev.sink.trim();

        if source_text.is_empty() || sink_text.is_empty() {
            status.set("error: :trace flow requires <source> and <sink>");
            continue;
        }

        // Clear previous traces
        for entity in &traced_query {
            commands.entity(entity).remove::<TracedPath>();
        }

        // Find entities matching source and sink texts
        // In the canvas, CanvasNode texts vary - they might be function names or filenames.
        // We'll match if the text exactly matches or contains the request.
        let mut source_ent = None;
        let mut sink_ent = None;

        for (ent, text_data) in &node_query {
            if text_data.content == source_text {
                source_ent = Some(ent);
            }
            if text_data.content == sink_text {
                sink_ent = Some(ent);
            }
        }

        // If exact match not found, try partial match
        if source_ent.is_none() || sink_ent.is_none() {
            for (ent, text_data) in &node_query {
                if source_ent.is_none() && text_data.content.contains(source_text) {
                    source_ent = Some(ent);
                }
                if sink_ent.is_none() && text_data.content.contains(sink_text) {
                    sink_ent = Some(ent);
                }
            }
        }

        let Some(src) = source_ent else {
            status.set(format!(
                "Trace: Could not find source node '{}'",
                source_text
            ));
            continue;
        };

        let Some(snk) = sink_ent else {
            status.set(format!("Trace: Could not find sink node '{}'", sink_text));
            continue;
        };

        let edges: Vec<(Entity, Entity, Entity)> = edge_query
            .iter()
            .map(|(e, edge)| (e, edge.source, edge.target))
            .collect();

        let traced_entities = find_traced_paths(src, snk, &edges);

        if traced_entities.is_empty() {
            status.set(format!(
                "Trace: No path found from '{}' to '{}'",
                source_text, sink_text
            ));
        } else {
            for ent in traced_entities {
                commands.entity(ent).insert(TracedPath);
            }
            status.set(format!("Trace: Path found and highlighted"));
        }
    }
}
