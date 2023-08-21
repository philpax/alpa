use std::{path::PathBuf, sync::OnceLock};

use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{
    command::{Command, CommandType, GenerateCommand, InputMethod, NewlineBehavior, PromptMode},
    keycode::Keycode,
};

static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn init() -> anyhow::Result<&'static Config> {
    assert!(CONFIG.get().is_none());

    // Get the config.
    let config_dir = ProjectDirs::from("org", "philpax", "alpa")
        .context("couldn't get project dir")?
        .config_dir()
        .to_owned();
    std::fs::create_dir_all(&config_dir).context("couldn't create config dir")?;

    let config_path = config_dir.join("config.toml");
    let config = if config_path.exists() {
        toml::from_str::<Config>(&std::fs::read_to_string(&config_path)?)?
    } else {
        Default::default()
    };
    std::fs::write(&config_path, toml::to_string_pretty(&config)?)?;

    Ok(CONFIG.get_or_init(|| config))
}

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
    vec![
        Command::new(
            [Keycode::LControl, Keycode::Escape],
            CommandType::Generate(GenerateCommand {
                input: InputMethod::SingleLineUi,
                mode: PromptMode::Prompt(
                    "SYSTEM: You are a general AI assistant.\nUSER: {{PROMPT}}\nASSISTANT: "
                        .to_string(),
                ),
                newline: NewlineBehavior::Enter,
            }),
        ),
        Command::new([Keycode::Escape], CommandType::Cancel),
    ]
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
