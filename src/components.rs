//! ECS components for the whiteboard.

use bevy::prelude::*;

#[derive(Component)]
pub struct CanvasNode;

/// Cached grid cell for a CanvasNode. Updated by update_spatial_index_system.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct GridCell(pub i32, pub i32);

#[derive(Component)]
pub struct TextData {
    pub content: String,
}

/// Marker — exactly one entity carries this at a time.
#[derive(Component)]
pub struct Selected;

/// Marker on the temporary floating tag labels during easymotion.
#[derive(Component)]
pub struct JumpTag;

/// Marker on the Text2d child entity that lives inside every CanvasNode.
#[derive(Component)]
pub struct TextLabel;

/// Directed edge between two CanvasNode entities.
#[derive(Component)]
pub struct Edge {
    pub source: Entity,
    pub target: Entity,
}

/// Marker for the primary 2D camera.
#[derive(Component)]
pub struct MainCamera;

/// Attached to a CanvasNode while it is being mouse-dragged.
/// `offset` is (cursor_world – node_center) at the moment the drag began,
/// so the node does not "snap" to the cursor centre.
#[derive(Component)]
pub struct Dragging {
    pub offset: Vec2,
}
