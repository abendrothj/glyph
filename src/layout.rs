//! Force-directed layout — nodes repel each other, edges attract.

use bevy::prelude::*;

use crate::components::{CanvasNode, Dragging, Edge};

/// When true, force-directed layout runs each frame to untangle nodes.
#[derive(Resource, Default)]
pub struct ForceLayoutActive(pub bool);

/// Repulsion strength between nodes.
const K_REP: f32 = 8000.0;
/// Attraction strength along edges.
const K_ATT: f32 = 0.008;
/// Ideal edge length (spring rest length).
const IDEAL_LEN: f32 = 250.0;
/// Max distance for repulsion (avoid tiny forces from far nodes).
const MAX_REP_DIST: f32 = 600.0;
/// Min distance to avoid division by zero.
const MIN_DIST: f32 = 1.0;
/// Damping per frame.
const DAMPING: f32 = 0.85;
/// Time step for stability.
const DT: f32 = 1.0 / 60.0;

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

    // Attraction: along edges
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
        if let Some(fv) = forces.get_mut(&edge.source) {
            *fv += dir * f;
        }
        if let Some(fv) = forces.get_mut(&edge.target) {
            *fv -= dir * f;
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
