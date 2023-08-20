use crate::{
    command::{InputMethod, PromptMode},
    config::{self, Config},
    keycode::Keycode,
    window,
};
use device_query::DeviceQuery;
use enigo::{Enigo, KeyboardControllable};
use std::{
    collections::HashSet,
    convert::Infallible,
    env, process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
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

    let (command_tx, command_rx) = flume::bounded(1);
    let is_generating = Arc::new(AtomicBool::new(false));

    let _input_thread = std::thread::spawn({
        let is_generating = is_generating.clone();
        move || {
            let device_state = device_query::DeviceState::new();
            let mut last_pressed = None;
            loop {
                let new_keycodes =
                    HashSet::from_iter(device_state.get_keys().into_iter().map(Keycode::from));

                if !is_generating.load(Ordering::SeqCst) {
                    last_pressed = None;
                }

                for command in &config.commands {
                    if last_pressed != Some(command) && command.is_pressed(&new_keycodes) {
                        command_tx.send(command).unwrap();
                        last_pressed = Some(command);
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    });

    while let Ok(command) = command_rx.recv() {
        is_generating.store(true, Ordering::SeqCst);

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

        let enigo = enigo.clone();
        model.start_session(Default::default()).infer(
            model.as_ref(),
            &mut rand::thread_rng(),
            &llm::InferenceRequest {
                prompt: (&new_prompt).into(),
                // TODO: expose sampler
                parameters: &llm::InferenceParameters::default(),
                play_back_previous_tokens: false,
                maximum_token_count: None,
            },
            &mut Default::default(),
            move |tok| {
                if let llm::InferenceResponse::InferredToken(t) = tok {
                    enigo.lock().unwrap().key_sequence(&t);
                }
                Ok::<_, Infallible>(llm::InferenceFeedback::Continue)
            },
        )?;

        is_generating.store(false, Ordering::SeqCst);
    }

    Ok(())
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
