use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::keycode::Keycode;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InputMethod {
    #[serde(rename = "single-line-ui")]
    SingleLineUi,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PromptMode {
    #[serde(rename = "autocomplete")]
    Autocomplete,
    #[serde(rename = "prompt")]
    Prompt(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub keys: HashSet<Keycode>,
    pub input: InputMethod,
    pub mode: PromptMode,
}

impl Command {
    pub fn new(
        keys: impl IntoIterator<Item = Keycode>,
        input: InputMethod,
        mode: PromptMode,
    ) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            input,
            mode,
        }
    }

    pub fn is_pressed(&self, keycodes: &HashSet<Keycode>) -> bool {
        keycodes.is_superset(&self.keys)
    }
}
