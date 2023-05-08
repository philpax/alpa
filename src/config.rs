use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub model: Model,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Model {
    pub path: PathBuf,
    pub context_token_length: usize,
    pub architecture: String,
    pub prefer_mmap: bool,
}
impl Model {
    pub fn architecture(&self) -> Option<llm::ModelArchitecture> {
        self.architecture.parse().ok()
    }
}
impl Default for Model {
    fn default() -> Self {
        Self {
            path: "models/7B/ggml-alpaca-q4_0.bin".into(),
            context_token_length: 2048,
            architecture: llm::ModelArchitecture::Llama.to_string(),
            prefer_mmap: true,
        }
    }
}

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
