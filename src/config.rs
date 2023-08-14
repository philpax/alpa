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
