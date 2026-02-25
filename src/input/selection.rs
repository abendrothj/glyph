//! Mouse selection and node drag/drop systems.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::components::{CanvasNode, Dragging, Edge, MainCamera, Selected};
use crate::core::helpers::spawn_canvas_node;
use crate::render::edges::{edge_label_world_pos, LABEL_HIT_HALF};
use crate::core::resources::SelectedEdge;
use crate::core::state::InputMode;

/// Tracks the source node when drawing an edge (Shift+drag from node).
#[derive(Resource, Default)]
pub struct DrawingEdge(pub Option<Entity>);

const NODE_HALF: Vec2 = Vec2::new(80.0, 60.0);

fn cursor_world_pos(
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let window = window_q.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_transform) = camera_q.single().ok()?;
    camera.viewport_to_world_2d(cam_transform, cursor).ok()
}

fn node_at_pos(
    node_query: &Query<(Entity, &Transform), With<CanvasNode>>,
    pos: Vec2,
) -> Option<Entity> {
    for (entity, transform) in node_query.iter() {
        let node_pos = transform.translation.truncate();
        if pos.x >= node_pos.x - NODE_HALF.x
            && pos.x <= node_pos.x + NODE_HALF.x
            && pos.y >= node_pos.y - NODE_HALF.y
            && pos.y <= node_pos.y + NODE_HALF.y
        {
            return Some(entity);
        }
    }
    None
}

/// Tracks last click on empty space for double-click detection.
#[derive(Resource, Default)]
pub struct LastEmptyClick {
    pub time: f64,
    pub pos: Vec2,
}

const DBL_CLICK_MS: f64 = 400.0;
const DBL_CLICK_DIST: f32 = 25.0;

/// Left-click to select a CanvasNode and begin dragging it.
///
/// Skipped entirely in VimInsert so that typing is never interrupted by
/// an accidental click.  In any other mode a click on a node:
///   1. Clears the previous selection.
///   2. Inserts `Selected` and `Dragging { offset }` on the clicked entity.
///   3. Transitions to `Standard` mode.
/// Click on edge label area selects that edge for inline label editing.
/// A click on empty canvas deselects.
pub fn mouse_selection_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut commands: Commands,
    mut last_empty: ResMut<LastEmptyClick>,
    mut selected_edge: ResMut<SelectedEdge>,
    node_query: Query<(Entity, &Transform), With<CanvasNode>>,
    edge_query: Query<(Entity, &Edge)>,
    node_transform_query: Query<&Transform, With<CanvasNode>>,
    selected_q: Query<Entity, With<Selected>>,
    dragging_q: Query<Entity, With<Dragging>>,
    mut next_state: ResMut<NextState<InputMode>>,
    current_state: Res<State<InputMode>>,
    config: Res<crate::core::config::GlyphConfig>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if *current_state.get() == InputMode::VimInsert {
        return;
    }
    // Space+click: pan mode, don't select or drag
    if keys.pressed(KeyCode::Space) {
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

    let shift = crate::core::helpers::shift_pressed(&keys);

    // Edge label hit-test first (before nodes). Click on label area selects edge for inline editing.
    let mut groups: std::collections::HashMap<(Entity, Entity), Vec<Entity>> =
        std::collections::HashMap::new();
    for (entity, edge) in &edge_query {
        groups
            .entry((edge.source, edge.target))
            .or_default()
            .push(entity);
    }
    let mut idx_map: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for (_, entities) in &groups {
        for (i, e) in entities.iter().enumerate() {
            idx_map.insert(*e, i);
        }
    }
    for (edge_entity, edge) in &edge_query {
        let Ok(src) = node_transform_query.get(edge.source) else {
            continue;
        };
        let Ok(tgt) = node_transform_query.get(edge.target) else {
            continue;
        };
        let idx = idx_map.get(&edge_entity).copied().unwrap_or(0);
        let (label_pos, _) = edge_label_world_pos(src, tgt, idx);
        if world_pos.x >= label_pos.x - LABEL_HIT_HALF.x
            && world_pos.x <= label_pos.x + LABEL_HIT_HALF.x
            && world_pos.y >= label_pos.y - LABEL_HIT_HALF.y
            && world_pos.y <= label_pos.y + LABEL_HIT_HALF.y
        {
            for prev in &selected_q {
                commands.entity(prev).remove::<Selected>();
            }
            for prev in &dragging_q {
                commands.entity(prev).remove::<Dragging>();
            }
            selected_edge.0 = Some(edge_entity);
            next_state.set(InputMode::Standard);
            info!("[SELECT] edge {:?} for label edit", edge_entity);
            return;
        }
    }

    for (entity, transform) in &node_query {
        let node_pos = transform.translation.truncate();
        if world_pos.x >= node_pos.x - NODE_HALF.x
            && world_pos.x <= node_pos.x + NODE_HALF.x
            && world_pos.y >= node_pos.y - NODE_HALF.y
            && world_pos.y <= node_pos.y + NODE_HALF.y
        {
            if shift {
                // Shift+click: start edge drawing instead of node drag
                commands.insert_resource(DrawingEdge(Some(entity)));
                selected_edge.0 = None;
                info!("[EDGE] start draw from {:?}", entity);
                return;
            }

            selected_edge.0 = None;
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

    // Click on empty space: double-click creates node, single-click deselects
    selected_edge.0 = None;
    let now = time.elapsed_secs_f64() * 1000.0;
    let is_double = (now - last_empty.time) < DBL_CLICK_MS
        && world_pos.distance(last_empty.pos) < DBL_CLICK_DIST;

    if is_double {
        for prev in &selected_q {
            commands.entity(prev).remove::<Selected>();
        }
        spawn_canvas_node(&mut commands, world_pos, "", config.node_color(), true);
        next_state.set(InputMode::VimInsert);
        info!("[CREATE] double-click → new node at {:?}", world_pos);
        last_empty.time = 0.0; // reset so third click doesn't create another
    } else {
        last_empty.time = now;
        last_empty.pos = world_pos;
        for prev in &selected_q {
            commands.entity(prev).remove::<Selected>();
        }
    }
}

/// While the left mouse button is held, move the dragged node to the cursor.
/// Space+drag pans instead, so we skip when Space is held.
pub fn node_drag_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut dragging_q: Query<(&mut Transform, &Dragging)>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }
    if keys.pressed(KeyCode::Space) {
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

/// Quadratic Bezier: B(t) = (1-t)²P0 + 2(1-t)tP1 + t²P2
fn bezier_point(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    u * u * p0 + 2.0 * u * t * p1 + t * t * p2
}

const PREVIEW_CURVE_SEGMENTS: usize = 24;

/// Draw a curved preview from the edge source to the cursor while dragging (matches final edge style).
pub fn edge_draw_preview_system(
    drawing: Res<DrawingEdge>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut gizmos: Gizmos,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    transform_query: Query<&Transform>,
) {
    let Some(source_entity) = drawing.0 else {
        return;
    };
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }
    let Ok(source_transform) = transform_query.get(source_entity) else {
        return;
    };
    let Some(cursor_pos) = cursor_world_pos(&window_q, &camera_q) else {
        return;
    };
    let p0 = source_transform.translation.truncate();
    let p2 = cursor_pos;
    let mid = (p0 + p2) * 0.5;
    let dir = (p2 - p0).normalize_or_zero();
    let dist = p0.distance(p2);
    let curve_mag = (dist * 0.12).min(50.0).max(15.0);
    let perp = Vec2::new(-dir.y, dir.x);
    let p1 = mid + perp * curve_mag;
    let mut prev = p0;
    for i in 1..=PREVIEW_CURVE_SEGMENTS {
        let t = i as f32 / PREVIEW_CURVE_SEGMENTS as f32;
        let pt = bezier_point(p0, p1, p2, t);
        gizmos.line_2d(prev, pt, Color::srgb(0.4, 0.6, 0.9));
        prev = pt;
    }
}

/// On mouse release: complete edge if over a node, else cancel.
pub fn edge_draw_drop_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut drawing: ResMut<DrawingEdge>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    node_query: Query<(Entity, &Transform), With<CanvasNode>>,
) {
    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(source_entity) = drawing.0 else {
        return;
    };
    let Some(cursor_pos) = cursor_world_pos(&window_q, &camera_q) else {
        drawing.0 = None;
        return;
    };
    if let Some(target_entity) = node_at_pos(&node_query, cursor_pos) {
        if target_entity != source_entity {
            commands.spawn(Edge {
                source: source_entity,
                target: target_entity,
                label: None,
            });
            info!("[EDGE] created {:?} → {:?}", source_entity, target_entity);
        }
    }
    drawing.0 = None;
}
