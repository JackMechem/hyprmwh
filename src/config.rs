use std::sync::{Arc, OnceLock, RwLock};

use iced::Color;
use iced_layershell::reexport::Anchor;
use serde::Deserialize;

static CONFIG_PATH: OnceLock<String> = OnceLock::new();
static CONFIG: RwLock<Option<Arc<Config>>> = RwLock::new(None);

pub fn set_path(path: String) {
    CONFIG_PATH.set(path).ok();
}

pub fn get() -> Arc<Config> {
    {
        let r = CONFIG.read().unwrap();
        if let Some(cfg) = r.as_ref() {
            return Arc::clone(cfg);
        }
    }
    let mut w = CONFIG.write().unwrap();
    if w.is_none() {
        *w = Some(Arc::new(Config::load()));
    }
    Arc::clone(w.as_ref().unwrap())
}

pub fn reload() {
    let mut w = CONFIG.write().unwrap();
    *w = Some(Arc::new(Config::load()));
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub window: WindowConfig,
    pub style: StyleConfig,
}

impl Config {
    fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("[config] Failed to parse {path}: {e}");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }
}

fn config_path() -> String {
    if let Some(path) = CONFIG_PATH.get() {
        return path.clone();
    }
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        format!("{home}/.config")
    });
    format!("{base}/hyprmwh/config.toml")
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct WindowConfig {
    pub anchor: AnchorPosition,
    pub width: u32,
    pub margin: i32,
    pub line_numbers: LineNumbers,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            anchor: AnchorPosition::Center,
            width: 600,
            margin: 0,
            line_numbers: LineNumbers::Relative,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LineNumbers {
    #[default]
    Relative,
    Absolute,
    Hidden,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AnchorPosition {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

impl AnchorPosition {
    pub fn to_anchor(&self) -> Anchor {
        match self {
            AnchorPosition::Center => Anchor::empty(),
            AnchorPosition::Top => Anchor::Top,
            AnchorPosition::Bottom => Anchor::Bottom,
            AnchorPosition::Left => Anchor::Left,
            AnchorPosition::Right => Anchor::Right,
        }
    }

    pub fn to_margin(&self, edge: i32) -> (i32, i32, i32, i32) {
        match self {
            AnchorPosition::Center => (0, 0, 0, 0),
            AnchorPosition::Top => (edge, 0, 0, 0),
            AnchorPosition::Bottom => (0, 0, edge, 0),
            AnchorPosition::Left => (0, 0, 0, edge),
            AnchorPosition::Right => (0, edge, 0, 0),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct StyleConfig {
    pub container_background: String,
    pub container_border: String,
    pub container_radius: f32,
    pub button_radius: f32,
    pub input_radius: f32,

    pub input_background: String,
    pub input_border_focused: String,
    pub input_border_hover: String,
    pub input_border_idle: String,

    pub button_background: String,
    pub button_hover: String,
    pub button_pressed: String,

    pub button_selected_background: String,
    pub button_selected_hover: String,
    pub button_selected_border: String,

    pub text_color: String,
    pub placeholder_color: String,

    pub statusbar_background: String,
    pub statusbar_text: String,
    pub statusbar_mode_normal: String,
    pub statusbar_mode_search: String,
    pub statusbar_mode_command: String,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            container_background:       "#26262ECC".to_string(),
            container_border:           "#FFFFFF1A".to_string(),
            container_radius:           18.0,
            button_radius:              50.0,
            input_radius:               20.0,

            input_background:           "#333340FF".to_string(),
            input_border_focused:       "#6699FFFF".to_string(),
            input_border_hover:         "#FFFFFF4D".to_string(),
            input_border_idle:          "#FFFFFF1A".to_string(),

            button_background:          "#333340FF".to_string(),
            button_hover:               "#40404CFF".to_string(),
            button_pressed:             "#4D4D61FF".to_string(),

            button_selected_background: "#405999FF".to_string(),
            button_selected_hover:      "#5973B3FF".to_string(),
            button_selected_border:     "#6699FF99".to_string(),

            text_color:                 "#FFFFFFFF".to_string(),
            placeholder_color:          "#FFFFFF66".to_string(),

            statusbar_background:       "#1A1A22FF".to_string(),
            statusbar_text:             "#FFFFFF99".to_string(),
            statusbar_mode_normal:      "#80CC80FF".to_string(),
            statusbar_mode_search:      "#FFFFFFFF".to_string(),
            statusbar_mode_command:     "#FFFFFFFF".to_string(),
        }
    }
}

pub fn parse_color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    match h.len() {
        6 => Color::from_rgb8(parse(&h[0..2]), parse(&h[2..4]), parse(&h[4..6])),
        8 => Color::from_rgba8(
            parse(&h[0..2]),
            parse(&h[2..4]),
            parse(&h[4..6]),
            parse(&h[6..8]) as f32 / 255.0,
        ),
        _ => {
            eprintln!("[config] Invalid color: {hex}");
            Color::WHITE
        }
    }
}
