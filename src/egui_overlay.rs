//! Phase 7: Immediate-mode UI overlays via bevy_egui.

use bevy::input::keyboard::Key;
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
use crate::components::SourceLocation;
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

/// Vim command-line buffer. Populated when `:` is pressed in VimNormal.
#[derive(Resource, Default)]
pub struct VimCmdLine {
    pub text: String,
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

/// Top bar: File menu + mode indicator + hint. Replaces Bevy UI menu.
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
                    if ui
                        .button(if force_layout.0 {
                            "Force Layout: On"
                        } else {
                            "Force Layout: Off"
                        })
                        .clicked()
                    {
                        force_layout.0 = !force_layout.0;
                        ui.close();
                    }
                });

                ui.separator();

                let mode_text = match state.get() {
                    InputMode::VimNormal => "NORMAL",
                    InputMode::VimInsert => "INSERT",
                    InputMode::VimEasymotion => "JUMP",
                    InputMode::Standard => "STANDARD",
                    InputMode::VimCommand => "COMMAND",
                };
                ui.label(egui::RichText::new(mode_text).strong().color(match state.get() {
                    InputMode::VimInsert => egui::Color32::from_rgb(100, 200, 120),
                    InputMode::VimEasymotion => egui::Color32::from_rgb(240, 200, 60),
                    InputMode::Standard => egui::Color32::from_rgb(160, 140, 220),
                    InputMode::VimNormal => egui::Color32::WHITE,
                    InputMode::VimCommand => egui::Color32::from_rgb(220, 180, 80),
                }));
                ui.add_space(8.0);
                let hint = if node_data_query.is_empty() {
                    "n: new node   f: jump   +/-: zoom   Cmd+K: commands   crawl ./src: call graph"
                } else {
                    match state.get() {
                        InputMode::Standard =>
                            "Esc/Ctrl+[: normal   Space+drag: pan   Shift+drag: draw edge",
                        InputMode::VimNormal =>
                            "hjkl/arrows: move   f: jump   gd: open   ge: edge   i: insert   n: new   a: add   ce: connect   dd: del   +/-: zoom   :: command",
                        InputMode::VimInsert =>
                            "Esc/Ctrl+[: normal   Ctrl+h: backspace",
                        InputMode::VimEasymotion =>
                            "Type letter to jump   Esc: cancel",
                        InputMode::VimCommand =>
                            ":w · :w <path> · :e <path> · :crawl <path> [--no-flow] · :q   Esc/Ctrl+[: cancel   Enter: execute",
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
///
/// Keyboard-only usage:
/// - Type to filter actions; press Enter to execute the first visible result.
/// - Typed commands: `crawl ./path` · `open ./file.glyph` · `save ./file.glyph` + Enter
pub fn ui_command_palette_system(
    mut contexts: EguiContexts,
    mut palette: ResMut<CommandPaletteState>,
    pending_dialog: ResMut<PendingFileDialog>,
    mut pending_load: ResMut<PendingLoad>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    mut recent: ResMut<RecentFiles>,
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
        .default_size(egui::vec2(420.0, 300.0))
        .show(ctx, |ui| {
            ui.add_space(8.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut palette.search_query)
                    .hint_text("Search or: crawl ./src · open ./file · save ./file  +Enter")
                    .desired_width(f32::INFINITY),
            );
            if palette.needs_initial_focus {
                response.request_focus();
                palette.needs_initial_focus = false;
            }

            let search_has_focus = response.has_focus();
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let q_raw = palette.search_query.trim().to_string();

            // ── Typed commands (Enter executes immediately) ──────────────────
            if enter_pressed && search_has_focus {
                if q_raw.starts_with("crawl ") {
                    let arg = q_raw["crawl ".len()..].trim();
                    let (path, no_flow) = if let Some(p) = arg.strip_suffix(" --no-flow") {
                        (p.trim().to_string(), true)
                    } else {
                        (arg.to_string(), false)
                    };
                    if !path.is_empty() {
                        crawl_events.write(crate::crawler::CrawlRequest { path, no_flow });
                        palette.search_query.clear();
                        palette.is_open = false;
                    }
                    return;
                } else if q_raw.starts_with("open ") {
                    let path_str = q_raw["open ".len()..].trim().to_string();
                    if !path_str.is_empty() {
                        let path = std::path::PathBuf::from(&path_str);
                        if path.exists() {
                            pending_load.0 = Some(path);
                            palette.search_query.clear();
                            palette.is_open = false;
                        } else {
                            warn!("[OPEN] File not found: {}", path_str);
                        }
                    }
                    return;
                } else if q_raw.starts_with("save ") {
                    let path_str = q_raw["save ".len()..].trim().to_string();
                    if !path_str.is_empty() {
                        let path = std::path::PathBuf::from(&path_str);
                        let cam_prefs = camera_full_q
                            .single()
                            .ok()
                            .map(|(t, p, _, _)| camera_prefs_from_parts(t, p));
                        match save_to_path(&path, &node_data_query, &edge_query, cam_prefs) {
                            Ok(()) => {
                                current_file.0 = Some(path.clone());
                                add_to_recent(&mut recent, path.clone());
                                info!("[SAVE] Saved to {}", path.display());
                            }
                            Err(e) => error!("[SAVE] {}", e),
                        }
                        palette.search_query.clear();
                        palette.is_open = false;
                    }
                    return;
                }
            }

            ui.add_space(12.0);

            // ── Button actions ────────────────────────────────────────────────
            // Two activation paths:
            //  1. Search box focused + Enter → `first_remaining` fires the first
            //     visible button automatically (no mouse needed).
            //  2. Tab to a button → egui gives it keyboard focus → Enter (or
            //     Space) triggers button.clicked() natively.  `first_remaining`
            //     is false in this case so only the focused button activates.
            let mut first_remaining = enter_pressed && search_has_focus;
            let mut handled = false;

            let q = palette.search_query.to_lowercase();
            let show = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);

            if show("save") {
                let btn = ui.button("Save Workspace   Cmd+S");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
                    handled = true;
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
                }
            }
            if show("load") {
                let btn = ui.button("Load workspace.glyph   Cmd+O");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
                    handled = true;
                    let path = std::path::Path::new(WORKSPACE_PATH);
                    if path.exists() {
                        pending_load.0 = Some(path.to_path_buf());
                    } else {
                        warn!("[LOAD] {} not found", WORKSPACE_PATH);
                    }
                    palette.is_open = false;
                }
            }
            if show("open") {
                let btn = ui.button("Open file...");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
                    handled = true;
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
                }
            }
            if show("add") {
                let btn = ui.button("Add Node");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
                    handled = true;
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
                }
            }
            if show("delete") {
                let btn = ui.button("Delete Selected Node");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
                    handled = true;
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
                        info!("[DELETE] removed selected node");
                    }
                    palette.is_open = false;
                }
            }
            if show("clear") {
                let btn = ui.button("Clear Canvas");
                let enter = std::mem::take(&mut first_remaining);
                if (btn.clicked() || enter) && !handled {
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
            }

            // Suppress unused warning when no button was matched.
            let _ = handled;

            // ── Typed command hint ────────────────────────────────────────────
            let shows_cmd_hint = !q.is_empty()
                && (q.contains("crawl") || q.contains("open") || q.contains("save"));
            if shows_cmd_hint {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Type a command + Enter:\n  crawl ./src — build call graph\n  open ./file.glyph — open file\n  save ./file.glyph — save to path",
                    )
                    .color(egui::Color32::GRAY)
                    .small(),
                );
            }
        });
}

// ── Vim command line ──────────────────────────────────────────────────────────

/// Executes a parsed vim command. Called from `vim_cmdline_system` on Enter.
fn execute_vim_command(
    text: &str,
    current_file: &mut CurrentFile,
    recent: &mut RecentFiles,
    pending_load: &mut PendingLoad,
    status: &mut crate::resources::StatusMessage,
    node_query: &Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: &Query<(Entity, &Edge)>,
    camera_query: &Query<(&Transform, &Projection), With<MainCamera>>,
    crawl_events: &mut MessageWriter<crate::crawler::CrawlRequest>,
) {
    if text.is_empty() {
        return;
    }
    let (cmd, arg) = match text.find(' ') {
        Some(pos) => (&text[..pos], text[pos + 1..].trim()),
        None => (text, ""),
    };
    match cmd {
        "w" | "write" => {
            let path = if arg.is_empty() {
                current_file
                    .0
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(WORKSPACE_PATH))
            } else {
                std::path::PathBuf::from(arg)
            };
            let cam_prefs = camera_query
                .single()
                .ok()
                .map(|(t, p)| camera_prefs_from_parts(t, p));
            match save_to_path(&path, node_query, edge_query, cam_prefs) {
                Ok(()) => {
                    current_file.0 = Some(path.clone());
                    add_to_recent(recent, path.clone());
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                    status.set(format!("Saved {}", name));
                    info!("[CMD] :w → saved to {}", path.display());
                }
                Err(e) => {
                    status.set(format!("Error: {}", e));
                    error!("[CMD] :w failed: {}", e);
                }
            }
        }
        "e" | "edit" => {
            if arg.is_empty() {
                status.set("error: :e requires a path");
                warn!("[CMD] :e requires a path");
            } else {
                let path = std::path::PathBuf::from(arg);
                if path.exists() {
                    pending_load.0 = Some(path);
                } else {
                    status.set(format!("error: file not found: {}", arg));
                    warn!("[CMD] :e — file not found: {}", arg);
                }
            }
        }
        "crawl" => {
            if arg.is_empty() {
                status.set("error: :crawl requires a path");
                warn!("[CMD] :crawl requires a path");
            } else {
                let (path, no_flow) = if let Some(p) = arg.strip_suffix(" --no-flow") {
                    (p.trim(), true)
                } else if let Some(p) = arg.strip_prefix("--no-flow ") {
                    (p.trim(), true)
                } else {
                    (arg, false)
                };
                crawl_events.write(crate::crawler::CrawlRequest {
                    path: path.to_string(),
                    no_flow,
                });
                info!("[CMD] :crawl {} (no_flow={})", path, no_flow);
            }
        }
        "q" | "quit" => {
            info!("[CMD] :q");
            std::process::exit(0);
        }
        _ => {
            status.set(format!("error: unknown command: :{}", text));
            warn!("[CMD] Unknown command: :{}", text);
        }
    }
}

/// Captures keyboard input while in `VimCommand` state.
/// Esc/Ctrl+[ cancels; Enter executes the command.
pub fn vim_cmdline_system(
    keys: Res<ButtonInput<Key>>,
    keycodes: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<crate::state::InputMode>>,
    mut cmdline: ResMut<VimCmdLine>,
    mut current_file: ResMut<CurrentFile>,
    mut recent: ResMut<RecentFiles>,
    mut pending_load: ResMut<PendingLoad>,
    mut status: ResMut<crate::resources::StatusMessage>,
    node_query: Query<
        (Entity, &Transform, &crate::components::TextData, &crate::components::NodeColor),
        With<CanvasNode>,
    >,
    edge_query: Query<(Entity, &Edge)>,
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
    mut crawl_events: MessageWriter<crate::crawler::CrawlRequest>,
) {
    let ctrl = keycodes.pressed(KeyCode::ControlLeft) || keycodes.pressed(KeyCode::ControlRight);

    // Cancel: Esc or Ctrl+[
    if keys.just_pressed(Key::Escape)
        || (ctrl && keycodes.just_pressed(KeyCode::BracketLeft))
    {
        cmdline.text.clear();
        next_state.set(crate::state::InputMode::VimNormal);
        info!("→ VimNormal (cmdline cancelled)");
        return;
    }

    // Execute: Enter
    if keys.just_pressed(Key::Enter) {
        let text = cmdline.text.trim().to_string();
        cmdline.text.clear();
        next_state.set(crate::state::InputMode::VimNormal);
        info!("→ VimNormal (executed: :{})", text);
        execute_vim_command(
            &text,
            &mut current_file,
            &mut recent,
            &mut pending_load,
            &mut status,
            &node_query,
            &edge_query,
            &camera_query,
            &mut crawl_events,
        );
        return;
    }

    // Backspace
    if keys.just_pressed(Key::Backspace) {
        cmdline.text.pop();
        return;
    }

    // Character input (Key::Space is not a Key::Character in Bevy)
    for key in keys.get_just_pressed() {
        match key {
            Key::Character(c) => cmdline.text.push_str(c.as_str()),
            Key::Space => cmdline.text.push(' '),
            _ => {}
        }
    }
}

/// Floating legend panel: lists each source file with its halo color swatch.
/// Only shown when crawled nodes (nodes with SourceLocation) are present.
pub fn ui_legend_system(
    mut contexts: EguiContexts,
    node_query: Query<&SourceLocation, With<CanvasNode>>,
) {
    // Collect unique absolute paths, sorted for stable ordering.
    let mut files: Vec<String> = node_query
        .iter()
        .map(|loc| loc.file.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if files.is_empty() {
        return;
    }
    files.sort();

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Modules")
        .resizable(false)
        .collapsible(true)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 40.0))
        .show(ctx, |ui| {
            for file in &files {
                let (r, g, b) = crate::cluster::palette_rgb(file);
                let swatch = egui::Color32::from_rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
                // Show only the filename, not the full path.
                let basename = std::path::Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file.as_str());
                ui.horizontal(|ui| {
                    // Colored square swatch
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(14.0, 14.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 3.0, swatch);
                    ui.label(egui::RichText::new(basename).small());
                });
            }
        });
}

/// Decrement the status message timer each frame so messages fade out.
pub fn status_message_tick_system(
    time: Res<Time>,
    mut status: ResMut<crate::resources::StatusMessage>,
) {
    if status.timer > 0.0 {
        status.timer = (status.timer - time.delta_secs()).max(0.0);
    }
}

/// Bottom bar: mode indicator and vim command line.
/// Shows `-- MODE --` normally; shows `:[text]|` in VimCommand.
/// A status message (crawl result / error) is shown on the right when active.
pub fn ui_bottom_bar_system(
    mut contexts: EguiContexts,
    state: Res<State<crate::state::InputMode>>,
    cmdline: Res<VimCmdLine>,
    status: Res<crate::resources::StatusMessage>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::bottom("bottom_bar")
        .default_height(22.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                match state.get() {
                    crate::state::InputMode::VimCommand => {
                        ui.label(
                            egui::RichText::new(format!(":{}|", cmdline.text))
                                .monospace()
                                .color(egui::Color32::WHITE),
                        );
                    }
                    crate::state::InputMode::VimNormal => {
                        ui.label(
                            egui::RichText::new("-- NORMAL --")
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                    }
                    crate::state::InputMode::VimInsert => {
                        ui.label(
                            egui::RichText::new("-- INSERT --")
                                .strong()
                                .color(egui::Color32::from_rgb(100, 200, 120)),
                        );
                    }
                    crate::state::InputMode::VimEasymotion => {
                        ui.label(
                            egui::RichText::new("-- JUMP --")
                                .strong()
                                .color(egui::Color32::from_rgb(240, 200, 60)),
                        );
                    }
                    crate::state::InputMode::Standard => {
                        ui.label(
                            egui::RichText::new("-- STANDARD --")
                                .strong()
                                .color(egui::Color32::from_rgb(160, 140, 220)),
                        );
                    }
                }

                // Status / error message — right-aligned, fades over the last second.
                if status.timer > 0.0 && !status.text.is_empty() {
                    let alpha = (status.timer.min(1.0) * 255.0) as u8;
                    let is_error = status.text.starts_with("crawl: ")
                        || status.text.starts_with("Error")
                        || status.text.starts_with("error");
                    let color = if is_error {
                        egui::Color32::from_rgba_premultiplied(240, 100, 80, alpha)
                    } else {
                        egui::Color32::from_rgba_premultiplied(100, 220, 130, alpha)
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(&status.text).color(color));
                    });
                }
            });
        });
}
