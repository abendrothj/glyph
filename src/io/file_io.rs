//! Phase 6: Offline save/load pipeline. Serializes canvas to .glyph files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::components::{CanvasNode, Edge, MainCamera, NodeColor, TextData};
use crate::core::helpers::spawn_node_with_color;

/// Default path for keyboard shortcut save/load when no file is open.
pub const WORKSPACE_PATH: &str = "workspace.glyph";

/// Folder for user workflows. Created on first use.
pub const WORKFLOWS_DIR: &str = "workflows";
const RECENT_FILE: &str = "workflows/.recent.json";
const MAX_RECENT: usize = 10;

/// Returns the workflows directory path. Creates it if missing; if it already exists, we use it as ours.
pub fn workflows_dir() -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join(WORKFLOWS_DIR);
    let _ = std::fs::create_dir_all(&dir); // no-op if exists
    dir
}

/// Recent files list, persisted to workflows/.recent.json.
#[derive(Resource, Default)]
pub struct RecentFiles(pub Vec<PathBuf>);

/// Load recent files from disk. Call on startup.
pub fn load_recent() -> Vec<PathBuf> {
    let path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(RECENT_FILE);
    let Ok(data) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Save recent files to disk.
pub fn save_recent(paths: &[PathBuf]) {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = base.join(RECENT_FILE);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = std::fs::File::create(&path) {
        let _ = serde_json::to_writer(file, paths);
    }
}

/// Add a path to recent, dedupe, trim to MAX_RECENT. Call after load/save.
pub fn add_to_recent(recent: &mut RecentFiles, path: PathBuf) {
    recent.0.retain(|p| p != &path);
    recent.0.insert(0, path);
    if recent.0.len() > MAX_RECENT {
        recent.0.truncate(MAX_RECENT);
    }
    save_recent(&recent.0);
}

/// Default node color when loading files without color (backwards compat).
const DEFAULT_NODE_COLOR: [f32; 3] = [0.70, 0.85, 0.95];

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializedColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl SerializedColor {
    pub fn from_bevy(color: &Color) -> Self {
        let srgba = color.to_srgba();
        Self {
            r: srgba.red,
            g: srgba.green,
            b: srgba.blue,
        }
    }
    pub fn to_bevy(&self) -> Color {
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
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializedCameraPrefs {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Serialize, Deserialize)]
pub struct CanvasSnapshot {
    pub nodes: Vec<SerializableNode>,
    pub edges: Vec<SerializableEdge>,
    #[serde(default)]
    pub camera: Option<SerializedCameraPrefs>,
}

/// Current file path for save. None = untitled.
#[derive(Resource, Default)]
pub struct CurrentFile(pub Option<std::path::PathBuf>);

/// Pending file dialog result from background thread. Check each frame.
/// Wrapped in Mutex because Receiver is Send but not Sync.
#[derive(Resource, Default)]
pub struct PendingFileDialog(
    pub std::sync::Mutex<Option<std::sync::mpsc::Receiver<FileDialogResult>>>,
);

/// Deferred load path. Set by egui/file-dialog; processed in Update to avoid B0001.
#[derive(Resource, Default)]
pub struct PendingLoad(pub Option<std::path::PathBuf>);

pub enum FileDialogResult {
    Open(std::path::PathBuf),
    SaveAs(std::path::PathBuf),
}

fn is_save_modifier_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    crate::core::helpers::ctrl_or_cmd_pressed(keys)
}

/// Extract camera prefs from (Transform, Projection) for serialization.
pub fn camera_prefs_from_parts(transform: &Transform, proj: &Projection) -> SerializedCameraPrefs {
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    SerializedCameraPrefs {
        x: transform.translation.x,
        y: transform.translation.y,
        scale,
    }
}

/// Core save logic — writes to the given path.
pub fn save_to_path(
    path: &Path,
    node_query: &Query<(Entity, &Transform, &TextData, &NodeColor), With<CanvasNode>>,
    edge_query: &Query<(Entity, &Edge)>,
    camera_prefs: Option<SerializedCameraPrefs>,
) -> Result<(), String> {
    let mut entity_to_id = HashMap::new();
    let mut nodes = Vec::new();
    let mut next_id: u64 = 0;

    for (entity, transform, text_data, node_color) in node_query {
        let id = next_id;
        next_id += 1;
        entity_to_id.insert(entity, id);
        nodes.push(SerializableNode {
            id,
            x: transform.translation.x,
            y: transform.translation.y,
            text: text_data.content.clone(),
            color: SerializedColor::from_bevy(&node_color.0),
        });
    }

    let mut edges = Vec::new();
    for (_, edge) in edge_query {
        let Some(&source_id) = entity_to_id.get(&edge.source) else {
            continue;
        };
        let Some(&target_id) = entity_to_id.get(&edge.target) else {
            continue;
        };
        edges.push(SerializableEdge {
            source_id,
            target_id,
            label: edge.label.clone(),
        });
    }

    let camera = camera_prefs;
    let snapshot = CanvasSnapshot {
        nodes,
        edges,
        camera,
    };
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Core load logic — reads from the given path and spawns entities.
pub fn load_from_path(
    path: &Path,
    mut commands: Commands,
    mut spatial_index: ResMut<crate::core::resources::SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: &Query<Entity, With<CanvasNode>>,
    edge_entity_query: &Query<Entity, With<Edge>>,
    camera_query: &mut Query<(&mut Transform, &mut Projection), With<MainCamera>>,
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
        let entity = spawn_node_with_color(&mut commands, node.x, node.y, &node.text, color);

        id_to_entity.insert(node.id, entity);
    }

    for edge in &snapshot.edges {
        let Some(&source) = id_to_entity.get(&edge.source_id) else {
            continue;
        };
        let Some(&target) = id_to_entity.get(&edge.target_id) else {
            continue;
        };
        commands.spawn(Edge {
            source,
            target,
            label: edge.label.clone(),
        });
    }

    if let Some(prefs) = &snapshot.camera {
        if let Ok((mut transform, mut proj)) = camera_query.single_mut() {
            transform.translation.x = prefs.x;
            transform.translation.y = prefs.y;
            if let Projection::Orthographic(ref mut ortho) = *proj {
                ortho.scale = prefs.scale.clamp(0.1, 10.0);
            }
        }
    }

    Ok(())
}

/// Save canvas on Ctrl+S (or Cmd+S). Uses current file, else workspace.glyph.
/// Menu bar Save As still opens a file dialog for multi-file.
pub fn save_canvas_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<(Entity, &Transform, &TextData, &NodeColor), With<CanvasNode>>,
    edge_query: Query<(Entity, &Edge)>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
) {
    if !keys.just_pressed(KeyCode::KeyS) || !is_save_modifier_pressed(&keys) {
        return;
    }

    let path = current_file
        .0
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));

    let cam_prefs = camera_query
        .single()
        .ok()
        .map(|(t, p)| camera_prefs_from_parts(t, p));
    match save_to_path(&path, &node_query, &edge_query, cam_prefs) {
        Ok(()) => {
            current_file.0 = Some(path.clone());
            info!("[SAVE] Saved to {}", path.display());
        }
        Err(e) => error!("[SAVE] Failed: {}", e),
    }
}

/// Processes PendingLoad set by egui/file-dialog. Runs in Update to avoid
/// B0001 conflict with egui systems that only need read-only camera.
pub fn process_pending_load_system(
    mut pending: ResMut<PendingLoad>,
    mut recent: ResMut<RecentFiles>,
    commands: Commands,
    spatial_index: ResMut<crate::core::resources::SpatialIndex>,
    current_file: ResMut<CurrentFile>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    let Some(path) = pending.0.take() else {
        return;
    };
    match load_from_path(
        &path,
        commands,
        spatial_index,
        current_file,
        &node_query,
        &edge_entity_query,
        &mut camera_query,
    ) {
        Ok(()) => {
            add_to_recent(&mut recent, path.clone());
            info!("[LOAD] Loaded from {}", path.display());
        }
        Err(e) => error!("[LOAD] {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that change current_dir to avoid races.
    static IO_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn load_recent_empty_when_missing() {
        let _g = IO_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let loaded = load_recent();
        std::env::set_current_dir(&old).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn save_recent_load_recent_roundtrip() {
        let _g = IO_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let paths = vec![PathBuf::from("a.glyph"), PathBuf::from("b.glyph")];
        save_recent(&paths);
        let loaded = load_recent();
        std::env::set_current_dir(&old).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0], paths[0]);
        assert_eq!(loaded[1], paths[1]);
    }

    #[test]
    fn add_to_recent_trims_to_max() {
        let _g = IO_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let mut recent = RecentFiles::default();
        for i in 0..15 {
            add_to_recent(&mut recent, PathBuf::from(format!("f{}.glyph", i)));
        }
        std::env::set_current_dir(&old).unwrap();
        assert_eq!(recent.0.len(), MAX_RECENT);
    }

    #[test]
    fn add_to_recent_dedupes_and_trims() {
        let _g = IO_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let mut recent = RecentFiles::default();
        add_to_recent(&mut recent, PathBuf::from("a.glyph"));
        add_to_recent(&mut recent, PathBuf::from("b.glyph"));
        add_to_recent(&mut recent, PathBuf::from("a.glyph")); // dedupe: a moves to front
        std::env::set_current_dir(&old).unwrap();
        assert_eq!(recent.0.len(), 2);
        assert_eq!(recent.0[0], PathBuf::from("a.glyph"));
        assert_eq!(recent.0[1], PathBuf::from("b.glyph"));
    }

    #[test]
    fn workflows_dir_creates_and_returns_path() {
        let _g = IO_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let wf = workflows_dir();
        std::env::set_current_dir(&old).unwrap();
        assert!(wf.ends_with("workflows"));
        assert!(dir.path().join("workflows").exists());
    }

    #[test]
    fn canvas_snapshot_roundtrip() {
        let snapshot = CanvasSnapshot {
            nodes: vec![
                SerializableNode {
                    id: 0,
                    x: 10.0,
                    y: 20.0,
                    text: "node1".to_string(),
                    color: SerializedColor {
                        r: 0.5,
                        g: 0.6,
                        b: 0.7,
                    },
                },
                SerializableNode {
                    id: 1,
                    x: 100.0,
                    y: 200.0,
                    text: "node2".to_string(),
                    color: SerializedColor {
                        r: 0.7,
                        g: 0.85,
                        b: 0.95,
                    },
                },
            ],
            edges: vec![SerializableEdge {
                source_id: 0,
                target_id: 1,
                label: Some("calls".to_string()),
            }],
            camera: Some(SerializedCameraPrefs {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
            }),
        };
        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        let loaded: CanvasSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.nodes[0].text, "node1");
        assert_eq!(loaded.edges[0].label.as_deref(), Some("calls"));
    }
}

/// Load canvas on Ctrl+O (or Cmd+O). Reads workspace.glyph directly.
/// Menu bar Open still opens a file picker for multi-file.
pub fn load_canvas_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Commands,
    spatial_index: ResMut<crate::core::resources::SpatialIndex>,
    current_file: ResMut<CurrentFile>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
) {
    if !keys.just_pressed(KeyCode::KeyO) || !is_save_modifier_pressed(&keys) {
        return;
    }

    let path = std::path::Path::new(WORKSPACE_PATH);
    if !path.exists() {
        warn!(
            "[LOAD] {} not found (save first with Ctrl+S / Cmd+S)",
            WORKSPACE_PATH
        );
        return;
    }

    match load_from_path(
        path,
        commands,
        spatial_index,
        current_file,
        &node_query,
        &edge_entity_query,
        &mut camera_query,
    ) {
        Ok(()) => info!("[LOAD] Loaded from {}", WORKSPACE_PATH),
        Err(e) => error!("[LOAD] Failed to deserialize: {}", e),
    }
}
