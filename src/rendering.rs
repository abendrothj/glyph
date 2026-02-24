//! Gizmo and text rendering systems.

use bevy::prelude::*;
use std::f32::consts::PI;

use crate::components::{Edge, EdgeLabel, Selected, TextData, TextLabel};
use crate::state::InputMode;

/// Number of segments for approximating Bezier curves.
const CURVE_SEGMENTS: usize = 24;

/// Quadratic Bezier: B(t) = (1-t)²P0 + 2(1-t)tP1 + t²P2
fn bezier_point(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    u * u * p0 + 2.0 * u * t * p1 + t * t * p2
}

/// Tangent at t for quadratic Bezier (for label rotation).
fn bezier_tangent(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    (2.0 * u * (p1 - p0) + 2.0 * t * (p2 - p1)).normalize_or_zero()
}

/// Draw curved edges using quadratic Bezier. Control point offset perpendicular for clear routing.
/// Edges between the same node pair alternate curve direction for efficient, non-overlapping layout.
pub fn draw_edges_system(
    mut gizmos: Gizmos,
    edge_query: Query<(Entity, &Edge)>,
    transform_query: Query<&Transform>,
) {
    let edge_color = Color::srgb(0.22, 0.32, 0.48);
    // Group edges by (source, target) so we alternate direction within each pair
    let mut groups: std::collections::HashMap<(Entity, Entity), Vec<Entity>> =
        std::collections::HashMap::new();
    for (entity, edge) in &edge_query {
        let key = (edge.source, edge.target);
        groups.entry(key).or_default().push(entity);
    }
    for ((source, target), entities) in groups {
        let Ok(src) = transform_query.get(source) else {
            continue;
        };
        let Ok(tgt) = transform_query.get(target) else {
            continue;
        };
        let p0 = src.translation.truncate();
        let p2 = tgt.translation.truncate();
        let mid = (p0 + p2) * 0.5;
        let dir = (p2 - p0).normalize_or_zero();
        let dist = p0.distance(p2);
        let curve_mag = (dist * 0.12).min(50.0).max(15.0);
        let perp = Vec2::new(-dir.y, dir.x);
        for (idx, _) in entities.iter().enumerate() {
            let sign = if idx % 2 == 0 { 1.0 } else { -1.0 };
            let p1 = mid + perp * curve_mag * sign;
            let mut prev = p0;
            for i in 1..=CURVE_SEGMENTS {
                let t = i as f32 / CURVE_SEGMENTS as f32;
                let pt = bezier_point(p0, p1, p2, t);
                gizmos.line_2d(prev, pt, edge_color);
                prev = pt;
            }
        }
    }
}

/// Spawn/update Text2d labels for edges at midpoint, rotated parallel to the curve.
/// Uses same grouping as draw_edges_system so label sits on the correct curve.
pub fn sync_edge_labels_system(
    mut commands: Commands,
    edge_query: Query<(Entity, &Edge)>,
    children_query: Query<&Children>,
    node_transform_query: Query<&Transform, Without<EdgeLabel>>,
    mut label_query: Query<(&mut Transform, &mut Text2d), With<EdgeLabel>>,
) {
    let mut groups: std::collections::HashMap<(Entity, Entity), Vec<Entity>> =
        std::collections::HashMap::new();
    for (entity, edge) in &edge_query {
        groups
            .entry((edge.source, edge.target))
            .or_default()
            .push(entity);
    }
    let mut idx_map: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for (_, entities) in &groups {
        for (i, e) in entities.iter().enumerate() {
            idx_map.insert(*e, i);
        }
    }
    for (edge_entity, edge) in &edge_query {
        let Some(ref label_text) = edge.label else {
            continue;
        };
        if label_text.is_empty() {
            continue;
        }

        let Ok(src) = node_transform_query.get(edge.source) else {
            continue;
        };
        let Ok(tgt) = node_transform_query.get(edge.target) else {
            continue;
        };

        let p0 = src.translation.truncate();
        let p2 = tgt.translation.truncate();
        let mid = (p0 + p2) * 0.5;
        let dir = (p2 - p0).normalize_or_zero();
        let dist = p0.distance(p2);
        let curve_mag = (dist * 0.12).min(50.0).max(15.0);
        let perp = Vec2::new(-dir.y, dir.x);
        let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
        let sign = if idx % 2 == 0 { 1.0 } else { -1.0 };
        let p1 = mid + perp * curve_mag * sign;
        let mid = bezier_point(p0, p1, p2, 0.5);
        let tangent = bezier_tangent(p0, p1, p2, 0.5);
        let mut angle = tangent.y.atan2(tangent.x);
        if tangent.x < 0.0 {
            angle += PI;
        }

        let label_entity = children_query
            .get(edge_entity)
            .ok()
            .and_then(|c| c.iter().find(|id| label_query.get(*id).is_ok()));

        if let Some(label_entity) = label_entity {
            if let Ok((mut transform, mut text2d)) = label_query.get_mut(label_entity) {
                transform.translation = mid.extend(1.0);
                transform.rotation = Quat::from_rotation_z(angle);
                text2d.clear();
                text2d.push_str(label_text);
            }
        } else {
            commands.entity(edge_entity).insert(Visibility::default());
            let label_entity = commands
                .spawn((
                    Text2d::new(label_text.clone()),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.5, 0.55, 0.65)),
                    Transform::from_xyz(mid.x, mid.y, 1.0)
                        .with_rotation(Quat::from_rotation_z(angle)),
                    EdgeLabel,
                ))
                .id();
            commands.entity(edge_entity).add_child(label_entity);
        }
    }
}

/// Draw a mode-coloured rectangle outline around the selected node.
///
/// VimNormal → blue   VimInsert → green   VimEasymotion → orange
/// Standard  → purple
pub fn draw_selection_system(
    mut gizmos: Gizmos,
    selected_query: Query<&Transform, With<Selected>>,
    state: Res<State<InputMode>>,
) {
    let Ok(transform) = selected_query.single() else {
        return;
    };

    let color = match state.get() {
        InputMode::VimInsert => Color::srgb(0.2, 0.85, 0.4),
        InputMode::VimEasymotion => Color::srgb(1.0, 0.6, 0.1),
        InputMode::Standard => Color::srgb(0.85, 0.4, 0.9),
        InputMode::VimNormal => Color::srgb(0.3, 0.6, 1.0),
    };

    gizmos.rect_2d(
        Isometry2d::from_translation(transform.translation.truncate()),
        Vec2::new(170.0, 130.0),
        color,
    );
}

/// When TextData.content changes, push the new string into the child Text2d.
pub fn sync_text_system(
    changed_nodes: Query<(&TextData, &Children), Changed<TextData>>,
    mut text_query: Query<&mut Text2d, With<TextLabel>>,
) {
    for (text_data, children) in &changed_nodes {
        for child in children {
            if let Ok(mut text2d) = text_query.get_mut(*child) {
                text2d.clear();
                text2d.push_str(&text_data.content);
            }
        }
    }
}
