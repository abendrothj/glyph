use bevy::prelude::*;

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum InputMode {
    Standard,
    #[default]
    VimNormal,
    VimInsert,
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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<InputMode>()
        .add_systems(Startup, setup_canvas)
        .add_systems(
            Update,
            (
                vim_normal_system.run_if(in_state(InputMode::VimNormal)),
                vim_insert_system.run_if(in_state(InputMode::VimInsert)),
                standard_mode_system.run_if(in_state(InputMode::Standard)),
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn setup_canvas(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Node A — spawned with Selected marker
    commands.spawn((
        Sprite::from_color(Color::srgb(0.92, 0.92, 0.90), Vec2::new(160.0, 80.0)),
        Transform::from_xyz(-300.0, 100.0, 0.0),
        CanvasNode,
        TextData { content: "Node A".to_string() },
        Selected,
    ));

    // Node B
    commands.spawn((
        Sprite::from_color(Color::srgb(0.70, 0.85, 0.95), Vec2::new(160.0, 80.0)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CanvasNode,
        TextData { content: "Node B".to_string() },
    ));

    // Node C
    commands.spawn((
        Sprite::from_color(Color::srgb(0.95, 0.80, 0.70), Vec2::new(160.0, 80.0)),
        Transform::from_xyz(300.0, -100.0, 0.0),
        CanvasNode,
        TextData { content: "Node C".to_string() },
    ));
}

// ---------------------------------------------------------------------------
// Input systems
// ---------------------------------------------------------------------------

fn vim_normal_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut query: Query<&mut Transform, With<Selected>>,
) {
    // Mode switch has priority — check it first.
    if keys.just_pressed(KeyCode::KeyI) {
        next_state.set(InputMode::VimInsert);
        info!("→ VimInsert");
        return;
    }

    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    // HJKL movement: use `pressed` so holding a key repeats.
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

fn vim_insert_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<InputMode>>,
    query: Query<&TextData, With<Selected>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
        return;
    }

    // Placeholder: echo every new keystroke alongside the node's content.
    for key in keys.get_just_pressed() {
        if let Ok(text_data) = query.single() {
            info!("[INSERT] {:?} → \"{}\"", key, text_data.content);
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
