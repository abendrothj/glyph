use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

/// Marker component for whiteboard nodes.
#[derive(Component)]
struct CanvasNode;

fn setup(mut commands: Commands) {
    // 2D orthographic camera
    commands.spawn(Camera2d);

    // Single rectangular node in the center
    commands.spawn((
        Sprite::from_color(Color::srgb(0.92, 0.92, 0.90), Vec2::new(200.0, 120.0)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CanvasNode,
    ));
}
