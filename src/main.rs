//! Vim whiteboard — modular entry point.

mod camera;
mod components;
mod easymotion;
mod egui_overlay;
mod helpers;
mod input;
mod io;
mod rendering;
mod resources;
mod selection;
mod spatial;
mod state;

use bevy::prelude::*;
use bevy_egui::{input::egui_wants_any_keyboard_input, EguiPlugin};

use camera::{camera_pan_system, camera_zoom_system};
use components::{CanvasNode, MainCamera, Selected, TextData, TextLabel};
use easymotion::{jump_tag_cleanup, jump_tag_setup, vim_easymotion_system};
use egui_overlay::{
    process_pending_file_dialog_system, toggle_command_palette_system, ui_command_palette_system,
    ui_top_bar_system, CommandPaletteState,
};
use input::{standard_mode_system, vim_insert_system, vim_normal_system};
use io::{load_canvas_system, save_canvas_system, CurrentFile};
use rendering::{draw_edges_system, draw_selection_system, sync_text_system};
use resources::{JumpMap, SpatialIndex};
use selection::{mouse_selection_system, node_drag_system, node_drop_system};
use spatial::{spatial_index_cleanup_system, update_spatial_index_system};
use state::InputMode;

/// Run Vim/input systems only when command palette is closed and egui is not
/// consuming keyboard input (e.g. typing in search bar, File menu open).
fn vim_input_available(palette: Res<CommandPaletteState>) -> bool {
    !palette.is_open
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .init_state::<InputMode>()
        .init_resource::<JumpMap>()
        .init_resource::<SpatialIndex>()
        .init_resource::<CurrentFile>()
        .init_resource::<CommandPaletteState>()
        .init_resource::<io::PendingFileDialog>()
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
                toggle_command_palette_system,
                camera_zoom_system,
                camera_pan_system,
                save_canvas_system
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                load_canvas_system
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                process_pending_file_dialog_system,
                mouse_selection_system,
                node_drag_system
                    .run_if(in_state(InputMode::Standard))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                node_drop_system.run_if(in_state(InputMode::Standard)),
                vim_normal_system
                    .run_if(in_state(InputMode::VimNormal))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                vim_insert_system
                    .run_if(in_state(InputMode::VimInsert))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                standard_mode_system
                    .run_if(in_state(InputMode::Standard))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                vim_easymotion_system
                    .run_if(in_state(InputMode::VimEasymotion))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                draw_edges_system,
                draw_selection_system,
                sync_text_system,
            ),
        )
        .add_systems(
            bevy_egui::EguiPrimaryContextPass,
            (ui_top_bar_system, ui_command_palette_system),
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
