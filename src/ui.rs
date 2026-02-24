//! Menu bar UI for file operations.

use bevy::prelude::*;
use std::sync::mpsc;

use crate::components::{CanvasNode, Edge};
use crate::io::{load_from_path, save_to_path, CurrentFile, FileDialogResult, PendingFileDialog};
use crate::resources::SpatialIndex;

/// Marker for the menu bar container.
#[derive(Component)]
struct MenuBar;

/// Marker for the Open file button.
#[derive(Component)]
pub(crate) struct OpenButton;

/// Marker for the Save button.
#[derive(Component)]
pub(crate) struct SaveButton;

/// Marker for the Save As button.
#[derive(Component)]
pub(crate) struct SaveAsButton;

pub fn spawn_menu_bar(mut commands: Commands) {
    commands
        .spawn((
            MenuBar,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.22)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    OpenButton,
                    Button,
                    Node {
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        height: Val::Px(22.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        margin: UiRect::right(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("Open"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
            parent
                .spawn((
                    SaveButton,
                    Button,
                    Node {
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        height: Val::Px(22.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        margin: UiRect::right(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("Save"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
            parent
                .spawn((
                    SaveAsButton,
                    Button,
                    Node {
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        height: Val::Px(22.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        margin: UiRect::right(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("Save As"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
        });
}

pub fn menu_button_system(
    mut interaction_query: Query<
        (Entity, &Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    open_q: Query<Entity, With<OpenButton>>,
    save_q: Query<Entity, With<SaveButton>>,
    save_as_q: Query<Entity, With<SaveAsButton>>,
    mut pending_dialog: ResMut<PendingFileDialog>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
    node_data_query: Query<(Entity, &Transform, &crate::components::TextData, &Sprite), With<CanvasNode>>,
    edge_query: Query<&Edge>,
) {
    const HOVER: Color = Color::srgb(0.35, 0.35, 0.38);
    const IDLE: Color = Color::srgb(0.28, 0.28, 0.30);

    for (entity, interaction, mut bg) in &mut interaction_query {
        *bg = match *interaction {
            Interaction::Pressed => BackgroundColor(HOVER),
            Interaction::Hovered => BackgroundColor(HOVER),
            Interaction::None => BackgroundColor(IDLE),
        };

        if *interaction != Interaction::Pressed {
            continue;
        }

        if open_q.get(entity).is_ok() {
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
            return;
        }

        if save_q.get(entity).is_ok() {
            if let Some(p) = &current_file.0 {
                let path = p.clone();
                match save_to_path(&path, &node_data_query, &edge_query) {
                    Ok(()) => {
                        current_file.0 = Some(path.clone());
                        info!("[SAVE] Saved to {}", path.display());
                    }
                    Err(e) => error!("[SAVE] {}", e),
                }
            } else {
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
            }
            return;
        }

        if save_as_q.get(entity).is_ok() {
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
            return;
        }
    }
}

/// Processes file dialog results from background thread. Runs every frame.
pub fn process_pending_file_dialog(
    mut pending_dialog: ResMut<PendingFileDialog>,
    mut commands: Commands,
    mut spatial_index: ResMut<SpatialIndex>,
    mut current_file: ResMut<CurrentFile>,
    node_query: Query<Entity, With<CanvasNode>>,
    edge_entity_query: Query<Entity, With<Edge>>,
    node_data_query: Query<(Entity, &Transform, &crate::components::TextData, &Sprite), With<CanvasNode>>,
    edge_query: Query<&Edge>,
) {
    let mut guard = match pending_dialog.0.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(rx) = guard.as_ref() else {
        return;
    };

    let result = rx.try_recv();
    match result {
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
