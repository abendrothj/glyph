//! E2E tests for the crawler: crawl → spawn nodes/edges → verify layout and topology.
//!
//! Runs headless with MinimalPlugins. Asserts on node count, positions (grid layout),
//! edge connections, and node labels (visual structure).

use bevy::prelude::*;
use glyph::core::components::{Edge, TextData};
use glyph::crawler::{handle_crawl_requests, CrawlRequest};
use glyph::render::layout::ForceLayoutActive;
use glyph::core::resources::SpatialIndex;
use glyph::core::spatial::{spatial_index_cleanup_system, update_spatial_index_system};
use std::fs;

fn crawler_e2e_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SpatialIndex>()
        .init_resource::<ForceLayoutActive>()
        .init_resource::<glyph::crawler::WatchState>()
        .init_resource::<glyph::core::resources::StatusMessage>()
        .insert_resource(glyph::core::config::GlyphConfig::default())
        .add_message::<CrawlRequest>()
        .add_systems(
            PostUpdate,
            (update_spatial_index_system, spatial_index_cleanup_system),
        )
        .add_systems(Update, handle_crawl_requests);
    app
}

/// Hierarchy layout constants (must match crawler/mod.rs).
const FLOW_ROW_HEIGHT: f32 = 380.0;

#[test]
fn e2e_crawl_spawns_nodes_in_hierarchy() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path();

    fs::write(
        dir_path.join("lib.rs"),
        r#"
fn bar() {}
fn foo() { bar(); }
fn baz() { foo(); }
"#,
    )
    .unwrap();

    let mut app = crawler_e2e_app();
    app.world_mut().write_message(CrawlRequest {
        path: dir_path.to_str().unwrap().to_string(),
        no_flow: false,
    });

    app.update();

    let world = app.world_mut();
    let nodes: Vec<_> = world
        .query::<(Entity, &Transform, &TextData)>()
        .iter(world)
        .filter(|(_, _, td)| {
            let name = td.content.as_str();
            name == "foo" || name == "bar" || name == "baz"
        })
        .collect();

    assert_eq!(nodes.len(), 3, "Expected 3 nodes (foo, bar, baz)");

    let positions: std::collections::HashMap<String, (f32, f32)> = nodes
        .into_iter()
        .map(|(_, t, td)| (td.content.clone(), (t.translation.x, t.translation.y)))
        .collect();

    // baz=root (calls no one), foo calls bar, baz calls foo → baz (top) → foo → bar (bottom)
    let bar_y = positions.get("bar").map(|(_, y)| *y).unwrap();
    let foo_y = positions.get("foo").map(|(_, y)| *y).unwrap();
    let baz_y = positions.get("baz").map(|(_, y)| *y).unwrap();

    assert!(baz_y > foo_y, "baz (root) should be above foo");
    assert!(foo_y > bar_y, "foo should be above bar");
    assert!(
        (baz_y - foo_y - FLOW_ROW_HEIGHT).abs() < 50.0,
        "baz-foo spacing ~ROW_HEIGHT"
    );
    assert!(
        (foo_y - bar_y - FLOW_ROW_HEIGHT).abs() < 50.0,
        "foo-bar spacing ~ROW_HEIGHT"
    );
}

#[test]
fn e2e_crawl_spawns_edges_between_callers_and_callees() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path();

    fs::write(
        dir_path.join("main.rs"),
        r#"
fn callee() {}
fn caller() { callee(); }
"#,
    )
    .unwrap();

    let mut app = crawler_e2e_app();
    app.world_mut().write_message(CrawlRequest {
        path: dir_path.to_str().unwrap().to_string(),
        no_flow: false,
    });

    app.update();

    let world = app.world_mut();
    let name_to_entity: std::collections::HashMap<String, Entity> = world
        .query::<(Entity, &TextData)>()
        .iter(world)
        .filter(|(_, td)| !td.content.is_empty())
        .map(|(e, td)| (td.content.clone(), e))
        .collect();

    assert!(name_to_entity.contains_key("caller"));
    assert!(name_to_entity.contains_key("callee"));

    let caller_entity = name_to_entity["caller"];
    let callee_entity = name_to_entity["callee"];

    let edges: Vec<_> = world.query::<&Edge>().iter(world).collect();
    let has_caller_to_callee = edges
        .iter()
        .any(|e| e.source == caller_entity && e.target == callee_entity);

    assert!(
        has_caller_to_callee,
        "Expected edge from caller to callee, got {} edges: {:?}",
        edges.len(),
        edges
            .iter()
            .map(|e| (e.source, e.target))
            .collect::<Vec<_>>()
    );
}

#[test]
fn e2e_crawl_visual_layout_no_overlap() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path();

    fs::write(
        dir_path.join("mod.rs"),
        r#"
fn a() {}
fn b() {}
fn c() {}
fn d() {}
fn e() {}
"#,
    )
    .unwrap();

    let mut app = crawler_e2e_app();
    app.world_mut().write_message(CrawlRequest {
        path: dir_path.to_str().unwrap().to_string(),
        no_flow: false,
    });

    app.update();

    let world = app.world_mut();
    let positions: Vec<(f32, f32)> = world
        .query::<(&Transform, &TextData)>()
        .iter(world)
        .filter(|(_, td)| !td.content.is_empty())
        .map(|(t, _)| (t.translation.x, t.translation.y))
        .collect();

    for (i, (x1, y1)) in positions.iter().enumerate() {
        for (j, (x2, y2)) in positions.iter().enumerate() {
            if i != j {
                let dx = x2 - x1;
                let dy = y2 - y1;
                let dist = (dx * dx + dy * dy).sqrt();
                assert!(
                    dist > 50.0,
                    "Nodes should not overlap: ({}, {}) and ({}, {}) are too close (dist={})",
                    x1,
                    y1,
                    x2,
                    y2,
                    dist
                );
            }
        }
    }
}
