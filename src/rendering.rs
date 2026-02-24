//! Gizmo and text rendering systems.

use bevy::prelude::*;

use crate::components::{Edge, Selected, TextData, TextLabel};
use crate::state::InputMode;

/// Draw a line between every pair of source/target entities that have an Edge.
pub fn draw_edges_system(
    mut gizmos: Gizmos,
    edge_query: Query<&Edge>,
    transform_query: Query<&Transform>,
) {
    for edge in &edge_query {
        let Ok(src) = transform_query.get(edge.source) else {
            continue;
        };
        let Ok(tgt) = transform_query.get(edge.target) else {
            continue;
        };
        gizmos.line_2d(
            src.translation.truncate(),
            tgt.translation.truncate(),
            Color::srgb(0.35, 0.35, 0.55),
        );
    }
}

/// Draw a mode-coloured rectangle outline around the selected node.
///
/// VimNormal → blue   VimInsert → green   VimEasymotion → orange
/// Standard  → purple
pub fn draw_selection_system(
    mut gizmos: Gizmos,
    selected_query: Query<&Transform, With<Selected>>,
    state: Res<State<InputMode>>,
) {
    let Ok(transform) = selected_query.single() else {
        return;
    };

    let color = match state.get() {
        InputMode::VimInsert => Color::srgb(0.2, 0.85, 0.4),
        InputMode::VimEasymotion => Color::srgb(1.0, 0.6, 0.1),
        InputMode::Standard => Color::srgb(0.85, 0.4, 0.9),
        InputMode::VimNormal => Color::srgb(0.3, 0.6, 1.0),
    };

    gizmos.rect_2d(
        Isometry2d::from_translation(transform.translation.truncate()),
        Vec2::new(170.0, 90.0),
        color,
    );
}

/// When TextData.content changes, push the new string into the child Text2d.
pub fn sync_text_system(
    changed_nodes: Query<(&TextData, &Children), Changed<TextData>>,
    mut text_query: Query<&mut Text2d, With<TextLabel>>,
) {
    for (text_data, children) in &changed_nodes {
        for child in children {
            if let Ok(mut text2d) = text_query.get_mut(*child) {
                text2d.clear();
                text2d.push_str(&text_data.content);
            }
        }
    }
}
