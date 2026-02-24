//! Vim input mode systems (normal, insert, standard).

use bevy::input::keyboard::Key;
use bevy::prelude::*;

use crate::components::{CanvasNode, Edge, Selected, TextData, TextLabel};
use crate::state::InputMode;

/// VimNormal: HJKL movement, i/f/Shift+A mode switches.
pub fn vim_normal_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform), With<Selected>>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    if keys.just_pressed(KeyCode::KeyI) {
        next_state.set(InputMode::VimInsert);
        info!("→ VimInsert");
        return;
    }

    if keys.just_pressed(KeyCode::KeyF) {
        next_state.set(InputMode::VimEasymotion);
        info!("→ VimEasymotion");
        return;
    }

    if shift && keys.just_pressed(KeyCode::KeyA) {
        if let Ok((source_entity, source_transform)) = query.single_mut() {
            let new_pos = source_transform.translation + Vec3::new(200.0, 0.0, 0.0);

            commands.entity(source_entity).remove::<Selected>();

            let new_node = commands
                .spawn((
                    Sprite::from_color(Color::srgb(0.80, 0.95, 0.80), Vec2::new(160.0, 80.0)),
                    Transform::from_translation(new_pos),
                    CanvasNode,
                    TextData { content: String::new() },
                    Selected,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(""),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.1, 0.1, 0.1)),
                        Transform::from_xyz(0.0, 0.0, 1.0),
                        TextLabel,
                    ));
                })
                .id();

            commands.spawn(Edge {
                source: source_entity,
                target: new_node,
            });
            info!("[GRAPH] Edge {:?} → {:?}", source_entity, new_node);
        }
        next_state.set(InputMode::VimInsert);
        info!("→ VimInsert (new node)");
        return;
    }

    let Ok((_, mut transform)) = query.single_mut() else {
        return;
    };

    if keys.pressed(KeyCode::KeyH) {
        transform.translation.x -= 10.0;
    }
    if keys.pressed(KeyCode::KeyL) {
        transform.translation.x += 10.0;
    }
    if keys.pressed(KeyCode::KeyK) {
        transform.translation.y += 10.0;
    }
    if keys.pressed(KeyCode::KeyJ) {
        transform.translation.y -= 10.0;
    }
}

/// VimInsert: capture typed text via ButtonInput<Key>.
pub fn vim_insert_system(
    keys: Res<ButtonInput<Key>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut query: Query<&mut TextData, With<Selected>>,
) {
    if keys.just_pressed(Key::Escape) {
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
        return;
    }

    if keys.just_pressed(Key::Backspace) {
        if let Ok(mut text_data) = query.single_mut() {
            text_data.content.pop();
            info!("[INSERT] ⌫ → \"{}\"", text_data.content);
        }
        return;
    }

    for key in keys.get_just_pressed() {
        if let Key::Character(c) = key {
            if let Ok(mut text_data) = query.single_mut() {
                text_data.content.push_str(c.as_str());
                info!("[INSERT] \"{}\" → \"{}\"", c, text_data.content);
            }
        }
    }
}

/// Standard mode: Escape returns to VimNormal.
pub fn standard_mode_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
    }
}
