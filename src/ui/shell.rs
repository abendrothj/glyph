//! Shell command execution: select a node, press `!`, type a command,
//! and the stdout is spawned as a new connected node.
//!
//! Uses an egui overlay similar to the fuzzy finder for command input.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::core::components::{CanvasNode, Edge, Selected, TextData};
use crate::core::helpers::spawn_node_with_color;

/// Resource controlling the shell command overlay.
#[derive(Resource, Default)]
pub struct ShellCommandState {
    pub is_open: bool,
    pub command: String,
    pub needs_focus: bool,
    /// The selected node's entity and text at the time `!` was pressed.
    pub source_entity: Option<Entity>,
    pub source_text: String,
}

/// System to detect `!` (Shift+1) in VimNormal and open the shell overlay.
pub fn shell_trigger_system(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<crate::core::state::InputMode>>,
    mut shell: ResMut<ShellCommandState>,
    selected_q: Query<(Entity, &TextData), With<Selected>>,
) {
    if *state.get() != crate::core::state::InputMode::VimNormal {
        return;
    }
    let shift = crate::core::helpers::shift_pressed(&keys);
    if shift && keys.just_pressed(KeyCode::Digit1) {
        if let Ok((entity, text_data)) = selected_q.single() {
            shell.is_open = true;
            shell.command.clear();
            shell.needs_focus = true;
            shell.source_entity = Some(entity);
            shell.source_text = text_data.content.clone();
        }
    }
}

/// The egui overlay that captures the shell command and executes it.
pub fn shell_command_ui_system(
    mut contexts: EguiContexts,
    mut shell: ResMut<ShellCommandState>,
    mut commands: Commands,
    config: Res<crate::core::config::GlyphConfig>,
    transform_q: Query<&Transform, With<CanvasNode>>,
) {
    if !shell.is_open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut should_close = false;
    let mut execute = false;

    egui::Window::new("! Shell")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
        .default_width(400.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("!")
                        .monospace()
                        .strong()
                        .color(egui::Color32::from_rgb(255, 180, 50)),
                );
                let response = ui.add(
                    egui::TextEdit::singleline(&mut shell.command)
                        .hint_text("shell command (node text piped to stdin)...")
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                if shell.needs_focus {
                    response.request_focus();
                    shell.needs_focus = false;
                }
            });

            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));

            if esc {
                should_close = true;
            }
            if enter && !shell.command.is_empty() {
                execute = true;
                should_close = true;
            }
        });

    if execute {
        let cmd_str = shell.command.clone();
        let source_text = shell.source_text.clone();
        let source_entity = shell.source_entity;

        // Run the shell command with the node's text piped to stdin
        let output = run_shell_command(&cmd_str, &source_text);

        // Spawn a new node with the output
        if let Some(source_entity) = source_entity {
            let source_pos = transform_q
                .get(source_entity)
                .map(|t| t.translation.truncate())
                .unwrap_or(Vec2::ZERO);
            let new_pos = source_pos + Vec2::new(220.0, -150.0);

            let output_text = if output.len() > 500 {
                format!("{}…", &output[..497])
            } else {
                output
            };

            let new_entity = spawn_node_with_color(
                &mut commands,
                new_pos.x,
                new_pos.y,
                &output_text,
                config.node_color(),
            );

            commands.spawn(Edge {
                source: source_entity,
                target: new_entity,
                label: Some(format!("!{}", cmd_str)),
            });

            info!("[SHELL] !{} → {:?}", cmd_str, new_entity);
        }
    }

    if should_close {
        shell.is_open = false;
    }
}

/// Execute a shell command, piping `stdin_text` to its stdin, returning stdout.
fn run_shell_command(cmd: &str, stdin_text: &str) -> String {
    let result = Command::new("sh")
        .args(["-c", cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match result {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(stdin_text.as_bytes());
            }
            match child.wait_with_output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if stdout.is_empty() && !stderr.is_empty() {
                        format!("stderr: {}", stderr)
                    } else {
                        stdout
                    }
                }
                Err(e) => format!("error: {}", e),
            }
        }
        Err(e) => format!("error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo_command() {
        let output = run_shell_command("echo hello", "");
        assert_eq!(output, "hello");
    }

    #[test]
    fn run_cat_pipes_stdin() {
        let output = run_shell_command("cat", "piped text");
        assert_eq!(output, "piped text");
    }

    #[test]
    fn run_wc_counts_stdin() {
        let output = run_shell_command("wc -c", "12345");
        // wc -c counts bytes; output is "5" (possibly with leading whitespace)
        let count: usize = output.trim().parse().unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn stderr_fallback_when_stdout_empty() {
        let output = run_shell_command("echo error >&2", "");
        assert!(output.starts_with("stderr:"), "got: {}", output);
    }

    #[test]
    fn invalid_command_returns_error_or_stderr() {
        let output = run_shell_command("this_command_does_not_exist_xyz", "");
        // Should contain either "error" or "not found" from stderr
        assert!(
            output.contains("error") || output.contains("not found") || output.contains("stderr"),
            "got: {}",
            output
        );
    }

    #[test]
    fn shell_command_state_default() {
        let state = ShellCommandState::default();
        assert!(!state.is_open);
        assert!(state.command.is_empty());
        assert!(state.source_entity.is_none());
    }
}
