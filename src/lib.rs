//! Glyph — Vim-style 2D whiteboard. Library for testing and reuse.

pub mod core;
pub mod crawler;
pub mod input;
#[path = "io/mod.rs"]
pub mod io;
pub mod render;
pub mod ui;

use bevy::prelude::*;
use bevy_egui::{input::egui_wants_any_keyboard_input, EguiPlugin};

use core::components::MainCamera;
use core::state::InputMode;

use input::camera::{
    camera_pan_keys_system, camera_pan_system, camera_zoom_keys_system, camera_zoom_system,
};
use input::easymotion::{
    jump_tag_cleanup, jump_tag_setup, vim_easymotion_system, EasymotionTarget,
};
use input::selection::{
    edge_draw_drop_system, edge_draw_preview_system, mouse_selection_system, node_drag_system,
    node_drop_system, DrawingEdge, LastEmptyClick,
};
use input::vim::{standard_mode_system, vim_insert_system, vim_normal_system};

use io::file_io::{
    load_canvas_system, load_recent, process_pending_load_system, save_canvas_system,
    workflows_dir, CurrentFile, PendingLoad, RecentFiles,
};

use render::cluster::cluster_blobs_system;
use render::edges::{
    draw_edges_system, draw_selection_system, sync_edge_labels_system, sync_text_system,
};
use render::layout::{force_directed_layout_system, ForceLayoutActive};

use core::resources::{JumpMap, SelectedEdge, SpatialIndex};
use core::spatial::{spatial_index_cleanup_system, update_spatial_index_system};

use ui::overlay::{
    process_pending_file_dialog_system, toggle_command_palette_system, ui_bottom_bar_system,
    ui_command_palette_system, ui_legend_system, ui_top_bar_system, vim_cmdline_system,
    CommandPaletteState, VimCmdLine,
};

/// Run Vim/input systems only when command palette is closed, not in command-line
/// mode, and egui is not consuming keyboard input (e.g. typing in search bar).
fn vim_input_available(palette: Res<CommandPaletteState>, state: Res<State<InputMode>>) -> bool {
    !palette.is_open && *state.get() != InputMode::VimCommand
}

/// Build and run the Glyph app.
pub fn run() {
    use std::io::{IsTerminal, Read};
    let app_config = core::config::load_config();
    let undo_cap = app_config.undo_history_cap;

    let mut stdin_snapshot = None;
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_ok() {
            let buf = buf.trim();
            if !buf.is_empty() {
                match serde_json::from_str::<io::file_io::CanvasSnapshot>(buf) {
                    Ok(snap) => stdin_snapshot = Some(snap),
                    Err(e) => eprintln!("Failed to parse stdin as JSON: {}", e),
                }
            }
        }
    }

    let mut is_headless = false;
    let mut export_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--headless" {
            is_headless = true;
        } else if arg == "--export" {
            export_path = args.next();
        }
    }

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Glyph".to_string(),
            visible: !is_headless,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(app_config.bg_color()))
    .insert_resource(app_config)
    .insert_resource(io::headless::HeadlessExportConfig {
        is_headless,
        export_path,
    })
    .add_plugins(EguiPlugin::default())
    .init_state::<InputMode>()
    .init_resource::<JumpMap>()
    .init_resource::<SpatialIndex>()
    .init_resource::<CurrentFile>()
    .init_resource::<CommandPaletteState>()
    .init_resource::<VimCmdLine>()
    .init_resource::<ui::fuzzy::FuzzyFinderState>()
    .init_resource::<ui::shell::ShellCommandState>()
    .init_resource::<input::vim::PendingOperations>()
    .init_resource::<input::vim::HjklHoldTime>()
    .init_resource::<input::vim::EasymotionConnectSource>()
    .init_resource::<input::vim::BackspaceHoldTime>()
    .init_resource::<input::vim::StartMovePos>()
    .init_resource::<input::vim::OriginalText>()
    .init_resource::<EasymotionTarget>()
    .init_resource::<io::file_io::PendingFileDialog>()
    .init_resource::<PendingLoad>()
    .init_resource::<ForceLayoutActive>()
    .init_resource::<RecentFiles>()
    .init_resource::<crawler::WatchState>()
    .init_resource::<core::marks::Marks>()
    .insert_resource(core::history::UndoHistory { cap: undo_cap, ..Default::default() })
    // new status message resource used for command feedback/errors
    .init_resource::<core::resources::StatusMessage>()
    .init_resource::<LastEmptyClick>()
    .init_resource::<DrawingEdge>()
    .init_resource::<SelectedEdge>();

    if let Some(snap) = stdin_snapshot {
        app.insert_resource(io::stdin::StdinSnapshot(snap));
    }

    app.add_systems(Startup, |mut recent: ResMut<RecentFiles>| {
        let _ = workflows_dir(); // ensure workflows folder exists
        recent.0 = load_recent();
    })
    .add_message::<crawler::CrawlRequest>()
    .add_message::<crawler::TraceRequest>()
    .add_systems(
        Startup,
        (
            setup_canvas,
            setup_gizmo_line_width,
            io::stdin::load_stdin_snapshot_system,
        ),
    )
    .add_systems(OnEnter(InputMode::VimEasymotion), jump_tag_setup)
    .add_systems(OnExit(InputMode::VimEasymotion), jump_tag_cleanup)
    .add_systems(
        PostUpdate,
        (update_spatial_index_system, spatial_index_cleanup_system),
    )
    .add_systems(
        Update,
        (
            // Toggle command palette on Cmd+K
            toggle_command_palette_system,
            io::headless::headless_export_system,
            camera_zoom_system,
            camera_zoom_keys_system
                .run_if(vim_input_available)
                .run_if(not(egui_wants_any_keyboard_input)),
            camera_pan_system,
            camera_pan_keys_system
                .run_if(vim_input_available)
                .run_if(not(egui_wants_any_keyboard_input)),
            save_canvas_system
                .run_if(vim_input_available)
                .run_if(not(egui_wants_any_keyboard_input)),
            load_canvas_system
                .run_if(vim_input_available)
                .run_if(not(egui_wants_any_keyboard_input)),
            process_pending_load_system,
            crawler::handle_crawl_requests,
            crawler::tracing::handle_trace_requests,
            crawler::watch_trigger_system,
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
            vim_cmdline_system.run_if(in_state(InputMode::VimCommand)),
        ),
    )
    .add_systems(
        Update,
        (
            force_directed_layout_system,
            cluster_blobs_system,
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
    .add_systems(bevy_egui::EguiPrimaryContextPass, ui_bottom_bar_system)
    .add_systems(bevy_egui::EguiPrimaryContextPass, ui_legend_system)
    .add_systems(Update, process_pending_file_dialog_system)
    .add_systems(Update, crate::ui::overlay::status_message_tick_system)
    .add_systems(
        Update,
        (ui::fuzzy::fuzzy_toggle_system
            .run_if(in_state(InputMode::VimNormal))
            .run_if(vim_input_available)
            .run_if(not(egui_wants_any_keyboard_input)),),
    )
    .add_systems(
        bevy_egui::EguiPrimaryContextPass,
        ui::fuzzy::fuzzy_finder_ui_system,
    )
    .add_systems(
        Update,
        ui::shell::shell_trigger_system
            .run_if(in_state(InputMode::VimNormal))
            .run_if(vim_input_available)
            .run_if(not(egui_wants_any_keyboard_input)),
    )
    .add_systems(
        bevy_egui::EguiPrimaryContextPass,
        ui::shell::shell_command_ui_system,
    )
    .run();
}

fn setup_gizmo_line_width(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 4.0;
}

fn setup_canvas(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
}
