//! Input mode state machine.

use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum InputMode {
    Standard,
    #[default]
    VimNormal,
    VimInsert,
    VimEasymotion,
    /// Vim `:` command-line mode. Active while user types a command.
    VimCommand,
}
