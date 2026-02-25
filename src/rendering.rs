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
        // Larger offset so curves are clearly visible on the canvas.
        let curve_mag = (dist * 0.35).clamp(35.0, 180.0);
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

/// Label offset above the curve (world units). Same for hit-testing.
const LABEL_OFFSET_ABOVE: f32 = 18.0;
/// Half-extents of the label hit box (world units).
pub const LABEL_HIT_HALF: Vec2 = Vec2::new(50.0, 12.0);

/// Compute label world position for an edge (above curve midpoint). Used by sync and hit-test.
pub fn edge_label_world_pos(
    src: &Transform,
    tgt: &Transform,
    idx: usize,
) -> (Vec2, f32) {
    let p0 = src.translation.truncate();
    let p2 = tgt.translation.truncate();
    let mid = (p0 + p2) * 0.5;
    let dir = (p2 - p0).normalize_or_zero();
    let dist = p0.distance(p2);
    let curve_mag = (dist * 0.35).clamp(35.0, 180.0);
    let perp = Vec2::new(-dir.y, dir.x);
    let sign = if idx % 2 == 0 { 1.0 } else { -1.0 };
    let p1 = mid + perp * curve_mag * sign;
    let mid_bezier = bezier_point(p0, p1, p2, 0.5);
    let tangent = bezier_tangent(p0, p1, p2, 0.5);
    let mut angle = tangent.y.atan2(tangent.x);
    if tangent.x < 0.0 {
        angle += PI;
    }
    let label_pos = mid_bezier + perp * sign * LABEL_OFFSET_ABOVE;
    (label_pos, angle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_label_world_pos_symmetric() {
        let src = Transform::from_xyz(0.0, 0.0, 0.0);
        let tgt = Transform::from_xyz(200.0, 0.0, 0.0);
        let (pos0, _) = edge_label_world_pos(&src, &tgt, 0);
        let (pos1, _) = edge_label_world_pos(&src, &tgt, 1);
        assert!((pos0.x - 100.0).abs() < 1.0, "label x near midpoint");
        assert!((pos1.x - 100.0).abs() < 1.0, "label x near midpoint");
        assert!(pos0.y != pos1.y, "idx 0 and 1 offset in opposite directions");
    }

    #[test]
    fn edge_label_world_pos_above_curve() {
        let src = Transform::from_xyz(0.0, 0.0, 0.0);
        let tgt = Transform::from_xyz(100.0, 100.0, 0.0);
        let (pos, angle) = edge_label_world_pos(&src, &tgt, 0);
        let mid = Vec2::new(50.0, 50.0);
        let dist = pos.distance(mid);
        assert!(dist > 10.0, "label offset from midpoint");
        assert!(angle.abs() < std::f32::consts::PI + 0.1, "angle in valid range");
    }
}

/// Spawn/update Text2d labels for edges at midpoint, offset above the curve.
/// Always creates a label for every edge (even empty) so there is a clickable area.
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
        let Ok(src) = node_transform_query.get(edge.source) else {
            continue;
        };
        let Ok(tgt) = node_transform_query.get(edge.target) else {
            continue;
        };
        let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
        let (label_pos, angle) = edge_label_world_pos(src, tgt, idx);
        let label_text = edge.label.as_deref().unwrap_or("");

        let label_entity = children_query
            .get(edge_entity)
            .ok()
            .and_then(|c| c.iter().find(|id| label_query.get(*id).is_ok()));

        if let Some(label_entity) = label_entity {
            if let Ok((mut transform, mut text2d)) = label_query.get_mut(label_entity) {
                transform.translation = label_pos.extend(1.0);
                transform.rotation = Quat::from_rotation_z(angle);
                text2d.clear();
                text2d.push_str(label_text);
            }
        } else {
            commands.entity(edge_entity).insert(Visibility::default());
            let label_entity = commands
                .spawn((
                    Text2d::new(label_text),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.5, 0.55, 0.65)),
                    Transform::from_xyz(label_pos.x, label_pos.y, 1.0)
                        .with_rotation(Quat::from_rotation_z(angle)),
                    EdgeLabel,
                ))
                .id();
            commands.entity(edge_entity).add_child(label_entity);
        }
    }
}

/// Draw a mode-coloured rectangle outline around the selected node, or a highlight at the selected edge label.
///
/// VimNormal → blue   VimInsert → green   VimEasymotion → orange
/// Standard  → purple
pub fn draw_selection_system(
    mut gizmos: Gizmos,
    selected_query: Query<&Transform, With<Selected>>,
    selected_edge: Res<crate::resources::SelectedEdge>,
    edge_query: Query<(Entity, &Edge)>,
    node_transform_query: Query<&Transform, Without<EdgeLabel>>,
    state: Res<State<InputMode>>,
) {
    let color = match state.get() {
        InputMode::VimInsert => Color::srgb(0.2, 0.85, 0.4),
        InputMode::VimEasymotion => Color::srgb(1.0, 0.6, 0.1),
        InputMode::Standard => Color::srgb(0.85, 0.4, 0.9),
        InputMode::VimNormal | InputMode::VimCommand => Color::srgb(0.3, 0.6, 1.0),
    };

    if let Some(edge_entity) = selected_edge.0 {
        if let Ok((_, edge)) = edge_query.get(edge_entity) {
            if let (Ok(src), Ok(tgt)) = (
                node_transform_query.get(edge.source),
                node_transform_query.get(edge.target),
            ) {
                let mut groups: std::collections::HashMap<(Entity, Entity), Vec<Entity>> =
                    std::collections::HashMap::new();
                for (e, ed) in &edge_query {
                    groups.entry((ed.source, ed.target)).or_default().push(e);
                }
                let mut idx_map: std::collections::HashMap<Entity, usize> =
                    std::collections::HashMap::new();
                for (_, entities) in &groups {
                    for (i, e) in entities.iter().enumerate() {
                        idx_map.insert(*e, i);
                    }
                }
                let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
                let (label_pos, _) = edge_label_world_pos(src, tgt, idx);
                gizmos.rect_2d(
                    Isometry2d::from_translation(label_pos),
                    LABEL_HIT_HALF * 2.0,
                    color,
                );
                return;
            }
        }
    }

    let Ok(transform) = selected_query.single() else {
        return;
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
