//! Camera controls and viewport utilities.

use bevy::prelude::*;

use crate::components::MainCamera;

/// Scroll-wheel zoom: adjusts the orthographic scale of the main camera.
/// Pinch/scroll in  → scale decreases (zoom in, things appear larger).
/// Pinch/scroll out → scale increases (zoom out, things appear smaller).
pub fn camera_zoom_system(
    mut mouse_wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut proj_q: Query<&mut Projection, With<MainCamera>>,
) {
    let Ok(mut proj) = proj_q.single_mut() else {
        return;
    };
    for event in mouse_wheel.read() {
        let Projection::Orthographic(ortho) = proj.as_mut() else {
            continue;
        };
        let delta = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.10,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
        };
        ortho.scale = (ortho.scale * (1.0 - delta)).clamp(0.1, 10.0);
    }
}

/// Pan: middle-click drag or Space+left-drag. Translate the camera opposite to mouse movement.
/// Pan speed is proportional to zoom scale so one pixel of mouse movement = one pixel viewport.
pub fn camera_pan_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut camera_q: Query<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let space = keys.pressed(KeyCode::Space);
    let panning = mouse_buttons.pressed(MouseButton::Middle)
        || (space && mouse_buttons.pressed(MouseButton::Left));

    if !panning {
        for _ in mouse_motion.read() {}
        return;
    }

    let Ok((mut cam_transform, projection)) = camera_q.single_mut() else {
        return;
    };
    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };

    for motion in mouse_motion.read() {
        cam_transform.translation.x -= motion.delta.x * scale;
        cam_transform.translation.y += motion.delta.y * scale;
    }
}

/// Arrow keys pan the camera. Hold for continuous movement (scale-aware).
const PAN_SPEED: f32 = 400.0; // pixels per second at scale 1.0

pub fn camera_pan_keys_system(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera_q: Query<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    if keys.pressed(KeyCode::ArrowLeft) {
        dx += PAN_SPEED;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dx -= PAN_SPEED;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        dy -= PAN_SPEED;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        dy += PAN_SPEED;
    }
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    let Ok((mut cam_transform, projection)) = camera_q.single_mut() else {
        return;
    };
    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };
    let delta = time.delta_secs() * scale;
    cam_transform.translation.x += dx * delta;
    cam_transform.translation.y += dy * delta;
}

/// Computes the world-space AABB of the visible viewport from the main camera.
pub fn viewport_world_bounds(
    camera: &Camera,
    cam_transform: &GlobalTransform,
    viewport_size: Vec2,
) -> (f32, f32, f32, f32) {
    let corners = [
        Vec2::ZERO,
        Vec2::new(viewport_size.x, 0.0),
        Vec2::new(viewport_size.x, viewport_size.y),
        Vec2::new(0.0, viewport_size.y),
    ];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for corner in corners {
        if let Ok(world) = camera.viewport_to_world_2d(cam_transform, corner) {
            min_x = min_x.min(world.x);
            max_x = max_x.max(world.x);
            min_y = min_y.min(world.y);
            max_y = max_y.max(world.y);
        }
    }

    (min_x, max_x, min_y, max_y)
}
