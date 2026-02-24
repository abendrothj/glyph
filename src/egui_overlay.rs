//! Phase 7: Immediate-mode UI overlays via bevy_egui.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use std::sync::mpsc;

use crate::components::{CanvasNode, Edge, MainCamera, Selected};
use crate::helpers::spawn_canvas_node;
use crate::io::{
    save_to_path, CurrentFile, FileDialogResult, PendingFileDialog, PendingLoad, WORKSPACE_PATH,
};
use crate::resources::SpatialIndex;
use crate::state::InputMode;

/// Command palette state. Cmd+K toggles.
#[derive(Resource, Default)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub search_query: String,
    /// Path buffer for "Open path" command.
    pub open_path_buffer: String,
}

/// Buffer for editing edge labels in the palette. Synced from edges when palette opens.
#[derive(Resource, Default)]
pub struct EdgeLabelEditBuffer(pub std::collections::HashMap<bevy::prelude::Entity, String>);

fn is_super_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight)
}

/// Toggle command palette on Cmd+K. Esc closes when open.
pub fn toggle_command_palette_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut palette: ResMut<CommandPaletteState>,
) {
    if keys.just_pressed(KeyCode::KeyK) && is_super_pressed(&keys) {
        palette.is_open = !palette.is_open;
        if palette.is_open {
            palette.search_query.clear();
            palette.open_path_buffer.clear();
        }
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if palette.is_open
        && (keys.just_pressed(KeyCode::Escape)
            || (ctrl && keys.just_pressed(KeyCode::BracketLeft)))
    {
        palette.is_open = false;
    }
}

/// Top bar: File menu + status. Replaces Bevy UI menu.
pub fn ui_top_bar_system(
    mut contexts: EguiContexts,
    state: Res<State<InputMode>>,
    mut palette: ResMut<CommandPaletteState>,
    pending_dialog: ResMut<PendingFileDialog>,
    mut current_file: ResMut<CurrentFile>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: Query<&Edge>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::top("top_bar")
        .default_height(36.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("Edit", |ui| {
                    if ui.button("Edge Labels...").clicked() {
                        palette.is_open = true;
                        ui.close();
                    }
                });
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
                        match save_to_path(&path, &node_data_query, &edge_query, &camera_query) {
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
                    InputMode::Standard => "Esc or Ctrl+[: Vim  Shift+drag: draw edge",
                    InputMode::VimNormal => "i f ge n a yy ce dd  hjkl",
                    InputMode::VimInsert => "Esc or Ctrl+[  Ctrl+h: backspace",
                    InputMode::VimEasymotion => "Type letter to jump  Esc: cancel",
                };
                ui.label(egui::RichText::new(hint).color(egui::Color32::GRAY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let name = current_file
                        .0
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled");
                    ui.label(egui::RichText::new(name).color(egui::Color32::DARK_GRAY));
                });
            });
        });
}

/// Processes file dialog results from background thread.
/// Open defers to PendingLoad (processed in Update) to avoid B0001.
pub fn process_pending_file_dialog_system(
    pending_dialog: ResMut<PendingFileDialog>,
    mut pending_load: ResMut<PendingLoad>,
    mut current_file: ResMut<CurrentFile>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: Query<&Edge>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
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
            pending_load.0 = Some(path);
        }
        Ok(FileDialogResult::SaveAs(path)) => {
            *guard = None;
            drop(guard);
            match save_to_path(&path, &node_data_query, &edge_query, &camera_query) {
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
/// Load defers to PendingLoad (processed in Update) to avoid B0001.
pub fn ui_command_palette_system(
    mut contexts: EguiContexts,
    mut palette: ResMut<CommandPaletteState>,
    pending_dialog: ResMut<PendingFileDialog>,
    mut pending_load: ResMut<PendingLoad>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    mut next_state: ResMut<NextState<crate::state::InputMode>>,
    mut label_buffer: ResMut<EdgeLabelEditBuffer>,
    node_query: Query<Entity, With<CanvasNode>>,
    mut edge_queries: ParamSet<(
        Query<(Entity, &mut Edge)>,
        Query<&Edge>,
    )>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    selected_q: Query<Entity, With<Selected>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
    camera_viewport_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
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

            let q = palette.search_query.to_lowercase();
            let show = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);

            if show("save") && ui.button("Save Workspace (Cmd+S)").clicked() {
                let path = current_file
                    .0
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));
                match save_to_path(&path, &node_data_query, &edge_queries.p1(), &camera_query) {
                    Ok(()) => {
                        current_file.0 = Some(path.clone());
                        info!("[SAVE] Saved to {}", path.display());
                    }
                    Err(e) => error!("[SAVE] {}", e),
                }
                palette.is_open = false;
            } else if show("load") && ui.button("Load workspace.glyph").clicked() {
                let path = std::path::Path::new(WORKSPACE_PATH);
                if path.exists() {
                    pending_load.0 = Some(path.to_path_buf());
                } else {
                    warn!("[LOAD] {} not found", WORKSPACE_PATH);
                }
                palette.is_open = false;
            } else if show("path") {
                let mut do_open = false;
                ui.horizontal(|ui| {
                    ui.label("Open path:");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut palette.open_path_buffer)
                            .hint_text("/path/to/workspace.glyph")
                            .desired_width(200.0),
                    );
                    if ui.button("Open").clicked() {
                        do_open = true;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        do_open = true;
                    }
                });
                if do_open {
                    let path = std::path::Path::new(palette.open_path_buffer.trim());
                    if !path.as_os_str().is_empty() && path.exists() {
                        pending_load.0 = Some(path.to_path_buf());
                        palette.open_path_buffer.clear();
                        palette.is_open = false;
                    } else if !path.as_os_str().is_empty() {
                        warn!("[LOAD] Path not found: {}", path.display());
                    }
                }
            } else if show("open") && ui.button("Open file...").clicked() {
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
                palette.is_open = false;
            } else if show("add") && ui.button("Add Node").clicked() {
                let pos = window_q.single().ok().and_then(|w| {
                    let size = w.resolution.physical_size();
                    let center = Vec2::new(size.x as f32 / 2.0, size.y as f32 / 2.0);
                    let (cam, xform) = camera_viewport_q.single().ok()?;
                    cam.viewport_to_world_2d(xform, center).ok()
                }).unwrap_or(Vec2::ZERO);
                spawn_canvas_node(&mut commands, pos, "", true);
                next_state.set(crate::state::InputMode::VimInsert);
                palette.is_open = false;
                info!("[CREATE] Add Node at {:?}", pos);
            } else if show("delete") && ui.button("Delete Selected Node").clicked() {
                if let Ok(node_entity) = selected_q.single() {
                    let to_despawn: Vec<_> = edge_queries
                        .p0()
                        .iter()
                        .filter(|(_, e)| e.source == node_entity || e.target == node_entity)
                        .map(|(e, _)| e)
                        .collect();
                    for e in &to_despawn {
                        commands.entity(*e).despawn();
                    }
                    commands.entity(node_entity).despawn();
                    let set: std::collections::HashSet<_> = to_despawn.into_iter().collect();
                    label_buffer.0.retain(|k, _| !set.contains(k));
                    palette.is_open = false;
                    info!("[DELETE] removed selected node");
                }
            } else if show("clear") && ui.button("Clear Canvas").clicked() {
                for entity in node_query.iter().collect::<Vec<_>>() {
                    commands.entity(entity).despawn();
                }
                for (e, _) in edge_queries.p0().iter() {
                    commands.entity(e).despawn();
                }
                spatial_index.clear();
                current_file.0 = None;
                label_buffer.0.clear();
                palette.is_open = false;
                info!("[CLEAR] Canvas cleared");
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Edge Labels").strong());
            ui.add_space(4.0);

            let edges: Vec<_> = edge_queries.p0().iter().map(|(e, edge)| (e, edge.clone())).collect();
            if edges.is_empty() {
                ui.label(egui::RichText::new("No edges. Shift+drag between nodes to create.").color(egui::Color32::GRAY));
            } else {
                for (edge_entity, edge) in &edges {
                    let src_name = node_data_query
                        .get(edge.source)
                        .map(|(_, _, t, _)| t.content.as_str())
                        .unwrap_or("?");
                    let tgt_name = node_data_query
                        .get(edge.target)
                        .map(|(_, _, t, _)| t.content.as_str())
                        .unwrap_or("?");
                    let label_str = edge.label.as_deref().unwrap_or("");
                    let edge_match = q.is_empty()
                        || src_name.to_lowercase().contains(&q)
                        || tgt_name.to_lowercase().contains(&q)
                        || label_str.to_lowercase().contains(&q);
                    if !edge_match {
                        continue;
                    }
                    if !label_buffer.0.contains_key(edge_entity) {
                        label_buffer.0.insert(
                            *edge_entity,
                            edge.label.as_deref().unwrap_or("").to_string(),
                        );
                    }
                    let label = label_buffer.0.get_mut(edge_entity).unwrap();
                    ui.horizontal(|ui| {
                        ui.label(format!("{} → {}:", src_name, tgt_name));
                        ui.add(
                            egui::TextEdit::singleline(label)
                                .desired_width(120.0)
                                .hint_text("label"),
                        );
                    });
                }
            }
        });

    for (edge_entity, new_label) in &label_buffer.0 {
        if let Ok((_, mut edge)) = edge_queries.p0().get_mut(*edge_entity) {
            edge.label = if new_label.is_empty() {
                None
            } else {
                Some(new_label.clone())
            };
        }
    }
}
