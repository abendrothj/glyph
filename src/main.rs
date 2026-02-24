//! Vim whiteboard — modular entry point.

mod camera;
mod components;
mod easymotion;
mod helpers;
mod input;
mod rendering;
mod resources;
mod selection;
mod spatial;
mod state;

use bevy::prelude::*;

use camera::{camera_pan_system, camera_zoom_system};
use components::{CanvasNode, MainCamera, Selected, TextData, TextLabel};
use easymotion::{jump_tag_cleanup, jump_tag_setup, vim_easymotion_system};
use input::{standard_mode_system, vim_insert_system, vim_normal_system};
use rendering::{draw_edges_system, draw_selection_system, sync_text_system};
use resources::{JumpMap, SpatialIndex};
use selection::{mouse_selection_system, node_drag_system, node_drop_system};
use spatial::{spatial_index_cleanup_system, update_spatial_index_system};
use state::InputMode;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<InputMode>()
        .init_resource::<JumpMap>()
        .init_resource::<SpatialIndex>()
        .add_systems(Startup, setup_canvas)
        .add_systems(OnEnter(InputMode::VimEasymotion), jump_tag_setup)
        .add_systems(OnExit(InputMode::VimEasymotion), jump_tag_cleanup)
        .add_systems(
            PostUpdate,
            (
                update_spatial_index_system,
                spatial_index_cleanup_system,
            ),
        )
        .add_systems(
            Update,
            (
                camera_zoom_system,
                camera_pan_system,
                mouse_selection_system,
                node_drag_system.run_if(in_state(InputMode::Standard)),
                node_drop_system.run_if(in_state(InputMode::Standard)),
                vim_normal_system.run_if(in_state(InputMode::VimNormal)),
                vim_insert_system.run_if(in_state(InputMode::VimInsert)),
                standard_mode_system.run_if(in_state(InputMode::Standard)),
                vim_easymotion_system.run_if(in_state(InputMode::VimEasymotion)),
                draw_edges_system,
                draw_selection_system,
                sync_text_system,
            ),
        )
        .run();
}

fn setup_canvas(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));

    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.92, 0.92, 0.90), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(-300.0, 100.0, 0.0),
            CanvasNode,
            TextData { content: "Node A".to_string() },
            Selected,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node A"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });

    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.70, 0.85, 0.95), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(0.0, 0.0, 0.0),
            CanvasNode,
            TextData { content: "Node B".to_string() },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node B"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });

    commands
        .spawn((
            Sprite::from_color(Color::srgb(0.95, 0.80, 0.70), Vec2::new(160.0, 80.0)),
            Transform::from_xyz(300.0, -100.0, 0.0),
            CanvasNode,
            TextData { content: "Node C".to_string() },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Node C"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                Transform::from_xyz(0.0, 0.0, 1.0),
                TextLabel,
            ));
        });
}
