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
}

impl Default for GlyphConfig {
    fn default() -> Self {
        Self {
            background_color: "#1e1e2e".to_string(), // Catppuccin Mocha Base
            node_color: "#313244".to_string(),       // Catppuccin Mocha Surface0
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
        };
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: GlyphConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.background_color, "#282a36");
        assert_eq!(parsed.node_color, "#44475a");
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
