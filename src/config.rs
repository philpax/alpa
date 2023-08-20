use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    command::{Command, InputMethod, PromptMode},
    keycode::Keycode,
};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub model: Model,
    #[serde(default = "default_commands")]
    pub commands: Vec<Command>,
}

fn default_commands() -> Vec<Command> {
    vec![Command::new(
        [Keycode::LControl, Keycode::Escape],
        InputMethod::SingleLineUi,
        PromptMode::Prompt(
            "SYSTEM: You are a general AI assistant.\nUSER: {{PROMPT}}\nASSISTANT: ".to_string(),
        ),
    )]
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
    pub use_gpu: bool,
}
impl Model {
    pub fn architecture(&self) -> anyhow::Result<llm::ModelArchitecture> {
        Ok(self.architecture.parse()?)
    }
}
impl Default for Model {
    fn default() -> Self {
        Self {
            path: "models/7B/ggml-alpaca-q4_0.bin".into(),
            context_token_length: 2048,
            architecture: llm::ModelArchitecture::Llama.to_string(),
            prefer_mmap: true,
            use_gpu: true,
        }
    }
}
