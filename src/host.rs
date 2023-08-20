use crate::{
    command::{InputMethod, PromptMode},
    config::{self, Config},
    keycode::Keycode,
    window,
};
use anyhow::Context;
use device_query::DeviceQuery;
use directories::ProjectDirs;
use enigo::{Enigo, KeyboardControllable};
use std::{
    collections::HashSet,
    convert::Infallible,
    env, process,
    sync::{Arc, Mutex},
};

pub(super) fn main() -> anyhow::Result<()> {
    let enigo = Arc::new(Mutex::new(Enigo::new()));

    let config = config::init()?;

    let model = llm::load_dynamic(
        Some(config.model.architecture()?),
        // TODO: support others
        &config.model.path,
        llm::TokenizerSource::Embedded,
        llm::ModelParameters {
            prefer_mmap: config.model.prefer_mmap,
            context_size: config.model.context_token_length,
            use_gpu: config.model.use_gpu,
            ..Default::default()
        },
        llm::load_progress_callback_stdout,
    )?;

    let commands = config.commands.clone();
    let device_state = device_query::DeviceState::new();
    loop {
        let new_keycodes =
            HashSet::from_iter(device_state.get_keys().into_iter().map(Keycode::from));

        for command in &commands {
            if !command.is_pressed(&new_keycodes) {
                continue;
            }

            let prompt = match command.input {
                InputMethod::SingleLineUi => ask_for_singleline_input(&config)?,
            };

            if prompt.is_empty() {
                continue;
            }

            let new_prompt = match &command.mode {
                PromptMode::Autocomplete => prompt,
                PromptMode::Prompt(template) => template.replace("{{PROMPT}}", &prompt),
            };

            infer(model.as_ref(), &new_prompt, |token| {
                enigo.lock().unwrap().key_sequence(&token);
            })?;
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn ask_for_singleline_input(config: &Config) -> anyhow::Result<String> {
    let request = serde_json::to_string(&window::Args {
        width: config.window.width,
        height: config.window.height,
    })?;

    let output = process::Command::new(env::current_exe()?)
        .arg(request)
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

fn infer(
    model: &dyn llm::Model,
    prompt: &str,
    mut callback: impl FnMut(String),
) -> anyhow::Result<()> {
    model.start_session(Default::default()).infer(
        model,
        &mut rand::thread_rng(),
        &llm::InferenceRequest {
            prompt: prompt.into(),
            // TODO: expose sampler
            parameters: &llm::InferenceParameters::default(),
            play_back_previous_tokens: false,
            maximum_token_count: None,
        },
        &mut Default::default(),
        move |tok| {
            if let llm::InferenceResponse::InferredToken(t) = tok {
                callback(t);
            }
            Ok::<_, Infallible>(llm::InferenceFeedback::Continue)
        },
    )?;

    Ok(())
}
