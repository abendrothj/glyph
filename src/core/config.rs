use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Application configuration loaded from `~/.glyphrc`.
#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
pub struct GlyphConfig {
    /// Background color of the canvas in hex format (e.g., "#1e1e2e").
    pub background_color: String,
    /// Default node fill color in hex format.
    pub node_color: String,
    #[serde(default = "default_hjkl_base_speed")]
    pub hjkl_base_speed: f32,
    #[serde(default = "default_hjkl_accel_threshold")]
    pub hjkl_accel_threshold: f32,
    #[serde(default = "default_hjkl_accel_mult")]
    pub hjkl_accel_mult: f32,
    #[serde(default = "default_flow_row_height")]
    pub flow_row_height: f32,
    #[serde(default = "default_flow_node_spacing")]
    pub flow_node_spacing: f32,
    #[serde(default = "default_status_message_duration")]
    pub status_message_duration: f32,
    #[serde(default = "default_undo_history_cap")]
    pub undo_history_cap: usize,
    #[serde(default = "default_curve_segments")]
    pub curve_segments: usize,
}

fn default_hjkl_base_speed() -> f32 { 10.0 }
fn default_hjkl_accel_threshold() -> f32 { 0.25 }
fn default_hjkl_accel_mult() -> f32 { 2.5 }
fn default_flow_row_height() -> f32 { 380.0 }
fn default_flow_node_spacing() -> f32 { 320.0 }
fn default_status_message_duration() -> f32 { 4.0 }
fn default_undo_history_cap() -> usize { 100 }
fn default_curve_segments() -> usize { 24 }

impl Default for GlyphConfig {
    fn default() -> Self {
        Self {
            background_color: "#1e1e2e".to_string(), // Catppuccin Mocha Base
            node_color: "#313244".to_string(),       // Catppuccin Mocha Surface0
            hjkl_base_speed: default_hjkl_base_speed(),
            hjkl_accel_threshold: default_hjkl_accel_threshold(),
            hjkl_accel_mult: default_hjkl_accel_mult(),
            flow_row_height: default_flow_row_height(),
            flow_node_spacing: default_flow_node_spacing(),
            status_message_duration: default_status_message_duration(),
            undo_history_cap: default_undo_history_cap(),
            curve_segments: default_curve_segments(),
        }
    }
}

impl GlyphConfig {
    /// Parse the background hex string into a Bevy Color.
    pub fn bg_color(&self) -> Color {
        bevy::color::Srgba::hex(&self.background_color)
            .unwrap_or(bevy::color::Srgba::new(0.12, 0.12, 0.18, 1.0))
            .into()
    }

    /// Parse the node hex string into a Bevy Color.
    pub fn node_color(&self) -> Color {
        bevy::color::Srgba::hex(&self.node_color)
            .unwrap_or(bevy::color::Srgba::new(0.38, 0.44, 0.52, 1.0))
            .into()
    }
}

/// Attempts to load the configuration from `~/.glyphrc`.
/// Falls back to default if the file is missing or invalid.
pub fn load_config() -> GlyphConfig {
    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".glyphrc");
        if let Ok(contents) = fs::read_to_string(path) {
            match toml::from_str(&contents) {
                Ok(config) => return config,
                Err(err) => {
                    eprintln!("Failed to parse ~/.glyphrc: {}", err);
                }
            }
        }
    }
    GlyphConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_valid_colors() {
        let config = GlyphConfig::default();
        assert_eq!(config.background_color, "#1e1e2e");
        assert_eq!(config.node_color, "#313244");
    }

    #[test]
    fn bg_color_parses_hex() {
        let config = GlyphConfig {
            background_color: "#ff0000".to_string(),
            node_color: "#00ff00".to_string(),
            ..Default::default()
        };
        let bg = config.bg_color();
        let srgba = bg.to_srgba();
        assert!((srgba.red - 1.0).abs() < 0.01);
        assert!(srgba.green.abs() < 0.01);
        assert!(srgba.blue.abs() < 0.01);
    }

    #[test]
    fn node_color_parses_hex() {
        let config = GlyphConfig {
            background_color: "#000000".to_string(),
            node_color: "#00ff00".to_string(),
            ..Default::default()
        };
        let nc = config.node_color();
        let srgba = nc.to_srgba();
        assert!(srgba.red.abs() < 0.01);
        assert!((srgba.green - 1.0).abs() < 0.01);
        assert!(srgba.blue.abs() < 0.01);
    }

    #[test]
    fn invalid_hex_falls_back() {
        let config = GlyphConfig {
            background_color: "not_a_color".to_string(),
            node_color: "also_bad".to_string(),
            ..Default::default()
        };
        // Should not panic, should fall back to defaults
        let _bg = config.bg_color();
        let _nc = config.node_color();
    }

    #[test]
    fn toml_roundtrip() {
        let config = GlyphConfig {
            background_color: "#282a36".to_string(),
            node_color: "#44475a".to_string(),
            ..Default::default()
        };
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: GlyphConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.background_color, "#282a36");
        assert_eq!(parsed.node_color, "#44475a");
    }

    #[test]
    fn new_fields_roundtrip_and_default() {
        let config = GlyphConfig {
            background_color: "#1e1e2e".to_string(),
            node_color: "#313244".to_string(),
            hjkl_base_speed: 15.0,
            hjkl_accel_threshold: 0.3,
            hjkl_accel_mult: 3.0,
            flow_row_height: 400.0,
            flow_node_spacing: 350.0,
            status_message_duration: 5.0,
            undo_history_cap: 200,
            curve_segments: 32,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: GlyphConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.hjkl_base_speed, 15.0);
        assert_eq!(parsed.curve_segments, 32);
        assert_eq!(parsed.undo_history_cap, 200);

        // Minimal TOML (only colors) should use defaults for new fields
        let minimal = r##"
background_color = "#1e1e2e"
node_color = "#313244"
"##;
        let parsed: GlyphConfig = toml::from_str(minimal).unwrap();
        assert_eq!(parsed.hjkl_base_speed, 10.0);
        assert_eq!(parsed.curve_segments, 24);
        assert_eq!(parsed.undo_history_cap, 100);
    }

    #[test]
    fn load_config_returns_default_when_no_file() {
        // load_config falls back to default when file doesn't exist
        // This test just asserts it doesn't panic
        let config = load_config();
        assert!(!config.background_color.is_empty());
        assert!(!config.node_color.is_empty());
    }
}
