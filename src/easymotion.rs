//! Vim easymotion jump-tag systems.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::viewport_world_bounds;
use crate::components::{CanvasNode, Edge, JumpTag, MainCamera, Selected};
use crate::helpers::keycode_to_char;
use crate::input::EasymotionConnectSource;
use crate::resources::{JumpMap, SpatialIndex};
use crate::state::InputMode;

/// OnEnter(VimEasymotion): assign letter tags only to CanvasNodes visible in the
/// current viewport, using the spatial index for O(1) culling.
pub fn jump_tag_setup(
    mut commands: Commands,
    mut jump_map: ResMut<JumpMap>,
    spatial_index: Res<SpatialIndex>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    transform_query: Query<&Transform, With<CanvasNode>>,
) {
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Ok(window) = window_q.single() else {
        return;
    };
    let viewport_size = window.resolution.physical_size().as_vec2();

    let (min_x, max_x, min_y, max_y) = viewport_world_bounds(camera, cam_transform, viewport_size);
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
        "[EASYMOTION] Tags: {:?} (viewport-culled)",
        {
            let mut keys: Vec<char> = jump_map.char_to_entity.keys().copied().collect();
            keys.sort_unstable();
            keys
        }
    );
}

/// in_state(VimEasymotion): one keypress teleports Selected to the tagged node. Esc/Ctrl+[ cancels.
/// When EasymotionConnectSource is set (ce), creates edge from source to target instead.
pub fn vim_easymotion_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut connect_source: ResMut<EasymotionConnectSource>,
    jump_map: Res<JumpMap>,
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::Escape) || (ctrl && keys.just_pressed(KeyCode::BracketLeft)) {
        connect_source.0 = None;
        next_state.set(InputMode::VimNormal);
        info!("[EASYMOTION] cancelled");
        return;
    }
    for key in keys.get_just_pressed() {
        let Some(tag_char) = keycode_to_char(key) else {
            continue;
        };
        let Some(&target) = jump_map.char_to_entity.get(&tag_char) else {
            continue;
        };

        if let Some(source) = connect_source.0.take() {
            if source != target {
                commands.spawn(Edge {
                    source,
                    target,
                    label: None,
                });
                info!("[EASYMOTION] Connected {:?} → {:?}", source, target);
            }
            if let Ok(prev) = selected_query.single() {
                commands.entity(prev).remove::<Selected>();
            }
            commands.entity(target).insert(Selected);
        } else {
            if let Ok(prev) = selected_query.single() {
                commands.entity(prev).remove::<Selected>();
            }
            commands.entity(target).insert(Selected);
            info!("[EASYMOTION] Jumped to {:?} via '{}'", target, tag_char);
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
