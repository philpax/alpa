use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::keycode::Keycode;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputMethod {
    #[serde(rename = "single-line-ui")]
    SingleLineUi,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PromptMode {
    #[serde(rename = "autocomplete")]
    Autocomplete,
    #[serde(rename = "prompt")]
    Prompt(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateCommand {
    pub input: InputMethod,
    pub mode: PromptMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandType {
    #[serde(rename = "generate")]
    Generate(GenerateCommand),
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Command {
    pub keys: HashSet<Keycode>,
    #[serde(rename = "type")]
    pub ty: CommandType,
}

impl Command {
    pub fn new(keys: impl IntoIterator<Item = Keycode>, ty: CommandType) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            ty,
        }
    }

    pub fn is_pressed(&self, keycodes: &HashSet<Keycode>) -> bool {
        keycodes.is_superset(&self.keys)
    }
}
