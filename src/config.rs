use directories::ProjectDirs;
use mlua::LuaSerdeExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct General {
    pub hotkey: Vec<String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            hotkey: vec!["LAlt".to_string(), "Backspace".to_string()],
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

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("org", "philpax", "alpa").expect("couldn't get project dir")
}

pub fn get(lua: &mlua::Lua) -> anyhow::Result<Config> {
    let project_dir = project_dirs();
    let config_dir = project_dir.config_dir();

    fs::create_dir_all(config_dir).expect("couldn't create config dir");

    let config_path = config_dir.join("config.lua");
    if !config_path.exists() {
        File::create(&config_path).expect("couldn't create config");
    }

    // read config
    let config: Config = lua.from_value(
        lua.load(&std::fs::read_to_string(&config_path)?)
            .set_name(config_path.to_string_lossy())?
            .eval()?,
    )?;

    Ok(config)
}
