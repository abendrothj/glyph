//! Utility functions.

use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextBounds};

use crate::components::{CanvasNode, Edge, NodeColor, NodeMainSprite, Selected, TextData, TextLabel};

/// Node size and shadow offset.
pub const NODE_SIZE: Vec2 = Vec2::new(160.0, 120.0);
const SHADOW_OFFSET: Vec2 = Vec2::new(-4.0, -4.0);
const SHADOW_SIZE: Vec2 = Vec2::new(168.0, 128.0);

/// Text area inside node (padding from edges). Enables multi-line wrapping.
const TEXT_BOUNDS: Vec2 = Vec2::new(150.0, 110.0);

/// Dark slate node color.
pub const NODE_COLOR: Color = Color::srgb(0.38, 0.44, 0.52);
/// Drop shadow color.
const SHADOW_COLOR: Color = Color::srgb(0.12, 0.14, 0.18);

/// Spawn a new canvas node at the given position with shadow and centered text.
pub fn spawn_canvas_node(
    commands: &mut Commands,
    position: Vec2,
    text: impl Into<String>,
    selected: bool,
) -> Entity {
    let content: String = text.into();
    let mut entity_cmd = commands.spawn((
        Transform::from_xyz(position.x, position.y, 0.0),
        Visibility::default(),
        CanvasNode,
        TextData { content: content.clone() },
        NodeColor(NODE_COLOR),
    ));
    if selected {
        entity_cmd.insert(Selected);
    }
    entity_cmd
        .with_children(|parent| {
            parent.spawn((
                Sprite::from_color(SHADOW_COLOR, SHADOW_SIZE),
                Transform::from_xyz(SHADOW_OFFSET.x, SHADOW_OFFSET.y, -0.1),
            ));
            parent.spawn((
                Sprite::from_color(NODE_COLOR, NODE_SIZE),
                Transform::from_xyz(0.0, 0.0, 0.0),
                NodeMainSprite,
            ));
            parent.spawn((
                Text2d::new(content),
                TextFont { font_size: 15.0, ..default() },
                TextColor(Color::srgb(0.95, 0.96, 0.98)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                TextBounds::from(TEXT_BOUNDS),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        })
        .id()
}

/// Spawn a node with a specific color (used by load_from_path).
pub fn spawn_node_with_color(
    commands: &mut Commands,
    x: f32,
    y: f32,
    text: &str,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Transform::from_xyz(x, y, 0.0),
            Visibility::default(),
            CanvasNode,
            TextData { content: text.to_string() },
            NodeColor(color),
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite::from_color(SHADOW_COLOR, SHADOW_SIZE),
                Transform::from_xyz(SHADOW_OFFSET.x, SHADOW_OFFSET.y, -0.1),
            ));
            parent.spawn((
                Sprite::from_color(color, NODE_SIZE),
                Transform::from_xyz(0.0, 0.0, 0.0),
                NodeMainSprite,
            ));
            parent.spawn((
                Text2d::new(text.to_string()),
                TextFont { font_size: 15.0, ..default() },
                TextColor(Color::srgb(0.95, 0.96, 0.98)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                TextBounds::from(TEXT_BOUNDS),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        })
        .id()
}

/// Delete a node and all edges connected to it.
pub fn delete_node(
    commands: &mut Commands,
    node_entity: Entity,
    edge_query: &Query<(Entity, &Edge)>,
) {
    for (edge_entity, edge) in edge_query.iter() {
        if edge.source == node_entity || edge.target == node_entity {
            commands.entity(edge_entity).despawn();
        }
    }
    commands.entity(node_entity).despawn();
}

pub fn keycode_to_char(key: &KeyCode) -> Option<char> {
    match key {
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        _ => None,
    }
}
