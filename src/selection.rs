//! Mouse selection and node drag/drop systems.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::{CanvasNode, Dragging, MainCamera, Selected};
use crate::state::InputMode;

/// Left-click to select a CanvasNode and begin dragging it.
///
/// Skipped entirely in VimInsert so that typing is never interrupted by
/// an accidental click.  In any other mode a click on a node:
///   1. Clears the previous selection.
///   2. Inserts `Selected` and `Dragging { offset }` on the clicked entity.
///   3. Transitions to `Standard` mode.
/// A click on empty canvas deselects.
pub fn mouse_selection_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut commands: Commands,
    node_query: Query<(Entity, &Transform), With<CanvasNode>>,
    selected_q: Query<Entity, With<Selected>>,
    dragging_q: Query<Entity, With<Dragging>>,
    mut next_state: ResMut<NextState<InputMode>>,
    current_state: Res<State<InputMode>>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if *current_state.get() == InputMode::VimInsert {
        return;
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor_pos) else {
        return;
    };

    const HALF: Vec2 = Vec2::new(80.0, 40.0);

    for (entity, transform) in &node_query {
        let node_pos = transform.translation.truncate();
        if world_pos.x >= node_pos.x - HALF.x
            && world_pos.x <= node_pos.x + HALF.x
            && world_pos.y >= node_pos.y - HALF.y
            && world_pos.y <= node_pos.y + HALF.y
        {
            for prev in &selected_q {
                commands.entity(prev).remove::<Selected>();
            }
            for prev in &dragging_q {
                commands.entity(prev).remove::<Dragging>();
            }

            let offset = world_pos - node_pos;
            commands
                .entity(entity)
                .insert((Selected, Dragging { offset }));

            next_state.set(InputMode::Standard);
            info!("[SELECT] {:?} @ {:?}", entity, node_pos);
            return;
        }
    }

    for prev in &selected_q {
        commands.entity(prev).remove::<Selected>();
    }
}

/// While the left mouse button is held, move the dragged node to the cursor.
pub fn node_drag_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut dragging_q: Query<(&mut Transform, &Dragging)>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor_pos) else {
        return;
    };

    for (mut transform, dragging) in &mut dragging_q {
        let target = world_pos - dragging.offset;
        transform.translation.x = target.x;
        transform.translation.y = target.y;
    }
}

/// When the left mouse button is released, remove the Dragging marker.
pub fn node_drop_system(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    dragging_q: Query<Entity, With<Dragging>>,
) {
    if mouse_buttons.just_released(MouseButton::Left) {
        for entity in &dragging_q {
            commands.entity(entity).remove::<Dragging>();
            info!("[DROP] {:?}", entity);
        }
    }
}
