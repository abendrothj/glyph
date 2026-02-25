//! Vim input mode systems (normal, insert, standard).

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::{Edge, NodeColor, Selected, SourceLocation, TextData};
use crate::easymotion::EasymotionTarget;
use crate::egui_overlay::VimCmdLine;
use crate::helpers::{delete_node, spawn_canvas_node, spawn_node_with_color};
use crate::resources::SelectedEdge;
use crate::state::InputMode;

fn cursor_world_pos(
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<crate::components::MainCamera>>,
) -> Option<Vec2> {
    let window = window_q.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_transform) = camera_q.single().ok()?;
    camera.viewport_to_world_2d(cam_transform, cursor).ok()
}

fn viewport_center_world(
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<crate::components::MainCamera>>,
) -> Option<Vec2> {
    let window = window_q.single().ok()?;
    let size = window.resolution.physical_size();
    let center = Vec2::new(size.x as f32 / 2.0, size.y as f32 / 2.0);
    let (camera, cam_transform) = camera_q.single().ok()?;
    camera.viewport_to_world_2d(cam_transform, center).ok()
}

/// Group of all vim operation pending states to stay under Bevy's 16 parameter system limit
#[derive(Resource, Default)]
pub struct PendingOperations {
    pub dd: bool,
    pub ge: bool,
    pub y: bool,
    pub ce: bool,
    pub mark_set: bool,
    pub mark_jump: bool,
}

/// Hold time for hjkl acceleration. Resets when key released.
#[derive(Resource, Default)]
pub struct HjklHoldTime(pub f32);

/// When set, easymotion creates edge from this entity to selected target instead of just jumping.
#[derive(Resource, Default)]
pub struct EasymotionConnectSource(pub Option<Entity>);

/// Hold time for Backspace/Ctrl+h repeat in Insert mode.
#[derive(Resource, Default)]
pub struct BackspaceHoldTime(pub f32);

const HJKL_BASE: f32 = 10.0;
const BACKSPACE_INITIAL_DELAY: f32 = 0.4;
const BACKSPACE_REPEAT_INTERVAL: f32 = 0.05;

const HJKL_ACCEL_THRESHOLD: f32 = 0.25;
const HJKL_ACCEL_MULT: f32 = 2.5;

/// Open a source file at the given line in the user's preferred editor.
///
/// Editor detection priority:
/// 1. `$EDITOR` / `$VISUAL` env var.
/// 2. Falls back to `code` (VS Code) if neither is set.
///
/// VS Code / Cursor / Windsurf → `--goto file:line`
/// Zed                         → `file:line` positional arg
/// Terminal editors (vim/nvim) → new Terminal.app window via osascript (macOS)
fn open_in_editor(file: &str, line: u32) {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "code".to_string());

    if editor.contains("code") || editor.contains("cursor") || editor.contains("windsurf") {
        let _ = std::process::Command::new(&editor)
            .args(["--goto", &format!("{}:{}", file, line)])
            .spawn();
    } else if editor.contains("zed") {
        let _ = std::process::Command::new(&editor)
            .arg(format!("{}:{}", file, line))
            .spawn();
    } else {
        // Terminal editor — open a new Terminal.app window on macOS.
        let safe_file = file.replace('\'', r"'\''");
        let cmd = format!("{} +{} '{}'", editor, line, safe_file);
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                &format!("tell application \"Terminal\" to do script \"{}\"", cmd),
            ])
            .spawn();
    }
}

/// VimNormal: hjkl movement, i/f/a mode switches, n=create, dd=delete. All home-row.
pub fn vim_normal_system(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<InputMode>>,
    mut commands: Commands,
    mut selected_edge: ResMut<SelectedEdge>,
    mut query: Query<
        (
            Entity,
            &mut Transform,
            &TextData,
            &NodeColor,
            Option<&SourceLocation>,
        ),
        With<Selected>,
    >,
    mut pending: ResMut<PendingOperations>,
    mut hjkl_hold: ResMut<HjklHoldTime>,
    mut cmdline: ResMut<VimCmdLine>,
    mut marks: ResMut<crate::marks::Marks>,
    edge_query: Query<(Entity, &Edge)>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_ro_q: Query<(&Camera, &GlobalTransform), With<crate::components::MainCamera>>,
    mut camera_mut_q: Query<
        &mut Transform,
        (With<crate::components::MainCamera>, Without<Selected>),
    >,
) {
    // `:` (Shift+;) — enter command-line mode
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift && keys.just_pressed(KeyCode::Semicolon) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        pending.mark_set = false;
        pending.mark_jump = false;
        cmdline.text.clear();
        next_state.set(InputMode::VimCommand);
        info!("→ VimCommand");
        return;
    }

    // dd or Delete/Backspace: delete selected edge (if SelectedEdge) or selected node
    // gd (g then d): go to definition — open source file in editor at the function's line.
    if keys.just_pressed(KeyCode::KeyD) {
        if pending.ge {
            pending.ge = false;
            if let Ok((_, _, _, _, loc)) = query.single() {
                if let Some(src) = loc {
                    open_in_editor(&src.file, src.line);
                    info!("[GD] → {}:{}", src.file, src.line);
                } else {
                    info!("[GD] selected node has no source location (hand-drawn node?)");
                }
            }
            return;
        }
        if pending.dd {
            pending.dd = false;
            if let Some(edge_entity) = selected_edge.0 {
                commands.entity(edge_entity).despawn();
                selected_edge.0 = None;
                info!("[DELETE] dd → removed edge {:?}", edge_entity);
            } else if let Ok((entity, ..)) = query.single() {
                delete_node(&mut commands, entity, &edge_query);
                info!("[DELETE] dd → removed node {:?}", entity);
            }
        } else {
            pending.dd = true;
        }
        return;
    }
    if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
        pending.dd = false;
        if let Some(edge_entity) = selected_edge.0 {
            commands.entity(edge_entity).despawn();
            selected_edge.0 = None;
            info!("[DELETE] removed edge {:?}", edge_entity);
        } else if let Ok((entity, ..)) = query.single() {
            delete_node(&mut commands, entity, &edge_query);
            info!("[DELETE] removed node {:?}", entity);
        }
        return;
    }

    // n or N: create new node at cursor (or viewport center) — home row
    if keys.just_pressed(KeyCode::KeyN) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        let pos = cursor_world_pos(&window_q, &camera_ro_q).unwrap_or_else(|| {
            viewport_center_world(&window_q, &camera_ro_q).unwrap_or(Vec2::ZERO)
        });
        for (entity, ..) in &query {
            commands.entity(entity).remove::<Selected>();
        }
        spawn_canvas_node(&mut commands, pos, "", true);
        next_state.set(InputMode::VimInsert);
        info!("[CREATE] N → new node at {:?}", pos);
        return;
    }

    // i: insert mode. If edge selected, edit its label. If node selected, edit its text. If no selection, create node first.
    if keys.just_pressed(KeyCode::KeyI) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        if selected_edge.0.is_some() {
            next_state.set(InputMode::VimInsert);
            info!("→ VimInsert (edge label)");
            return;
        }
        if query.single_mut().is_err() {
            let pos = cursor_world_pos(&window_q, &camera_ro_q).unwrap_or_else(|| {
                viewport_center_world(&window_q, &camera_ro_q).unwrap_or(Vec2::ZERO)
            });
            spawn_canvas_node(&mut commands, pos, "", true);
            info!("[CREATE] i (no selection) → new node at {:?}", pos);
        }
        next_state.set(InputMode::VimInsert);
        info!("→ VimInsert");
        return;
    }

    if keys.just_pressed(KeyCode::KeyF) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        commands.insert_resource(EasymotionTarget::Node);
        next_state.set(InputMode::VimEasymotion);
        info!("→ VimEasymotion (nodes)");
        return;
    }

    // ge: easymotion for edge label edit — letters on edges, pick one → VimInsert
    if keys.just_pressed(KeyCode::KeyE) && pending.ge {
        pending.dd = false;
        pending.y = false;
        pending.ge = false;
        pending.ce = false;
        commands.insert_resource(EasymotionTarget::EdgeLabel);
        next_state.set(InputMode::VimEasymotion);
        info!("→ VimEasymotion (edge labels)");
        return;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        pending.dd = false;
        pending.y = false;
        pending.ce = false;
        pending.ge = true;
        return;
    }

    // yy: duplicate selected node
    if keys.just_pressed(KeyCode::KeyY) {
        pending.dd = false;
        pending.ge = false;
        pending.ce = false;
        if pending.y {
            pending.y = false;
            if let Ok((entity, transform, text_data, node_color, _)) = query.single() {
                let pos = transform.translation.truncate() + Vec2::new(50.0, 50.0);
                let new_entity = spawn_node_with_color(
                    &mut commands,
                    pos.x,
                    pos.y,
                    &text_data.content,
                    node_color.0,
                );
                commands.entity(entity).remove::<Selected>();
                commands.entity(new_entity).insert(Selected);
                next_state.set(InputMode::VimInsert);
                info!("[DUPLICATE] yy → {:?}", new_entity);
            }
        } else {
            pending.y = true;
        }
        return;
    }

    // ce: connect selected to existing (enters easymotion to pick target node)
    if keys.just_pressed(KeyCode::KeyE) && pending.ce {
        pending.dd = false;
        pending.ge = false;
        pending.ce = false;
        if let Ok((source_entity, ..)) = query.single() {
            commands.insert_resource(EasymotionConnectSource(Some(source_entity)));
            commands.insert_resource(EasymotionTarget::Node);
            next_state.set(InputMode::VimEasymotion);
            info!("[VIM] ce → connect to...");
        }
        return;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = true;
        return;
    }

    // a: add edge + new node (requires selection)
    if keys.just_pressed(KeyCode::KeyA) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        if let Ok((source_entity, source_transform, ..)) = query.single_mut() {
            let new_pos = (source_transform.translation + Vec3::new(200.0, 0.0, 0.0)).truncate();
            commands.entity(source_entity).remove::<Selected>();
            let new_node = spawn_canvas_node(&mut commands, new_pos, "", true);
            commands.spawn(Edge {
                source: source_entity,
                target: new_node,
                label: None,
            });
            next_state.set(InputMode::VimInsert);
            info!("[GRAPH] Edge {:?} → {:?}", source_entity, new_node);
        }
        return;
    }

    // m: Start mark set sequence
    if keys.just_pressed(KeyCode::KeyM) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        pending.mark_jump = false;
        pending.mark_set = true;
        return;
    }

    // ': Start mark jump sequence (Apostrophe is Quote)
    if keys.just_pressed(KeyCode::Quote) {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        pending.mark_set = false;
        pending.mark_jump = true;
        return;
    }

    // Handle second character for mark setting
    if pending.mark_set {
        if let Some(key) = keys.get_just_pressed().next() {
            if let Some(ch) = crate::helpers::keycode_to_char(key) {
                // Determine the position to save. If there's a selected node, use its pos.
                // Otherwise use the viewport center.
                let pos = if let Ok((_, transform, ..)) = query.single() {
                    transform.translation.truncate()
                } else if let Some(p) = viewport_center_world(&window_q, &camera_ro_q) {
                    p
                } else {
                    Vec2::ZERO
                };
                crate::marks::set_mark(&mut marks, ch, pos);
                info!("[MARK] set mark '{}' at {:?}", ch, pos);
                // Also show it in the status message
                // This requires adding StatusMessage as a ResMut to vim_normal_system. Wait, it's easier to just log it for now and update later if needed.
            }
            pending.mark_set = false;
            return;
        }
    }

    // Handle second character for mark jumping
    if pending.mark_jump {
        if let Some(key) = keys.get_just_pressed().next() {
            if let Some(ch) = crate::helpers::keycode_to_char(key) {
                if let Some(pos) = crate::marks::get_mark(&marks, ch) {
                    // Update camera position
                    if let Ok(mut cam_transform) = camera_mut_q.single_mut() {
                        cam_transform.translation.x = pos.x;
                        cam_transform.translation.y = pos.y;
                        info!("[MARK] jumped to mark '{}' at {:?}", ch, pos);
                    }
                }
            }
            pending.mark_jump = false;
            return;
        }
    }

    let Ok((_, mut transform, ..)) = query.single_mut() else {
        return;
    };

    // Arrow keys also move the selected node in VimNormal (camera pan is suppressed
    // for arrows when a node is selected — see camera_pan_keys_system).
    let moving = keys.pressed(KeyCode::KeyH)
        || keys.pressed(KeyCode::KeyL)
        || keys.pressed(KeyCode::KeyK)
        || keys.pressed(KeyCode::KeyJ)
        || keys.pressed(KeyCode::ArrowLeft)
        || keys.pressed(KeyCode::ArrowRight)
        || keys.pressed(KeyCode::ArrowUp)
        || keys.pressed(KeyCode::ArrowDown);
    if moving {
        pending.dd = false;
        pending.ge = false;
        pending.y = false;
        pending.ce = false;
        hjkl_hold.0 += time.delta_secs();
    } else {
        hjkl_hold.0 = 0.0;
    }
    let speed = if hjkl_hold.0 > HJKL_ACCEL_THRESHOLD {
        HJKL_BASE * HJKL_ACCEL_MULT
    } else {
        HJKL_BASE
    };
    if keys.pressed(KeyCode::KeyH) || keys.pressed(KeyCode::ArrowLeft) {
        transform.translation.x -= speed;
    }
    if keys.pressed(KeyCode::KeyL) || keys.pressed(KeyCode::ArrowRight) {
        transform.translation.x += speed;
    }
    if keys.pressed(KeyCode::KeyK) || keys.pressed(KeyCode::ArrowUp) {
        transform.translation.y += speed;
    }
    if keys.pressed(KeyCode::KeyJ) || keys.pressed(KeyCode::ArrowDown) {
        transform.translation.y -= speed;
    }
}

/// VimInsert: capture typed text via ButtonInput<Key>. Ctrl+[ and Ctrl+h are home-row Esc/Backspace.
/// Hold Backspace/Ctrl+h for repeat delete.
/// Edits node TextData when a node is selected, or Edge.label when an edge is selected.
pub fn vim_insert_system(
    keys: Res<ButtonInput<Key>>,
    keycodes: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut backspace_hold: ResMut<BackspaceHoldTime>,
    mut next_state: ResMut<NextState<InputMode>>,
    selected_edge: Res<SelectedEdge>,
    mut edge_query: Query<&mut Edge>,
    mut query: Query<&mut TextData, With<Selected>>,
) {
    let ctrl = keycodes.pressed(KeyCode::ControlLeft) || keycodes.pressed(KeyCode::ControlRight);

    // Esc or Ctrl+[ → normal (Ctrl+[ is home-row friendly)
    if keys.just_pressed(Key::Escape) || (ctrl && keycodes.just_pressed(KeyCode::BracketLeft)) {
        if let Some(edge_entity) = selected_edge.0 {
            if let Ok(mut edge) = edge_query.get_mut(edge_entity) {
                if edge.label.as_deref() == Some("") {
                    edge.label = None;
                }
            }
        }
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
        return;
    }

    // Editing edge label when SelectedEdge is set
    if let Some(edge_entity) = selected_edge.0 {
        if let Ok(mut edge) = edge_query.get_mut(edge_entity) {
            if edge.label.is_none() {
                edge.label = Some(String::new());
            }
            let label = edge.label.as_mut().unwrap();

            // Backspace or Ctrl+h
            let backspace_pressed =
                keys.pressed(Key::Backspace) || (ctrl && keycodes.pressed(KeyCode::KeyH));
            let backspace_just =
                keys.just_pressed(Key::Backspace) || (ctrl && keycodes.just_pressed(KeyCode::KeyH));
            if backspace_pressed {
                let mut do_delete = backspace_just;
                if backspace_just {
                    backspace_hold.0 = 0.0;
                } else {
                    backspace_hold.0 += time.delta_secs();
                    if backspace_hold.0 >= BACKSPACE_INITIAL_DELAY {
                        do_delete = true;
                        backspace_hold.0 -= BACKSPACE_REPEAT_INTERVAL;
                    }
                }
                if do_delete {
                    label.pop();
                }
                return;
            }
            backspace_hold.0 = 0.0;

            for key in keys.get_just_pressed() {
                if let Key::Character(c) = key {
                    label.push_str(c.as_str());
                    info!("[INSERT] edge label \"{}\"", label);
                }
            }
            // Normalize empty to None on next sync (we keep Some("") while editing)
            return;
        }
    }

    // Editing node text when a node is selected
    let backspace_pressed =
        keys.pressed(Key::Backspace) || (ctrl && keycodes.pressed(KeyCode::KeyH));
    let backspace_just =
        keys.just_pressed(Key::Backspace) || (ctrl && keycodes.just_pressed(KeyCode::KeyH));
    if backspace_pressed {
        let mut do_delete = backspace_just;
        if backspace_just {
            backspace_hold.0 = 0.0;
        } else {
            backspace_hold.0 += time.delta_secs();
            if backspace_hold.0 >= BACKSPACE_INITIAL_DELAY {
                do_delete = true;
                backspace_hold.0 -= BACKSPACE_REPEAT_INTERVAL;
            }
        }
        if do_delete {
            if let Ok(mut text_data) = query.single_mut() {
                text_data.content.pop();
            }
        }
        return;
    }
    backspace_hold.0 = 0.0;

    for key in keys.get_just_pressed() {
        if let Key::Character(c) = key {
            if let Ok(mut text_data) = query.single_mut() {
                text_data.content.push_str(c.as_str());
                info!("[INSERT] \"{}\" → \"{}\"", c, text_data.content);
            }
        }
    }
}

/// Standard mode: Escape or Ctrl+[ returns to VimNormal. i with SelectedEdge enters VimInsert. Delete removes selected edge.
pub fn standard_mode_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut selected_edge: ResMut<SelectedEdge>,
    mut next_state: ResMut<NextState<InputMode>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::Escape) || (ctrl && keys.just_pressed(KeyCode::BracketLeft)) {
        next_state.set(InputMode::VimNormal);
        info!("→ VimNormal");
    } else if keys.just_pressed(KeyCode::KeyI) && selected_edge.0.is_some() {
        next_state.set(InputMode::VimInsert);
        info!("→ VimInsert (edge label)");
    } else if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace))
        && selected_edge.0.is_some()
    {
        let edge_entity = selected_edge.0.take().unwrap();
        commands.entity(edge_entity).despawn();
        info!("[DELETE] removed edge {:?}", edge_entity);
    }
}
