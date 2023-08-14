use crate::{config::Config, window};
use anyhow::Context;
use device_query::{DeviceQuery, DeviceState, Keycode};
use directories::ProjectDirs;
use enigo::{Enigo, KeyboardControllable};
use std::{
    collections::HashSet,
    convert::Infallible,
    env,
    process::Command,
    sync::{Arc, Mutex},
};

pub(super) fn main() -> anyhow::Result<()> {
    let enigo = Arc::new(Mutex::new(Enigo::new()));

    // Get the config.
    let config_dir = ProjectDirs::from("org", "philpax", "alpa")
        .context("couldn't get project dir")?
        .config_dir()
        .to_owned();
    std::fs::create_dir_all(&config_dir).context("couldn't create config dir")?;

    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, toml::to_string_pretty(&Config::default())?)?;
    }

    let hotkeys_to_listen_for: HashSet<Vec<Keycode>> =
        HashSet::from_iter([vec![Keycode::LControl, Keycode::Escape]]);

    let config: Config = toml::from_str(&std::fs::read_to_string(&config_path)?)?;

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

    let device_state = device_query::DeviceState::new();
    let mut old_keycodes = HashSet::new();
    loop {
        let new_keycodes: HashSet<_> = hotkeys_to_listen_for
            .iter()
            .filter(|kcs| is_hotkey_pressed(&device_state, kcs))
            .cloned()
            .collect();

        for keycodes in new_keycodes.difference(&old_keycodes) {
            if keycodes == &vec![Keycode::LControl, Keycode::Escape] {
                let prompt = ask_for_singleline_input(&config)?;

                infer(model.as_ref(), &prompt, |output| {
                    enigo.lock().unwrap().key_sequence(&output);
                })?;
            }
        }
        old_keycodes = new_keycodes;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn ask_for_singleline_input(config: &Config) -> anyhow::Result<String> {
    let request = serde_json::to_string(&window::Args {
        width: config.window.width,
        height: config.window.height,
    })?;

    let output = Command::new(env::current_exe()?).arg(request).output()?;
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
            match tok {
                llm::InferenceResponse::InferredToken(t) => {
                    callback(t);
                }
                _ => {}
            }
            Ok::<_, Infallible>(llm::InferenceFeedback::Continue)
        },
    )?;

    Ok(())
}

pub fn is_hotkey_pressed(device_state: &DeviceState, hotkey_str: &[Keycode]) -> bool {
    HashSet::<Keycode>::from_iter(device_state.get_keys())
        .is_superset(&HashSet::from_iter(hotkey_str.iter().copied()))
}
