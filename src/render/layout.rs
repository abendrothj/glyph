//! Force-directed layout — nodes repel each other, edges attract.

use bevy::prelude::*;

use crate::core::components::{CanvasNode, Dragging, Edge};

/// When true, force-directed layout runs each frame to untangle nodes.
#[derive(Resource, Default)]
pub struct ForceLayoutActive(pub bool);

/// Repulsion strength between nodes.
const K_REP: f32 = 25000.0;
/// Attraction strength along edges (kept low — dense graphs have many edges, so attraction was overpowering).
const K_ATT: f32 = 0.001;
/// Ideal edge length (spring rest length).
const IDEAL_LEN: f32 = 400.0;
/// Max distance for repulsion (nodes beyond this don't repel — was 600, too small for large graphs).
const MAX_REP_DIST: f32 = 2000.0;
/// Min distance to avoid division by zero.
const MIN_DIST: f32 = 1.0;
/// Damping per frame.
const DAMPING: f32 = 0.9;
/// Time step for stability (slightly higher for faster convergence).
const DT: f32 = 1.0 / 50.0;

/// Apply force-directed layout: repulsion between nodes, attraction along edges.
pub fn force_directed_layout_system(
    layout_active: Res<ForceLayoutActive>,
    mut node_query: Query<(Entity, &mut Transform), With<CanvasNode>>,
    edge_query: Query<&Edge>,
    dragging_query: Query<Entity, With<Dragging>>,
) {
    if !layout_active.0 {
        return;
    }

    let dragging: std::collections::HashSet<Entity> = dragging_query.iter().collect();

    let positions: Vec<(Entity, Vec2)> = node_query
        .iter()
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();

    if positions.len() < 2 {
        return;
    }

    let mut forces: std::collections::HashMap<Entity, Vec2> =
        std::collections::HashMap::with_capacity(positions.len());
    for (e, _) in &positions {
        forces.insert(*e, Vec2::ZERO);
    }

    // Degree (edge count per node) — normalize attraction so high-degree nodes don't get over-pulled.
    let mut degree: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::with_capacity(positions.len());
    for (e, _) in &positions {
        degree.insert(*e, 0);
    }
    for edge in &edge_query {
        degree.entry(edge.source).and_modify(|c| *c += 1).or_insert(1);
        degree.entry(edge.target).and_modify(|c| *c += 1).or_insert(1);
    }

    // Repulsion: each pair of nodes
    for (i, (e1, p1)) in positions.iter().enumerate() {
        for (e2, p2) in positions.iter().skip(i + 1) {
            let delta = *p2 - *p1;
            let d = delta.length().max(MIN_DIST);
            if d > MAX_REP_DIST {
                continue;
            }
            let dir = delta.normalize_or_zero();
            let f = K_REP / (d * d);
            *forces.get_mut(e1).unwrap() -= dir * f;
            *forces.get_mut(e2).unwrap() += dir * f;
        }
    }

    // Attraction: along edges (normalized by sqrt(degree) so hubs don't collapse)
    for edge in &edge_query {
        let p1 = positions.iter().find(|(e, _)| *e == edge.source).map(|(_, p)| *p);
        let p2 = positions.iter().find(|(e, _)| *e == edge.target).map(|(_, p)| *p);
        let (Some(p1), Some(p2)) = (p1, p2) else {
            continue;
        };
        let delta = p2 - p1;
        let d = delta.length().max(MIN_DIST);
        let dir = delta.normalize_or_zero();
        let f = K_ATT * (d - IDEAL_LEN);
        let norm_s = 1.0 / (degree.get(&edge.source).copied().unwrap_or(1) as f32).sqrt();
        let norm_t = 1.0 / (degree.get(&edge.target).copied().unwrap_or(1) as f32).sqrt();
        if let Some(fv) = forces.get_mut(&edge.source) {
            *fv += dir * f * norm_s;
        }
        if let Some(fv) = forces.get_mut(&edge.target) {
            *fv -= dir * f * norm_t;
        }
    }

    // Apply forces
    for (entity, mut transform) in &mut node_query {
        if dragging.contains(&entity) {
            continue;
        }
        let Some(&force) = forces.get(&entity) else {
            continue;
        };
        let delta = force * DT * DAMPING;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}
