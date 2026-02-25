//! Fuzzy finder overlay — press `/` in VimNormal to search across all node text.
//!
//! Uses `fuzzy-matcher` (skim algorithm) for scoring. Results are ranked and
//! selecting one jumps the camera to that node's position.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::core::components::{CanvasNode, MainCamera, Selected, TextData};

/// Resource controlling the fuzzy finder overlay state.
#[derive(Resource, Default)]
pub struct FuzzyFinderState {
    pub is_open: bool,
    pub query: String,
    pub needs_focus: bool,
}

/// System to toggle the fuzzy finder with `/` in VimNormal mode.
/// Consumes the key so vim_normal_system doesn't get it.
pub fn fuzzy_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<crate::core::state::InputMode>>,
    mut finder: ResMut<FuzzyFinderState>,
) {
    if *state.get() != crate::core::state::InputMode::VimNormal {
        return;
    }
    if keys.just_pressed(KeyCode::Slash) {
        finder.is_open = !finder.is_open;
        if finder.is_open {
            finder.query.clear();
            finder.needs_focus = true;
        }
    }
}

/// The egui overlay that renders the fuzzy finder window.
pub fn fuzzy_finder_ui_system(
    mut contexts: EguiContexts,
    mut finder: ResMut<FuzzyFinderState>,
    mut commands: Commands,
    node_query: Query<(Entity, &Transform, &TextData), With<CanvasNode>>,
    selected_q: Query<Entity, With<Selected>>,
    mut camera_q: Query<&mut Transform, (With<MainCamera>, Without<CanvasNode>)>,
) {
    if !finder.is_open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Collect and score all nodes
    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(Entity, Vec2, String, i64)> = Vec::new();

    for (entity, transform, text_data) in &node_query {
        let text = &text_data.content;
        if finder.query.is_empty() {
            scored.push((entity, transform.translation.truncate(), text.clone(), 0));
        } else if let Some(score) = matcher.fuzzy_match(text, &finder.query) {
            scored.push((
                entity,
                transform.translation.truncate(),
                text.clone(),
                score,
            ));
        }
    }
    scored.sort_by(|a, b| b.3.cmp(&a.3));
    // Cap at 15 results for performance
    scored.truncate(15);

    let mut should_close = false;
    let mut jump_target: Option<(Entity, Vec2)> = None;

    egui::Window::new("🔍 Find Node")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
        .default_width(400.0)
        .show(ctx, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut finder.query)
                    .hint_text("Search nodes...")
                    .desired_width(f32::INFINITY),
            );
            if finder.needs_focus {
                response.request_focus();
                finder.needs_focus = false;
            }

            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));

            if esc {
                should_close = true;
                return;
            }

            ui.add_space(4.0);

            for (entity, pos, text, score) in &scored {
                let label = if finder.query.is_empty() {
                    text.clone()
                } else {
                    format!("{} ({})", text, score)
                };
                let display = if label.len() > 60 {
                    format!("{}…", &label[..59])
                } else {
                    label
                };
                if ui.selectable_label(false, &display).clicked()
                    || (enter && jump_target.is_none())
                {
                    jump_target = Some((*entity, *pos));
                    should_close = true;
                }
            }

            if scored.is_empty() && !finder.query.is_empty() {
                ui.label(
                    egui::RichText::new("No matches")
                        .color(egui::Color32::GRAY)
                        .italics(),
                );
            }
        });

    if should_close {
        finder.is_open = false;
    }

    if let Some((target_entity, target_pos)) = jump_target {
        // Deselect previous
        for prev in &selected_q {
            commands.entity(prev).remove::<Selected>();
        }
        // Select the target
        commands.entity(target_entity).insert(Selected);
        // Jump camera
        if let Ok(mut cam_transform) = camera_q.single_mut() {
            cam_transform.translation.x = target_pos.x;
            cam_transform.translation.y = target_pos.y;
        }
        info!("[FUZZY] Jumped to {:?} at {:?}", target_entity, target_pos);
    }
}
