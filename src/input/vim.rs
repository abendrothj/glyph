//! Vim input mode systems (normal, insert, standard).

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::components::{Edge, NodeColor, Selected, SourceLocation, TextData};
use crate::core::helpers::{delete_node, spawn_canvas_node, spawn_node_with_color};
use crate::core::history::{apply_action, Action, UndoHistory};
use crate::core::resources::{SelectedEdge, StatusMessage};
use crate::core::state::InputMode;
use crate::input::easymotion::EasymotionTarget;
use crate::ui::overlay::VimCmdLine;

fn cursor_world_pos(
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
) -> Option<Vec2> {
    let window = window_q.iter().next()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_transform) = camera_q.iter().next()?;
    camera.viewport_to_world_2d(cam_transform, cursor).ok()
}

fn viewport_center_world(
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_q: &Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
) -> Option<Vec2> {
    let window = window_q.iter().next()?;
    let size = window.resolution.physical_size();
    let center = Vec2::new(size.x as f32 / 2.0, size.y as f32 / 2.0);
    let (camera, cam_transform) = camera_q.iter().next()?;
    camera.viewport_to_world_2d(cam_transform, center).ok()
}

#[derive(Resource, Default)]
pub struct PendingOperations {
    pub dd: bool,
    pub ge: bool,
    pub y: bool,
    pub ce: bool,
    pub mark_set: bool,
    pub mark_jump: bool,
}

impl PendingOperations {
    pub fn clear_all(&mut self) {
        self.dd = false;
        self.ge = false;
        self.y = false;
        self.ce = false;
        self.mark_set = false;
        self.mark_jump = false;
    }
}

#[derive(Resource, Default)]
pub struct HjklHoldTime(pub f32);

#[derive(Resource, Default)]
pub struct EasymotionConnectSource(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct BackspaceHoldTime(pub f32);

#[derive(Resource, Default)]
pub struct StartMovePos(pub Option<Vec2>);

#[derive(Resource, Default)]
pub struct OriginalText(pub Option<String>);

#[derive(SystemParam)]
pub struct VimNormalParams<'w, 's> {
    pub keys: Res<'w, ButtonInput<KeyCode>>,
    pub time: Res<'w, Time>,
    pub next_state: ResMut<'w, NextState<InputMode>>,
    pub commands: Commands<'w, 's>,
    pub selected_edge: ResMut<'w, SelectedEdge>,
    pub pending: ResMut<'w, PendingOperations>,
    pub hjkl_hold: ResMut<'w, HjklHoldTime>,
    pub cmdline: ResMut<'w, VimCmdLine>,
    pub marks: ResMut<'w, crate::core::marks::Marks>,
    pub history: ResMut<'w, UndoHistory>,
    pub start_move_pos: ResMut<'w, StartMovePos>,
    pub status: ResMut<'w, StatusMessage>,
    pub config: Res<'w, crate::core::config::GlyphConfig>,
    pub query: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Transform,
            &'static mut TextData,
            &'static mut NodeColor,
            Option<&'static SourceLocation>,
        ),
        (With<Selected>, Without<crate::core::components::MainCamera>),
    >,
    pub edge_query: Query<'w, 's, (Entity, &'static Edge)>,
}

const BACKSPACE_INITIAL_DELAY: f32 = 0.4;
const BACKSPACE_REPEAT_INTERVAL: f32 = 0.05;

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

fn is_movement_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::KeyH)
        || keys.pressed(KeyCode::KeyL)
        || keys.pressed(KeyCode::KeyK)
        || keys.pressed(KeyCode::KeyJ)
        || keys.pressed(KeyCode::ArrowLeft)
        || keys.pressed(KeyCode::ArrowRight)
        || keys.pressed(KeyCode::ArrowUp)
        || keys.pressed(KeyCode::ArrowDown)
}

// ── Command handlers ────────────────────────────────────────────────────────

fn handle_undo_redo(params: &mut VimNormalParams) {
    // u: Undo
    if params.keys.just_pressed(KeyCode::KeyU) && !crate::core::helpers::ctrl_pressed(&params.keys)
    {
        if let Some(action) = params.history.pop_undo() {
            info!("[UNDO] popped action: {:?}", action);
            let action_copy = action.clone();
            params.history.push_redo(action);
            let mut query = params.query.reborrow();
            apply_action(
                &action_copy,
                true,
                &mut params.commands,
                &mut query,
                &params.edge_query,
            );
        }
    }

    // Ctrl+R: Redo
    if params.keys.just_pressed(KeyCode::KeyR) && crate::core::helpers::ctrl_pressed(&params.keys) {
        if let Some(action) = params.history.pop_redo() {
            info!("[REDO] popped action: {:?}", action);
            let action_copy = action.clone();
            params.history.undo_stack.push(action);
            let mut query = params.query.reborrow();
            apply_action(
                &action_copy,
                false,
                &mut params.commands,
                &mut query,
                &params.edge_query,
            );
        }
    }
}

fn handle_command_mode_entry(params: &mut VimNormalParams) -> bool {
    let shift = crate::core::helpers::shift_pressed(&params.keys);
    if shift && params.keys.just_pressed(KeyCode::Semicolon) {
        params.pending.clear_all();
        params.cmdline.text.clear();
        params.next_state.set(InputMode::VimCommand);
        return true;
    }
    false
}

fn delete_selected_edge_or_node(params: &mut VimNormalParams) {
    if let Some(edge_entity) = params.selected_edge.0 {
        if let Ok((_, edge)) = params.edge_query.get(edge_entity) {
            params.history.push(Action::DeleteEdge {
                source: edge.source,
                target: edge.target,
                label: edge.label.clone(),
            });
        }
        params.commands.entity(edge_entity).despawn();
        params.selected_edge.0 = None;
    } else if let Some((entity, transform, text_data, node_color, ..)) =
        params.query.iter().next()
    {
        let mut node_edges = Vec::new();
        for (_, edge) in params.edge_query.iter() {
            if edge.source == entity || edge.target == entity {
                node_edges.push((edge.source, edge.target, edge.label.clone()));
            }
        }
        params.history.push(Action::DeleteNode {
            pos: transform.translation.truncate(),
            text: text_data.content.clone(),
            color: node_color.0,
            edges: node_edges,
        });
        delete_node(&mut params.commands, entity, &params.edge_query);
    }
}

fn handle_dd_delete(params: &mut VimNormalParams) -> bool {
    if params.keys.just_pressed(KeyCode::KeyD) {
        if params.pending.ge {
            params.pending.ge = false;
            if let Some((_, _, _, _, Some(src))) = params.query.iter().next() {
                open_in_editor(&src.file, src.line);
            }
            return true;
        }
        if params.pending.dd {
            params.pending.dd = false;
            delete_selected_edge_or_node(params);
        } else {
            params.pending.dd = true;
        }
        return true;
    }
    if params.keys.just_pressed(KeyCode::Delete) || params.keys.just_pressed(KeyCode::Backspace) {
        params.pending.dd = false;
        delete_selected_edge_or_node(params);
        return true;
    }
    false
}

fn handle_node_creation(
    params: &mut VimNormalParams,
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_ro_q: &Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
) -> bool {
    if !params.keys.just_pressed(KeyCode::KeyN) {
        return false;
    }
    params.pending.clear_all();
    let pos = cursor_world_pos(window_q, camera_ro_q).unwrap_or_else(|| {
        viewport_center_world(window_q, camera_ro_q).unwrap_or(Vec2::ZERO)
    });
    for (entity, ..) in params.query.iter() {
        params.commands.entity(entity).remove::<Selected>();
    }
    let entity = spawn_canvas_node(
        &mut params.commands,
        pos,
        "",
        params.config.node_color(),
        true,
    );
    params.history.push(Action::CreateNode {
        entity,
        pos,
        text: "".to_string(),
        color: params.config.node_color(),
    });
    params.next_state.set(InputMode::VimInsert);
    true
}

fn handle_insert_mode(
    params: &mut VimNormalParams,
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_ro_q: &Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
) -> bool {
    if !params.keys.just_pressed(KeyCode::KeyI) {
        return false;
    }
    params.pending.clear_all();
    if params.selected_edge.0.is_some() {
        params.next_state.set(InputMode::VimInsert);
        return true;
    }
    if params.query.iter().next().is_none() {
        let pos = cursor_world_pos(window_q, camera_ro_q).unwrap_or_else(|| {
            viewport_center_world(window_q, camera_ro_q).unwrap_or(Vec2::ZERO)
        });
        let entity = spawn_canvas_node(
            &mut params.commands,
            pos,
            "",
            params.config.node_color(),
            true,
        );
        params.history.push(Action::CreateNode {
            entity,
            pos,
            text: "".to_string(),
            color: params.config.node_color(),
        });
    }
    params.next_state.set(InputMode::VimInsert);
    true
}

fn handle_easymotion(params: &mut VimNormalParams) -> bool {
    if params.keys.just_pressed(KeyCode::KeyF) {
        params.pending.clear_all();
        params.commands.insert_resource(EasymotionTarget::Node);
        params.next_state.set(InputMode::VimEasymotion);
        return true;
    }

    if params.keys.just_pressed(KeyCode::KeyE) && params.pending.ge {
        params.pending.clear_all();
        params.commands.insert_resource(EasymotionTarget::EdgeLabel);
        params.next_state.set(InputMode::VimEasymotion);
        return true;
    }
    if params.keys.just_pressed(KeyCode::KeyG) {
        params.pending.dd = false;
        params.pending.y = false;
        params.pending.ce = false;
        params.pending.ge = true;
        return true;
    }
    false
}

fn handle_yy_duplicate(params: &mut VimNormalParams) -> bool {
    if !params.keys.just_pressed(KeyCode::KeyY) {
        return false;
    }
    params.pending.dd = false;
    params.pending.ge = false;
    params.pending.ce = false;
    if params.pending.y {
        params.pending.y = false;
        if let Some((entity, transform, text_data, node_color, _)) = params.query.iter().next() {
            let pos = transform.translation.truncate() + Vec2::new(50.0, 50.0);
            let new_entity = spawn_node_with_color(
                &mut params.commands,
                pos.x,
                pos.y,
                &text_data.content,
                node_color.0,
            );
            params.commands.entity(entity).remove::<Selected>();
            params.commands.entity(new_entity).insert(Selected);
            params.history.push(Action::CreateNode {
                entity: new_entity,
                pos,
                text: text_data.content.clone(),
                color: node_color.0,
            });
            params.next_state.set(InputMode::VimInsert);
        }
    } else {
        params.pending.y = true;
    }
    true
}

fn handle_ce_create_edge(params: &mut VimNormalParams) -> bool {
    if params.keys.just_pressed(KeyCode::KeyE) && params.pending.ce {
        params.pending.clear_all();
        if let Some((source_entity, ..)) = params.query.iter().next() {
            params
                .commands
                .insert_resource(EasymotionConnectSource(Some(source_entity)));
            params.commands.insert_resource(EasymotionTarget::Node);
            params.next_state.set(InputMode::VimEasymotion);
        }
        return true;
    }
    if params.keys.just_pressed(KeyCode::KeyC) {
        params.pending.dd = false;
        params.pending.ge = false;
        params.pending.y = false;
        params.pending.ce = true;
        return true;
    }
    false
}

fn handle_append_node(params: &mut VimNormalParams) -> bool {
    if !params.keys.just_pressed(KeyCode::KeyA) {
        return false;
    }
    params.pending.clear_all();
    if let Some((source_entity, source_transform, ..)) = params.query.iter_mut().next() {
        let new_pos = (source_transform.translation + Vec3::new(200.0, 0.0, 0.0)).truncate();
        params.commands.entity(source_entity).remove::<Selected>();
        let new_node = spawn_canvas_node(
            &mut params.commands,
            new_pos,
            "",
            params.config.node_color(),
            true,
        );
        let edge_entity = params
            .commands
            .spawn(Edge {
                source: source_entity,
                target: new_node,
                label: None,
            })
            .id();
        params.history.push(Action::CreateNode {
            entity: new_node,
            pos: new_pos,
            text: "".to_string(),
            color: params.config.node_color(),
        });
        params.history.push(Action::CreateEdge {
            entity: edge_entity,
            source: source_entity,
            target: new_node,
            label: None,
        });
        params.next_state.set(InputMode::VimInsert);
    }
    true
}

fn handle_marks(
    params: &mut VimNormalParams,
    window_q: &Query<&Window, With<PrimaryWindow>>,
    camera_ro_q: &Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
    camera_mut_q: &mut Query<
        &mut Transform,
        (With<crate::core::components::MainCamera>, Without<Selected>),
    >,
) -> bool {
    if params.keys.just_pressed(KeyCode::KeyM) {
        params.pending.clear_all();
        params.pending.mark_set = true;
        return true;
    }

    if params.keys.just_pressed(KeyCode::Quote) {
        params.pending.clear_all();
        params.pending.mark_jump = true;
        return true;
    }

    if params.pending.mark_set {
        if let Some(key) = params.keys.get_just_pressed().next() {
            if let Some(ch) = crate::core::helpers::keycode_to_char(key) {
                let pos = if let Some((_, transform, ..)) = params.query.iter().next() {
                    transform.translation.truncate()
                } else if let Some(p) = viewport_center_world(window_q, camera_ro_q) {
                    p
                } else {
                    Vec2::ZERO
                };
                crate::core::marks::set_mark(&mut params.marks, ch, pos);
                info!("[MARK] set mark '{}' at {:?}", ch, pos);
            }
            params.pending.mark_set = false;
            return true;
        }
    }

    if params.pending.mark_jump {
        if let Some(key) = params.keys.get_just_pressed().next() {
            if let Some(ch) = crate::core::helpers::keycode_to_char(key) {
                if let Some(pos) = crate::core::marks::get_mark(&params.marks, ch) {
                    if let Some(mut cam_transform) = camera_mut_q.iter_mut().next() {
                        cam_transform.translation.x = pos.x;
                        cam_transform.translation.y = pos.y;
                        info!("[MARK] jumped to mark '{}' at {:?}", ch, pos);
                    }
                }
            }
            params.pending.mark_jump = false;
            return true;
        }
    }
    false
}

fn handle_hjkl_movement(params: &mut VimNormalParams) {
    // Capture move end
    if let Some(start_pos) = params.start_move_pos.0 {
        if let Some((entity, transform, ..)) = params.query.iter().next() {
            if !is_movement_pressed(&params.keys) {
                let end_pos = transform.translation.truncate();
                if (start_pos - end_pos).length() > 0.1 {
                    params.history.push(Action::MoveNode {
                        entity,
                        from: start_pos,
                        to: end_pos,
                    });
                }
                params.start_move_pos.0 = None;
                params.hjkl_hold.0 = 0.0;
            }
        }
    }

    // Node movement
    if let Some((_, mut node_transform, ..)) = params.query.iter_mut().next() {
        if is_movement_pressed(&params.keys) {
            if params.start_move_pos.0.is_none() {
                params.start_move_pos.0 = Some(node_transform.translation.truncate());
            }
            params.hjkl_hold.0 += params.time.delta_secs();
            let speed = if params.hjkl_hold.0 > params.config.hjkl_accel_threshold {
                params.config.hjkl_base_speed * params.config.hjkl_accel_mult
            } else {
                params.config.hjkl_base_speed
            };
            if params.keys.pressed(KeyCode::KeyH) || params.keys.pressed(KeyCode::ArrowLeft) {
                node_transform.translation.x -= speed;
            }
            if params.keys.pressed(KeyCode::KeyL) || params.keys.pressed(KeyCode::ArrowRight) {
                node_transform.translation.x += speed;
            }
            if params.keys.pressed(KeyCode::KeyK) || params.keys.pressed(KeyCode::ArrowUp) {
                node_transform.translation.y += speed;
            }
            if params.keys.pressed(KeyCode::KeyJ) || params.keys.pressed(KeyCode::ArrowDown) {
                node_transform.translation.y -= speed;
            }
        }
    }
}

// ── Main system orchestrator ────────────────────────────────────────────────

pub fn vim_normal_system(
    mut params: VimNormalParams,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_ro_q: Query<(&Camera, &GlobalTransform), With<crate::core::components::MainCamera>>,
    mut camera_mut_q: Query<
        &mut Transform,
        (With<crate::core::components::MainCamera>, Without<Selected>),
    >,
) {
    handle_undo_redo(&mut params);
    if handle_command_mode_entry(&mut params) { return; }
    if handle_dd_delete(&mut params) { return; }
    if handle_node_creation(&mut params, &window_q, &camera_ro_q) { return; }
    if handle_insert_mode(&mut params, &window_q, &camera_ro_q) { return; }
    if handle_easymotion(&mut params) { return; }
    if handle_yy_duplicate(&mut params) { return; }
    if handle_ce_create_edge(&mut params) { return; }
    if handle_append_node(&mut params) { return; }
    if handle_marks(&mut params, &window_q, &camera_ro_q, &mut camera_mut_q) { return; }
    handle_hjkl_movement(&mut params);
}

// ── Insert mode ─────────────────────────────────────────────────────────────

pub fn vim_insert_system(
    keys: Res<ButtonInput<Key>>,
    keycodes: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut backspace_hold: ResMut<BackspaceHoldTime>,
    mut original_text: ResMut<OriginalText>,
    mut history: ResMut<UndoHistory>,
    mut next_state: ResMut<NextState<InputMode>>,
    selected_edge: Res<SelectedEdge>,
    mut edge_query: Query<&mut Edge>,
    mut query: Query<(Entity, &mut TextData), With<Selected>>,
) {
    let ctrl = keycodes.pressed(KeyCode::ControlLeft) || keycodes.pressed(KeyCode::ControlRight);

    if original_text.0.is_none() {
        if let Some(edge_entity) = selected_edge.0 {
            if let Ok(edge) = edge_query.get(edge_entity) {
                original_text.0 = Some(edge.label.clone().unwrap_or_default());
            }
        } else if let Some((_, text_data)) = query.iter().next() {
            original_text.0 = Some(text_data.content.clone());
        }
    }

    if keys.just_pressed(Key::Escape) || (ctrl && keycodes.just_pressed(KeyCode::BracketLeft)) {
        if let Some(old_text) = original_text.0.take() {
            if let Some(edge_entity) = selected_edge.0 {
                if let Ok(edge) = edge_query.get(edge_entity) {
                    let new_text = edge.label.clone().unwrap_or_default();
                    if old_text != new_text {
                        history.push(Action::EditText {
                            entity: edge_entity,
                            old: old_text,
                            new: new_text,
                        });
                    }
                }
            } else if let Some((entity, text_data)) = query.iter().next() {
                let new_text = text_data.content.clone();
                if old_text != new_text {
                    history.push(Action::EditText {
                        entity,
                        old: old_text,
                        new: new_text,
                    });
                }
            }
        }

        if let Some(edge_entity) = selected_edge.0 {
            if let Ok(mut edge) = edge_query.get_mut(edge_entity) {
                if edge.label.as_deref() == Some("") {
                    edge.label = None;
                }
            }
        }
        next_state.set(InputMode::VimNormal);
        return;
    }

    if let Some(edge_entity) = selected_edge.0 {
        if let Ok(mut edge) = edge_query.get_mut(edge_entity) {
            if edge.label.is_none() {
                edge.label = Some(String::new());
            }
            let label = edge.label.as_mut().unwrap();

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
                }
            }
            return;
        }
    }

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
            if let Some((_, mut text_data)) = query.iter_mut().next() {
                text_data.content.pop();
            }
        }
        return;
    }
    backspace_hold.0 = 0.0;

    for key in keys.get_just_pressed() {
        if let Key::Character(c) = key {
            if let Some((_, mut text_data)) = query.iter_mut().next() {
                text_data.content.push_str(c.as_str());
            }
        }
    }
}

// ── Standard mode ───────────────────────────────────────────────────────────

pub fn standard_mode_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut selected_edge: ResMut<SelectedEdge>,
    mut next_state: ResMut<NextState<InputMode>>,
) {
    let ctrl = crate::core::helpers::ctrl_pressed(&keys);
    if keys.just_pressed(KeyCode::Escape) || (ctrl && keys.just_pressed(KeyCode::BracketLeft)) {
        next_state.set(InputMode::VimNormal);
    } else if keys.just_pressed(KeyCode::KeyI) && selected_edge.0.is_some() {
        next_state.set(InputMode::VimInsert);
    } else if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace))
        && selected_edge.0.is_some()
    {
        let edge_entity = selected_edge.0.take().unwrap();
        commands.entity(edge_entity).despawn();
    }
}
