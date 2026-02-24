//! Phase 7: Immediate-mode UI overlays via bevy_egui.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use std::sync::mpsc;

use crate::components::{CanvasNode, Edge, MainCamera, Selected};
use crate::helpers::spawn_canvas_node;
use crate::io::{
    add_to_recent, camera_prefs_from_parts, save_to_path, workflows_dir, CurrentFile,
    FileDialogResult, PendingFileDialog, PendingLoad, RecentFiles, WORKSPACE_PATH,
};
use crate::resources::SpatialIndex;
use crate::state::InputMode;

/// Command palette state. Cmd+K toggles.
#[derive(Resource, Default)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub search_query: String,
    /// When true, focus search on next frame; then allow Tab to move to buttons.
    pub needs_initial_focus: bool,
}

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
            palette.needs_initial_focus = true;
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
    pending_dialog: ResMut<PendingFileDialog>,
    mut pending_load: ResMut<PendingLoad>,
    current_file: Res<CurrentFile>,
    recent: Res<RecentFiles>,
    mut force_layout: ResMut<crate::layout::ForceLayoutActive>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: Query<(Entity, &Edge)>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
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
                                .set_directory(workflows_dir())
                                .pick_file()
                            {
                                let _ = tx.send(FileDialogResult::Open(path));
                            }
                        });
                        *pending_dialog.0.lock().unwrap() = Some(rx);
                        ui.close();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        if recent.0.is_empty() {
                            ui.label("No recent files");
                        } else {
                            for path in recent.0.iter() {
                                let label = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(path.to_str().unwrap_or("?"));
                                if ui.button(label).clicked() {
                                    pending_load.0 = Some(path.clone());
                                    ui.close();
                                }
                            }
                        }
                    });
                    if ui.button("Save").clicked() {
                        if current_file.0.is_none() {
                            // Untitled: open Save As so user can choose name
                            let (tx, rx) = mpsc::channel();
                            std::thread::spawn(move || {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("glyph", &["glyph"])
                                    .set_directory(workflows_dir())
                                    .set_file_name("untitled.glyph")
                                    .save_file()
                                {
                                    let _ = tx.send(FileDialogResult::SaveAs(path));
                                }
                            });
                            *pending_dialog.0.lock().unwrap() = Some(rx);
                        } else {
                            let path = current_file.0.clone().unwrap();
                            let cam_prefs = camera_query
                                .single()
                                .ok()
                                .map(|(t, p)| camera_prefs_from_parts(t, p));
                            match save_to_path(&path, &node_data_query, &edge_query, cam_prefs) {
                                Ok(()) => info!("[SAVE] Saved to {}", path.display()),
                                Err(e) => error!("[SAVE] {}", e),
                            }
                        }
                        ui.close();
                    }
                    if ui.button("Save As...").clicked() {
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("glyph", &["glyph"])
                                .set_directory(workflows_dir())
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
                ui.menu_button("Edit", |ui| {
                    if ui.button(if force_layout.0 { "Force Layout: On" } else { "Force Layout: Off" }).clicked() {
                        force_layout.0 = !force_layout.0;
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
                let hint = if node_data_query.is_empty() {
                    "Get started: n (new node)  Cmd+K (commands)  crawl ./src (call graph)"
                } else {
                    match state.get() {
                        InputMode::Standard => "Esc or Ctrl+[: Vim  Space+drag: pan  Shift+drag: draw edge",
                        InputMode::VimNormal => "i: insert  f: jump  n: new  a: add  yy: dup  ce: connect  dd: del  hjkl: move  Arrows: pan",
                        InputMode::VimInsert => "Esc or Ctrl+[: normal  Ctrl+h: backspace",
                        InputMode::VimEasymotion => "Type letter to jump  Esc: cancel",
                    }
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
    mut recent: ResMut<RecentFiles>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: Query<(Entity, &Edge)>,
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
            let cam_prefs = camera_query
                .single()
                .ok()
                .map(|(t, p)| camera_prefs_from_parts(t, p));
            match save_to_path(&path, &node_data_query, &edge_query, cam_prefs) {
                Ok(()) => {
                    current_file.0 = Some(path.clone());
                    add_to_recent(&mut recent, path.clone());
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
    node_query: Query<Entity, With<CanvasNode>>,
    edge_query: Query<(Entity, &Edge)>,
    node_data_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    selected_q: Query<Entity, With<Selected>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_full_q: Query<
        (&Transform, &Projection, &Camera, &GlobalTransform),
        With<MainCamera>,
    >,
    mut crawl_events: MessageWriter<crate::crawler::CrawlRequest>,
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
                    .hint_text("Search commands... or type 'crawl ./src' + Enter")
                    .desired_width(f32::INFINITY),
            );
            if palette.needs_initial_focus {
                response.request_focus();
                palette.needs_initial_focus = false;
            }
            // Intercept Enter: if text starts with "crawl ", send CrawlRequest event.
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let q = palette.search_query.trim();
                if q.starts_with("crawl ") {
                    let path = q["crawl ".len()..].trim().to_string();
                    if !path.is_empty() {
                        crawl_events.write(crate::crawler::CrawlRequest { path });
                        palette.search_query.clear();
                        palette.is_open = false;
                    }
                }
            }
            ui.add_space(16.0);

            let q = palette.search_query.to_lowercase();
            let show = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);

            if show("save") && ui.button("Save Workspace (Cmd+S)").clicked() {
                let path = current_file
                    .0
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH));
                let cam_prefs = camera_full_q
                .single()
                .ok()
                .map(|(t, p, _, _)| camera_prefs_from_parts(t, p));
                match save_to_path(&path, &node_data_query, &edge_query, cam_prefs) {
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
            } else if show("open") && ui.button("Open file...").clicked() {
                let (tx, rx) = mpsc::channel();
                let start_dir = workflows_dir();
                std::thread::spawn(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("glyph", &["glyph"])
                        .set_directory(start_dir)
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
                    let (_, _, cam, xform) = camera_full_q.single().ok()?;
                    cam.viewport_to_world_2d(xform, center).ok()
                }).unwrap_or(Vec2::ZERO);
                spawn_canvas_node(&mut commands, pos, "", true);
                next_state.set(crate::state::InputMode::VimInsert);
                palette.is_open = false;
                info!("[CREATE] Add Node at {:?}", pos);
            } else if show("delete") && ui.button("Delete Selected Node").clicked() {
                if let Ok(node_entity) = selected_q.single() {
                    let to_despawn: Vec<_> = edge_query
                        .iter()
                        .filter(|(_, e)| e.source == node_entity || e.target == node_entity)
                        .map(|(e, _)| e)
                        .collect();
                    for e in &to_despawn {
                        commands.entity(*e).despawn();
                    }
                    commands.entity(node_entity).despawn();
                    palette.is_open = false;
                    info!("[DELETE] removed selected node");
                }
            } else if show("clear") && ui.button("Clear Canvas").clicked() {
                for entity in node_query.iter().collect::<Vec<_>>() {
                    commands.entity(entity).despawn();
                }
                for (e, _) in edge_query.iter() {
                    commands.entity(e).despawn();
                }
                spatial_index.clear();
                current_file.0 = None;
                palette.is_open = false;
                info!("[CLEAR] Canvas cleared");
            }

            if !q.is_empty() && q.contains("crawl") {
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Crawl: type \"crawl ./src\" (or any path) and press Enter.")
                        .color(egui::Color32::GRAY)
                        .small(),
                );
            }
        });
}
