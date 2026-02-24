//! Phase 7: Immediate-mode UI overlays via bevy_egui.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::sync::mpsc;

use crate::components::{CanvasNode, Edge};
use crate::io::{
    load_from_path, save_to_path, CurrentFile, FileDialogResult, PendingFileDialog, WORKSPACE_PATH,
};
use crate::resources::SpatialIndex;
use crate::state::InputMode;

/// Command palette state. Cmd+K toggles.
#[derive(Resource, Default)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub search_query: String,
}

fn is_super_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight)
}

/// Toggle command palette on Cmd+K. Runs in Update before egui.
pub fn toggle_command_palette_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut palette: ResMut<CommandPaletteState>,
) {
    if keys.just_pressed(KeyCode::KeyK) && is_super_pressed(&keys) {
        palette.is_open = !palette.is_open;
        if palette.is_open {
            palette.search_query.clear();
        }
    }
}

/// Top bar: File menu + status. Replaces Bevy UI menu.
pub fn ui_top_bar_system(
    mut contexts: EguiContexts,
    state: Res<State<InputMode>>,
    mut pending_dialog: ResMut<PendingFileDialog>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &Sprite),
        With<CanvasNode>,
    >,
    edge_query: Query<&Edge>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::top("top_bar")
        .default_height(36.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("glyph", &["glyph"])
                                .pick_file()
                            {
                                let _ = tx.send(FileDialogResult::Open(path));
                            }
                        });
                        *pending_dialog.0.lock().unwrap() = Some(rx);
                        ui.close();
                    }
                    if ui.button("Save").clicked() {
                        let path = current_file
                            .0
                            .clone()
                            .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));
                        match save_to_path(&path, &node_data_query, &edge_query) {
                            Ok(()) => {
                                current_file.0 = Some(path.clone());
                                info!("[SAVE] Saved to {}", path.display());
                            }
                            Err(e) => error!("[SAVE] {}", e),
                        }
                        ui.close();
                    }
                    if ui.button("Save As...").clicked() {
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("glyph", &["glyph"])
                                .set_file_name("untitled.glyph")
                                .save_file()
                            {
                                let _ = tx.send(FileDialogResult::SaveAs(path));
                            }
                        });
                        *pending_dialog.0.lock().unwrap() = Some(rx);
                        ui.close();
                    }
                });

                ui.separator();

                let mode_text = match state.get() {
                    InputMode::VimNormal => "VIM NORMAL",
                    InputMode::VimInsert => "INSERT",
                    InputMode::VimEasymotion => "EASYMOTION",
                    InputMode::Standard => "STANDARD",
                };
                ui.label(egui::RichText::new(mode_text).strong());
                ui.add_space(8.0);
                let hint = match state.get() {
                    InputMode::Standard => "Esc: Vim bindings",
                    InputMode::VimNormal => "i: insert  f: easymotion",
                    InputMode::VimInsert => "Esc: normal",
                    InputMode::VimEasymotion => "Type letter to jump",
                };
                ui.label(egui::RichText::new(hint).color(egui::Color32::GRAY));
            });
        });
}

/// Processes file dialog results from background thread.
pub fn process_pending_file_dialog_system(
    mut pending_dialog: ResMut<PendingFileDialog>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &Sprite),
        With<CanvasNode>,
    >,
    edge_query: Query<&Edge>,
) {
    let mut guard = match pending_dialog.0.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(rx) = guard.as_ref() else {
        return;
    };

    match rx.try_recv() {
        Ok(FileDialogResult::Open(path)) => {
            *guard = None;
            drop(guard);
            if let Err(e) = load_from_path(
                &path,
                commands,
                spatial_index,
                current_file,
                &node_query,
                &edge_entity_query,
            ) {
                error!("[LOAD] {}", e);
            } else {
                info!("[LOAD] Loaded from {}", path.display());
            }
        }
        Ok(FileDialogResult::SaveAs(path)) => {
            *guard = None;
            drop(guard);
            match save_to_path(&path, &node_data_query, &edge_query) {
                Ok(()) => {
                    current_file.0 = Some(path.clone());
                    info!("[SAVE] Saved to {}", path.display());
                }
                Err(e) => error!("[SAVE] {}", e),
            }
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            *guard = None;
        }
    }
}

/// Command palette window. Cmd+K to open.
pub fn ui_command_palette_system(
    mut contexts: EguiContexts,
    mut palette: ResMut<CommandPaletteState>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &Sprite),
        With<CanvasNode>,
    >,
    edge_query: Query<&Edge>,
) {
    if !palette.is_open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let center = ctx.input(|i: &egui::InputState| i.viewport_rect().center());

    egui::Window::new("Command Palette")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .default_pos(center)
        .default_size(egui::vec2(400.0, 280.0))
        .show(ctx, |ui| {
            ui.add_space(8.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut palette.search_query)
                    .hint_text("Search commands...")
                    .desired_width(f32::INFINITY),
            );
            if !response.has_focus() {
                response.request_focus();
            }
            ui.add_space(16.0);

            if ui.button("Save Workspace (Cmd+S)").clicked() {
                let path = current_file
                    .0
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));
                match save_to_path(&path, &node_data_query, &edge_query) {
                    Ok(()) => {
                        current_file.0 = Some(path.clone());
                        info!("[SAVE] Saved to {}", path.display());
                    }
                    Err(e) => error!("[SAVE] {}", e),
                }
                palette.is_open = false;
            } else if ui.button("Load Workspace (Cmd+O)").clicked() {
                let path = std::path::Path::new(WORKSPACE_PATH);
                if path.exists() {
                    if let Err(e) = load_from_path(
                        path,
                        commands,
                        spatial_index,
                        current_file,
                        &node_query,
                        &edge_entity_query,
                    ) {
                        error!("[LOAD] {}", e);
                    } else {
                        info!("[LOAD] Loaded from {}", WORKSPACE_PATH);
                    }
                } else {
                    warn!("[LOAD] {} not found", WORKSPACE_PATH);
                }
                palette.is_open = false;
            } else if ui.button("Clear Canvas").clicked() {
                for entity in node_query.iter().collect::<Vec<_>>() {
                    commands.entity(entity).despawn();
                }
                for entity in edge_entity_query.iter().collect::<Vec<_>>() {
                    commands.entity(entity).despawn();
                }
                spatial_index.clear();
                current_file.0 = None;
                palette.is_open = false;
                info!("[CLEAR] Canvas cleared");
            }
        });
}
