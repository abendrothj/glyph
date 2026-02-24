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
        // Assign letters to edges whose label position is in viewport
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

        let mut visible_edges: Vec<(Entity, Vec2)> = Vec::new();
        for (edge_entity, edge) in &edge_query {
            let Ok(src) = node_transform_query.get(edge.source) else {
                continue;
            };
            let Ok(tgt) = node_transform_query.get(edge.target) else {
                continue;
            };
            let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
            let (label_pos, _) = edge_label_world_pos(src, tgt, idx);
            if label_pos.x >= min_x && label_pos.x <= max_x
                && label_pos.y >= min_y && label_pos.y <= max_y
            {
                visible_edges.push((edge_entity, label_pos));
            }
        }

        for ((edge_entity, label_pos), tag_char) in visible_edges
            .into_iter()
            .zip("abcdefghijklmnopqrstuvwxyz".chars())
        {
            jump_map.char_to_entity.insert(tag_char, edge_entity);
            let pos = label_pos.extend(1.0);
            commands.spawn((
                Text2d::new(tag_char.to_uppercase().to_string()),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.1)),
                Transform::from_translation(pos),
                JumpTag,
            ));
        }
        info!(
            "[EASYMOTION] Edge tags: {:?}",
            jump_map.char_to_entity.keys().copied().collect::<Vec<_>>()
        );
    } else {
        // Node mode: spatial index for viewport-culled nodes
        let visible_entities = spatial_index.entities_in_bounds(min_x, max_x, min_y, max_y);

        for (entity, tag_char) in visible_entities
            .into_iter()
            .zip("abcdefghijklmnopqrstuvwxyz".chars())
        {
            let Ok(transform) = transform_query.get(entity) else {
                continue;
            };

            jump_map.char_to_entity.insert(tag_char, entity);

            let label_pos = transform.translation + Vec3::new(0.0, -65.0, 1.0);
            commands.spawn((
                Text2d::new(tag_char.to_uppercase().to_string()),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.1)),
                Transform::from_translation(label_pos),
                JumpTag,
            ));
        }

        info!(
            "[EASYMOTION] Node tags: {:?} (viewport-culled)",
            {
                let mut keys: Vec<char> = jump_map.char_to_entity.keys().copied().collect();
                keys.sort_unstable();
                keys
            }
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
