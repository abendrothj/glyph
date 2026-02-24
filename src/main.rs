//! Vim whiteboard — modular entry point.

mod camera;
mod components;
mod crawler;
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
use components::{MainCamera, Selected};
use helpers::spawn_node_with_color;
use easymotion::{jump_tag_cleanup, jump_tag_setup, vim_easymotion_system};
use egui_overlay::{
    process_pending_file_dialog_system, toggle_command_palette_system, ui_command_palette_system,
    ui_top_bar_system, CommandPaletteState,
};
use input::{standard_mode_system, vim_insert_system, vim_normal_system, PendingDDelete};
use io::{load_canvas_system, process_pending_load_system, save_canvas_system, CurrentFile, PendingLoad};
use rendering::{
    draw_edges_system, draw_selection_system, sync_edge_labels_system, sync_text_system,
};
use resources::{JumpMap, SpatialIndex};
use selection::{
    edge_draw_drop_system, edge_draw_preview_system, mouse_selection_system, node_drag_system,
    node_drop_system, DrawingEdge, LastEmptyClick,
};
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
        .init_resource::<PendingDDelete>()
        .init_resource::<input::PendingGE>()
        .init_resource::<input::PendingY>()
        .init_resource::<input::PendingCE>()
        .init_resource::<input::HjklHoldTime>()
        .init_resource::<input::EasymotionConnectSource>()
        .init_resource::<input::BackspaceHoldTime>()
        .init_resource::<LastEmptyClick>()
        .init_resource::<DrawingEdge>()
        .init_resource::<egui_overlay::EdgeLabelEditBuffer>()
        .init_resource::<io::PendingFileDialog>()
        .init_resource::<PendingLoad>()
        .init_resource::<crawler::PendingCrawl>()
        .add_systems(Startup, (setup_canvas, setup_gizmo_line_width))
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
                process_pending_load_system,
                crawler::process_crawl_request_system,
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
                edge_draw_preview_system
                    .run_if(in_state(InputMode::Standard))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                edge_draw_drop_system
                    .run_if(in_state(InputMode::Standard))
                    .run_if(vim_input_available)
                    .run_if(not(egui_wants_any_keyboard_input)),
                draw_selection_system,
                sync_text_system,
                sync_edge_labels_system,
            ),
        )
        .add_systems(bevy_egui::EguiPrimaryContextPass, ui_top_bar_system)
        .add_systems(bevy_egui::EguiPrimaryContextPass, ui_command_palette_system)
        .add_systems(Update, process_pending_file_dialog_system)
        .run();
}

fn setup_gizmo_line_width(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 4.0;
}

fn setup_canvas(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));

    let a = spawn_node_with_color(
        &mut commands,
        -300.0,
        100.0,
        "Node A",
        Color::srgb(0.45, 0.52, 0.62),
    );
    commands.entity(a).insert(Selected);

    spawn_node_with_color(
        &mut commands,
        0.0,
        0.0,
        "Node B",
        Color::srgb(0.38, 0.44, 0.52),
    );

    spawn_node_with_color(
        &mut commands,
        300.0,
        -100.0,
        "Node C",
        Color::srgb(0.48, 0.42, 0.38),
    );
}
