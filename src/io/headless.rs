//! Headless export: `--headless --export output.png`
//!
//! Uses Bevy 0.18's `Screenshot` component + `save_to_disk` observer pattern.
//! The window is created but kept invisible when `--headless` is active.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// CLI configuration for headless mode.
#[derive(Resource)]
pub struct HeadlessExportConfig {
    pub is_headless: bool,
    pub export_path: Option<String>,
}

/// System that triggers a screenshot on the second frame, then exits.
///
/// We wait two frames so that the scene (nodes, edges, gizmos) is fully rendered
/// before capturing.
pub fn headless_export_system(
    mut commands: Commands,
    mut frames: Local<u32>,
    config: Res<HeadlessExportConfig>,
) {
    if !config.is_headless {
        return;
    }

    *frames += 1;

    // Wait for frame 3 to ensure the scene has fully rendered
    if *frames == 3 {
        if let Some(path) = &config.export_path {
            let path = path.clone();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            info!("[HEADLESS] Screenshot queued");
        }
    }

    // Give one extra frame for the screenshot to be captured and saved
    if *frames == 5 {
        info!("[HEADLESS] Export complete, exiting");
        std::process::exit(0);
    }
}
