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

/// Marker on the main (foreground) sprite child of a CanvasNode.
#[derive(Component)]
pub struct NodeMainSprite;

/// Stores the node's fill color for serialization. Synced with NodeMainSprite child.
#[derive(Component, Clone, Copy)]
pub struct NodeColor(pub Color);

/// Directed edge between two CanvasNode entities.
#[derive(Component, Clone)]
pub struct Edge {
    pub source: Entity,
    pub target: Entity,
    /// Optional label displayed along the edge path.
    pub label: Option<String>,
}

/// Marker on the Text2d child of an Edge entity for label rendering.
#[derive(Component)]
pub struct EdgeLabel;

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

/// Source file location for a crawled node.
/// Absent on hand-drawn nodes; present whenever the crawler spawned the node.
#[derive(Component, Clone)]
pub struct SourceLocation {
    /// Absolute path to the source file containing this function.
    pub file: String,
    /// 1-indexed line number of the function definition.
    pub line: u32,
}

/// Marker on the small filename Text2d rendered at the bottom of crawled nodes.
#[derive(Component)]
pub struct FileLabel;
