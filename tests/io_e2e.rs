//! E2E tests for save/load: save round-trip, load from file, verify.

use bevy::prelude::*;
use glyph::components::{CanvasNode, Edge, MainCamera, NodeColor, TextData};
use glyph::helpers::spawn_node_with_color;
use glyph::io::{process_pending_load_system, save_to_path, CurrentFile, PendingLoad};
use glyph::resources::SpatialIndex;
use glyph::spatial::{spatial_index_cleanup_system, update_spatial_index_system};
use std::fs;
use std::path::PathBuf;

#[derive(Resource, Default)]
struct TestSavePath(pub Option<PathBuf>);

fn io_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SpatialIndex>()
        .init_resource::<CurrentFile>()
        .init_resource::<PendingLoad>()
        .init_resource::<TestSavePath>()
        .add_systems(Startup, |mut commands: Commands| {
            let n1 = spawn_node_with_color(&mut commands, 100.0, 200.0, "hello", Color::srgb(0.5, 0.6, 0.7));
            let n2 = spawn_node_with_color(&mut commands, 300.0, 400.0, "world", Color::srgb(0.8, 0.9, 1.0));
            commands.spawn(Edge {
                source: n1,
                target: n2,
                label: Some("connects".to_string()),
            });
        })
        .add_systems(
            Update,
            |mut path: ResMut<TestSavePath>,
             node_query: Query<(Entity, &Transform, &TextData, &NodeColor), With<CanvasNode>>,
             edge_query: Query<(Entity, &Edge)>| {
                if let Some(p) = path.0.take() {
                    let _ = save_to_path(&p, &node_query, &edge_query, None);
                }
            },
        )
        .add_systems(PostUpdate, (update_spatial_index_system, spatial_index_cleanup_system));
    app
}

#[test]
fn e2e_save_produces_valid_glyph() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.glyph");

    let mut app = io_test_app();
    app.update();
    *app.world_mut().resource_mut::<TestSavePath>() = TestSavePath(Some(path.clone()));
    app.update();

    assert!(path.exists());
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("\"text\": \"hello\""));
    assert!(contents.contains("\"text\": \"world\""));
    assert!(contents.contains("\"label\": \"connects\""));
}

#[test]
fn e2e_load_restores_nodes_and_edges() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("load_test.glyph");

    fs::write(
        &path,
        r#"{
  "nodes": [
    {"id": 0, "x": 50.0, "y": 100.0, "text": "alpha", "color": {"r": 0.7, "g": 0.85, "b": 0.95}},
    {"id": 1, "x": 250.0, "y": 100.0, "text": "beta", "color": {"r": 0.7, "g": 0.85, "b": 0.95}}
  ],
  "edges": [
    {"source_id": 0, "target_id": 1, "label": "calls"}
  ]
}"#,
    )
    .unwrap();

    let mut app = io_test_app();
    app.world_mut().spawn((Camera2d, MainCamera));
    *app.world_mut().resource_mut::<PendingLoad>() = PendingLoad(Some(path.clone()));
    app.add_systems(Update, process_pending_load_system);
    app.update();

    let world = app.world_mut();
    let nodes: Vec<_> = world
        .query::<(&Transform, &TextData)>()
        .iter(world)
        .filter(|(_, td)| !td.content.is_empty())
        .collect();

    assert_eq!(nodes.len(), 2);
    let texts: Vec<_> = nodes.iter().map(|(_, td)| td.content.as_str()).collect();
    assert!(texts.contains(&"alpha"));
    assert!(texts.contains(&"beta"));

    let edges: Vec<_> = world.query::<&Edge>().iter(world).collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].label.as_deref(), Some("calls"));
}
