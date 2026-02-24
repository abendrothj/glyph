//! Phase 6: Offline save/load pipeline. Serializes canvas to .glyph files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::components::{CanvasNode, Edge, TextData, TextLabel};

/// Default path for keyboard shortcut save/load when no file is open.
pub const WORKSPACE_PATH: &str = "workspace.glyph";

/// Default node color when loading files without color (backwards compat).
const DEFAULT_NODE_COLOR: [f32; 3] = [0.70, 0.85, 0.95];

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializedColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl SerializedColor {
    fn from_bevy(color: &Color) -> Self {
        let srgba = color.to_srgba();
        Self {
            r: srgba.red,
            g: srgba.green,
            b: srgba.blue,
        }
    }
    fn to_bevy(&self) -> Color {
        Color::srgb(self.r, self.g, self.b)
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableNode {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub text: String,
    #[serde(default = "default_color")]
    pub color: SerializedColor,
}

fn default_color() -> SerializedColor {
    SerializedColor {
        r: DEFAULT_NODE_COLOR[0],
        g: DEFAULT_NODE_COLOR[1],
        b: DEFAULT_NODE_COLOR[2],
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableEdge {
    pub source_id: u64,
    pub target_id: u64,
}

#[derive(Serialize, Deserialize)]
pub struct CanvasSnapshot {
    pub nodes: Vec<SerializableNode>,
    pub edges: Vec<SerializableEdge>,
}

/// Current file path for save. None = untitled.
#[derive(Resource, Default)]
pub struct CurrentFile(pub Option<std::path::PathBuf>);

/// Pending file dialog result from background thread. Check each frame.
/// Wrapped in Mutex because Receiver is Send but not Sync.
#[derive(Resource, Default)]
pub struct PendingFileDialog(pub std::sync::Mutex<Option<std::sync::mpsc::Receiver<FileDialogResult>>>);

pub enum FileDialogResult {
    Open(std::path::PathBuf),
    SaveAs(std::path::PathBuf),
}

fn is_save_modifier_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight)
}

/// Core save logic — writes to the given path.
pub fn save_to_path(
    path: &Path,
    node_query: &Query<(Entity, &Transform, &TextData, &Sprite), With<CanvasNode>>,
    edge_query: &Query<&Edge>,
) -> Result<(), String> {
    let mut entity_to_id = HashMap::new();
    let mut nodes = Vec::new();
    let mut next_id: u64 = 0;

    for (entity, transform, text_data, sprite) in node_query {
        let id = next_id;
        next_id += 1;
        entity_to_id.insert(entity, id);
        nodes.push(SerializableNode {
            id,
            x: transform.translation.x,
            y: transform.translation.y,
            text: text_data.content.clone(),
            color: SerializedColor::from_bevy(&sprite.color),
        });
    }

    let mut edges = Vec::new();
    for edge in edge_query {
        let Some(&source_id) = entity_to_id.get(&edge.source) else {
            continue;
        };
        let Some(&target_id) = entity_to_id.get(&edge.target) else {
            continue;
        };
        edges.push(SerializableEdge {
            source_id,
            target_id,
        });
    }

    let snapshot = CanvasSnapshot { nodes, edges };
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Core load logic — reads from the given path and spawns entities.
pub fn load_from_path(
    path: &Path,
    mut commands: Commands,
    mut spatial_index: ResMut<crate::resources::SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: &Query<Entity, With<CanvasNode>>,
    edge_entity_query: &Query<Entity, With<Edge>>,
) -> Result<(), String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let snapshot: CanvasSnapshot = serde_json::from_str(&contents).map_err(|e| e.to_string())?;

    let nodes_to_despawn: Vec<Entity> = node_query.iter().collect();
    let edges_to_despawn: Vec<Entity> = edge_entity_query.iter().collect();
    for entity in nodes_to_despawn {
        commands.entity(entity).despawn();
    }
    for entity in edges_to_despawn {
        commands.entity(entity).despawn();
    }
    spatial_index.clear();

    current_file.0 = Some(path.to_path_buf());

    let mut id_to_entity = HashMap::new();

    for node in &snapshot.nodes {
        let color = node.color.to_bevy();
        let entity = commands
            .spawn((
                Sprite::from_color(color, Vec2::new(160.0, 80.0)),
                Transform::from_xyz(node.x, node.y, 0.0),
                CanvasNode,
                TextData {
                    content: node.text.clone(),
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text2d::new(node.text.clone()),
                    TextFont { font_size: 14.0, ..default() },
                    TextColor(Color::srgb(0.1, 0.1, 0.1)),
                    Transform::from_xyz(0.0, 0.0, 1.0),
                    TextLabel,
                ));
            })
            .id();

        id_to_entity.insert(node.id, entity);
    }

    for edge in &snapshot.edges {
        let Some(&source) = id_to_entity.get(&edge.source_id) else {
            continue;
        };
        let Some(&target) = id_to_entity.get(&edge.target_id) else {
            continue;
        };
        commands.spawn(Edge { source, target });
    }

    Ok(())
}

/// Save canvas on Ctrl+S (or Cmd+S). Uses current file, else workspace.glyph.
/// Menu bar Save As still opens a file dialog for multi-file.
pub fn save_canvas_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<(Entity, &Transform, &TextData, &Sprite), With<CanvasNode>>,
    edge_query: Query<&Edge>,
) {
    if !keys.just_pressed(KeyCode::KeyS) || !is_save_modifier_pressed(&keys) {
        return;
    }

    let path = current_file
        .0
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));

    match save_to_path(&path, &node_query, &edge_query) {
        Ok(()) => {
            current_file.0 = Some(path.clone());
            info!("[SAVE] Saved to {}", path.display());
        }
        Err(e) => error!("[SAVE] Failed: {}", e),
    }
}

/// Load canvas on Ctrl+O (or Cmd+O). Reads workspace.glyph directly.
/// Menu bar Open still opens a file picker for multi-file.
pub fn load_canvas_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut spatial_index: ResMut<crate::resources::SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    if !keys.just_pressed(KeyCode::KeyO) || !is_save_modifier_pressed(&keys) {
        return;
    }

    let path = std::path::Path::new(WORKSPACE_PATH);
    if !path.exists() {
        warn!("[LOAD] {} not found (save first with Ctrl+S / Cmd+S)", WORKSPACE_PATH);
        return;
    }

    match load_from_path(
        path,
        commands,
        spatial_index,
        current_file,
        &node_query,
        &edge_entity_query,
    ) {
        Ok(()) => info!("[LOAD] Loaded from {}", WORKSPACE_PATH),
        Err(e) => error!("[LOAD] Failed to deserialize: {}", e),
    }
}
