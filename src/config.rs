use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub style: Style,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Window {
    pub width: u32,
    pub height: u32,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width: 640,
            height: 32,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct General {}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Style {
    pub font: Option<String>,

    pub bg_color: Option<String>,
    pub input_bg_color: Option<String>,
    pub hovered_bg_color: Option<String>,
    pub selected_bg_color: Option<String>,

    pub text_color: Option<String>,
    pub stroke_color: Option<String>,
}
