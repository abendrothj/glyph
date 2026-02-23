use bevy::{
    input::keyboard::Key,
    prelude::*,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum InputMode {
    Standard,
    #[default]
    VimNormal,
    VimInsert,
    VimEasymotion,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct CanvasNode;

#[derive(Component)]
struct TextData {
    content: String,
}

/// Marker — exactly one entity carries this at a time.
#[derive(Component)]
struct Selected;

/// Marker on the temporary floating tag labels during easymotion.
#[derive(Component)]
struct JumpTag;

/// Marker on the Text2d child entity that lives inside every CanvasNode.
#[derive(Component)]
struct TextLabel;

/// Directed edge between two CanvasNode entities.
#[derive(Component)]
struct Edge {
    source: Entity,
    target: Entity,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Maps single-character jump tags ('a', 'b', …) to their CanvasNode entity.
/// Populated on VimEasymotion entry; cleared on exit.
#[derive(Resource, Default)]
struct JumpMap {
    char_to_entity: HashMap<char, Entity>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<InputMode>()
        .init_resource::<JumpMap>()
        .add_systems(Startup, setup_canvas)
        .add_systems(OnEnter(InputMode::VimEasymotion), jump_tag_setup)
        .add_systems(OnExit(InputMode::VimEasymotion), jump_tag_cleanup)
        .add_systems(
            Update,
            (
                // Input
                vim_normal_system.run_if(in_state(InputMode::VimNormal)),
                vim_insert_system.run_if(in_state(InputMode::VimInsert)),
                standard_mode_system.run_if(in_state(InputMode::Standard)),
                vim_easymotion_system.run_if(in_state(InputMode::VimEasymotion)),
                // Rendering (runs every frame regardless of mode)
                draw_edges_system,
                draw_selection_system,
                sync_text_system,
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn setup_canvas(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Node A — initially selected
    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.92, 0.92, 0.90), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(-300.0, 100.0, 0.0),
            CanvasNode,
            TextData { content: "Node A".to_string() },
            Selected,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node A"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                // Relative to parent: centered, one layer in front of the sprite.
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });

    // Node B
    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.70, 0.85, 0.95), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(0.0, 0.0, 0.0),
            CanvasNode,
            TextData { content: "Node B".to_string() },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node B"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });

    // Node C
    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.95, 0.80, 0.70), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(300.0, -100.0, 0.0),
            CanvasNode,
            TextData { content: "Node C".to_string() },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node C"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });
}

// ---------------------------------------------------------------------------
// Easymotion systems
// ---------------------------------------------------------------------------

/// OnEnter(VimEasymotion): assign a letter tag to every CanvasNode and
/// spawn a floating Text2d label above each one.
fn jump_tag_setup(
    mut commands: Commands,
    mut jump_map: ResMut<JumpMap>,
    nodes: Query<(Entity, &Transform), With<CanvasNode>>,
) {
    for ((entity, transform), tag_char) in nodes.iter().zip("abcdefghijklmnopqrstuvwxyz".chars()) {
        jump_map.char_to_entity.insert(tag_char, entity);

        let label_pos = transform.translation + Vec3::new(0.0, 50.0, 1.0);
        commands.spawn((
            Text2d::new(tag_char.to_uppercase().to_string()),
            TextFont { font_size: 28.0, ..default() },
            TextColor(Color::srgb(1.0, 0.85, 0.1)),
            Transform::from_translation(label_pos),
            JumpTag,
        ));
    }

    info!(
        "[EASYMOTION] Tags: {:?}",
        {
            let mut keys: Vec<char> = jump_map.char_to_entity.keys().copied().collect();
            keys.sort_unstable();
            keys
        }
    );
}

/// in_state(VimEasymotion): one keypress teleports Selected to the tagged node.
fn vim_easymotion_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    jump_map: Res<JumpMap>,
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
) {
    for key in keys.get_just_pressed() {
        let Some(tag_char) = keycode_to_char(key) else {
            continue;
        };
        let Some(&target) = jump_map.char_to_entity.get(&tag_char) else {
            continue;
        };

        if let Ok(prev) = selected_query.single() {
            commands.entity(prev).remove::<Selected>();
        }
        commands.entity(target).insert(Selected);

        next_state.set(InputMode::VimNormal);
        info!("[EASYMOTION] Jumped to {:?} via '{}'", target, tag_char);
        return;
    }
}

/// OnExit(VimEasymotion): despawn all JumpTag labels and clear the map.
fn jump_tag_cleanup(
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

// ---------------------------------------------------------------------------
// Rendering systems
// ---------------------------------------------------------------------------

/// Draw a line between every pair of source/target entities that have an Edge.
fn draw_edges_system(
    mut gizmos: Gizmos,
    edge_query: Query<&Edge>,
    transform_query: Query<&Transform>,
) {
    for edge in &edge_query {
        let Ok(src) = transform_query.get(edge.source) else {
            continue;
        };
        let Ok(tgt) = transform_query.get(edge.target) else {
            continue;
        };
        gizmos.line_2d(
            src.translation.truncate(),
            tgt.translation.truncate(),
            Color::srgb(0.35, 0.35, 0.55),
        );
    }
}

/// Draw a mode-coloured rectangle outline around the selected node.
///
/// VimNormal  → blue   VimInsert → green   VimEasymotion → orange
fn draw_selection_system(
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
        _ => Color::srgb(0.3, 0.6, 1.0),
    };

    // Outline sits 5px outside the 160×80 sprite on every side.
    gizmos.rect_2d(
        Isometry2d::from_translation(transform.translation.truncate()),
        Vec2::new(170.0, 90.0),
        color,
    );
}

/// When TextData.content changes, push the new string into the child Text2d.
fn sync_text_system(
    changed_nodes: Query<(&TextData, &Children), Changed<TextData>>,
    mut text_query: Query<&mut Text2d, With<TextLabel>>,
) {
    for (text_data, children) in &changed_nodes {
        for child in children {
            if let Ok(mut text2d) = text_query.get_mut(*child) {
                // Text2d derefs to String, so we can clear + push directly.
                text2d.clear();
                text2d.push_str(&text_data.content);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Input systems
// ---------------------------------------------------------------------------

fn vim_normal_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform), With<Selected>>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // --- Mode switches (priority over movement) ----------------------------

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

    // Shift+A: spawn a new connected node to the right, select it, begin typing.
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

    // --- HJKL spatial movement --------------------------------------------

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

/// VimInsert: capture typed text via ButtonInput<Key> (the logical-key resource).
///
/// Key::Character(SmolStr) gives layout-aware character input — 'A' vs 'a',
/// accents, etc. — without requiring EventReader or observers. Named keys
/// handle Escape and Backspace directly.
fn vim_insert_system(
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

    // Key::Character captures every printable key, already layout- and
    // shift-aware (pressing Shift+A gives Key::Character("A") not "a").
    for key in keys.get_just_pressed() {
        if let Key::Character(c) = key {
            if let Ok(mut text_data) = query.single_mut() {
                text_data.content.push_str(c.as_str());
                info!("[INSERT] \"{}\" → \"{}\"", c, text_data.content);
            }
        }
    }
}

fn standard_mode_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn keycode_to_char(key: &KeyCode) -> Option<char> {
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
