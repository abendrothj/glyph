//! Vim easymotion jump-tag systems.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::viewport_world_bounds;
use crate::components::{CanvasNode, Edge, JumpTag, MainCamera, Selected};
use crate::helpers::keycode_to_char;
use crate::input::EasymotionConnectSource;
use crate::rendering::edge_label_world_pos;
use crate::resources::{JumpMap, SelectedEdge, SpatialIndex};
use crate::state::InputMode;

/// What easymotion is targeting: nodes (f, ce) or edges for label edit (ge).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum EasymotionTarget {
    #[default]
    Node,
    EdgeLabel,
}

const TAG_CHARS: &str = "abcdefghijklmnopqrstuvwxyz";

/// Sort order for jump tags: top-to-bottom, then left-to-right.
/// This makes tag assignment spatially predictable — the top-left node is always
/// 'a', the next one right is 'b', etc. — so users can build spatial muscle memory.
fn sort_by_position(a: &Vec2, b: &Vec2) -> std::cmp::Ordering {
    b.y.partial_cmp(&a.y)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
}

/// OnEnter(VimEasymotion): assign letter tags to visible nodes or edges based on EasymotionTarget.
pub fn jump_tag_setup(
    mut commands: Commands,
    mut jump_map: ResMut<JumpMap>,
    target: Res<EasymotionTarget>,
    spatial_index: Res<SpatialIndex>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    transform_query: Query<&Transform, With<CanvasNode>>,
    edge_query: Query<(Entity, &Edge)>,
    node_transform_query: Query<&Transform, With<CanvasNode>>,
) {
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Ok(window) = window_q.single() else {
        return;
    };
    let viewport_size = window.resolution.physical_size().as_vec2();
    let (min_x, max_x, min_y, max_y) = viewport_world_bounds(camera, cam_transform, viewport_size);

    if *target == EasymotionTarget::EdgeLabel {
        // Build per-(source, target) index for multi-edge offset calculation.
        let mut groups: std::collections::HashMap<(Entity, Entity), Vec<Entity>> =
            std::collections::HashMap::new();
        for (entity, edge) in &edge_query {
            groups
                .entry((edge.source, edge.target))
                .or_default()
                .push(entity);
        }
        let mut idx_map: std::collections::HashMap<Entity, usize> =
            std::collections::HashMap::new();
        for (_, entities) in &groups {
            for (i, e) in entities.iter().enumerate() {
                idx_map.insert(*e, i);
            }
        }

        // Collect in-viewport edges with their label world positions.
        let mut visible: Vec<(Entity, Vec2)> = Vec::new();
        for (edge_entity, edge) in &edge_query {
            let Ok(src) = node_transform_query.get(edge.source) else {
                continue;
            };
            let Ok(tgt) = node_transform_query.get(edge.target) else {
                continue;
            };
            let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
            let (label_pos, _) = edge_label_world_pos(src, tgt, idx);
            if label_pos.x >= min_x
                && label_pos.x <= max_x
                && label_pos.y >= min_y
                && label_pos.y <= max_y
            {
                visible.push((edge_entity, label_pos));
            }
        }

        // Sort for consistent, spatially predictable tag assignment.
        visible.sort_by(|(_, a), (_, b)| sort_by_position(a, b));

        if visible.len() > TAG_CHARS.len() {
            warn!(
                "[EASYMOTION] {} visible edges but only {} tags available — zoom in to reach all",
                visible.len(),
                TAG_CHARS.len()
            );
        }

        for ((edge_entity, label_pos), tag_char) in visible.iter().zip(TAG_CHARS.chars()) {
            jump_map.char_to_entity.insert(tag_char, *edge_entity);
            commands.spawn((
                Text2d::new(tag_char.to_uppercase().to_string()),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.1)),
                Transform::from_translation(label_pos.extend(2.0)),
                JumpTag,
            ));
        }
        info!(
            "[EASYMOTION] Edge tags assigned: {} of {} visible",
            visible.len().min(TAG_CHARS.len()),
            visible.len()
        );
    } else {
        // Node mode: collect visible nodes with their world positions, then sort.
        // Positions are resolved before the zip so no tag chars are wasted on
        // entities whose transforms happen to be missing.
        let mut visible: Vec<(Entity, Vec2)> = spatial_index
            .entities_in_bounds(min_x, max_x, min_y, max_y)
            .into_iter()
            .filter_map(|entity| {
                let Ok(transform) = transform_query.get(entity) else {
                    return None;
                };
                Some((entity, transform.translation.truncate()))
            })
            .collect();

        // Consistent top-to-bottom, left-to-right ordering — 'a' is always the
        // top-left visible node regardless of insertion or HashMap iteration order.
        visible.sort_by(|(_, a), (_, b)| sort_by_position(a, b));

        if visible.len() > TAG_CHARS.len() {
            warn!(
                "[EASYMOTION] {} visible nodes but only {} tags available — zoom in to reach all",
                visible.len(),
                TAG_CHARS.len()
            );
        }

        for ((entity, pos), tag_char) in visible.iter().zip(TAG_CHARS.chars()) {
            jump_map.char_to_entity.insert(tag_char, *entity);
            // Place tag above the node (node half-height = 60, tag at +70) so it
            // never overlaps the node's own text. z=2 renders above box and text.
            let label_pos = Vec3::new(pos.x, pos.y + 70.0, 2.0);
            commands.spawn((
                Text2d::new(tag_char.to_uppercase().to_string()),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.1)),
                Transform::from_translation(label_pos),
                JumpTag,
            ));
        }

        info!(
            "[EASYMOTION] Node tags assigned: {} of {} visible",
            visible.len().min(TAG_CHARS.len()),
            visible.len()
        );
    }
}

/// in_state(VimEasymotion): one keypress selects the tagged target. Esc/Ctrl+[ cancels.
/// Node mode: teleports Selected to node (or creates edge if ce). EdgeLabel mode: sets SelectedEdge, enters VimInsert.
pub fn vim_easymotion_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut connect_source: ResMut<EasymotionConnectSource>,
    mut selected_edge: ResMut<SelectedEdge>,
    target: Res<EasymotionTarget>,
    jump_map: Res<JumpMap>,
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::Escape) || (ctrl && keys.just_pressed(KeyCode::BracketLeft)) {
        connect_source.0 = None;
        selected_edge.0 = None;
        next_state.set(InputMode::VimNormal);
        info!("[EASYMOTION] cancelled");
        return;
    }
    for key in keys.get_just_pressed() {
        let Some(tag_char) = keycode_to_char(key) else {
            continue;
        };
        let Some(&target_entity) = jump_map.char_to_entity.get(&tag_char) else {
            continue;
        };

        if *target == EasymotionTarget::EdgeLabel {
            selected_edge.0 = Some(target_entity);
            for prev in &selected_query {
                commands.entity(prev).remove::<Selected>();
            }
            next_state.set(InputMode::VimInsert);
            info!("[EASYMOTION] Edge label {:?} via '{}' → VimInsert", target_entity, tag_char);
            return;
        }

        // Node mode
        if let Some(source) = connect_source.0.take() {
            if source != target_entity {
                commands.spawn(Edge {
                    source,
                    target: target_entity,
                    label: None,
                });
                info!("[EASYMOTION] Connected {:?} → {:?}", source, target_entity);
            }
            if let Ok(prev) = selected_query.single() {
                commands.entity(prev).remove::<Selected>();
            }
            commands.entity(target_entity).insert(Selected);
        } else {
            if let Ok(prev) = selected_query.single() {
                commands.entity(prev).remove::<Selected>();
            }
            commands.entity(target_entity).insert(Selected);
            info!("[EASYMOTION] Jumped to {:?} via '{}'", target_entity, tag_char);
        }
        next_state.set(InputMode::VimNormal);
        return;
    }
}

/// OnExit(VimEasymotion): despawn all JumpTag labels and clear the map.
pub fn jump_tag_cleanup(
    mut commands: Commands,
    mut jump_map: ResMut<JumpMap>,
    tag_query: Query<Entity, With<JumpTag>>,
) {
    for entity in &tag_query {
        commands.entity(entity).despawn();
    }
    jump_map.char_to_entity.clear();
    info!("[EASYMOTION] Tags cleaned up");
}
