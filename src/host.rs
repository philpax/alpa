use crate::{config, util, window};
use anyhow::Context;
use device_query::Keycode;
use enigo::{Enigo, KeyboardControllable};
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{Arc, Mutex},
};

pub(super) async fn main() -> anyhow::Result<()> {
    use config::Config;
    use directories::ProjectDirs;
    use mlua::LuaSerdeExt;

    let enigo = Arc::new(Mutex::new(Enigo::new()));

    let lua = mlua::Lua::new();

    let config = lua.create_table()?;
    lua.globals().set("config", config)?;

    let internal = lua.create_table()?;
    lua.globals().set("internal", internal)?;

    // Run the prelude.
    lua.load(include_str!("prelude.lua"))
        .set_name("prelude")?
        .eval()?;

    // Run the config.
    let config_dir = ProjectDirs::from("org", "philpax", "alpa")
        .context("couldn't get project dir")?
        .config_dir()
        .to_owned();
    std::fs::create_dir_all(&config_dir).context("couldn't create config dir")?;

    let config_path = config_dir.join("config.lua");
    if !config_path.exists() {
        std::fs::write(&config_path, include_str!("../resources/config.lua"))?;
    }

    lua.load(&std::fs::read_to_string(&config_path)?)
        .set_name(config_path.to_string_lossy())?
        .eval::<()>()?;

    let config: mlua::Table = lua.globals().get("config")?;

    let hotkeys_to_listen_for = find_registered_hotkeys(vec![], config.get("hotkeys")?)?
        .into_iter()
        .collect::<HashSet<_>>();

    let config: Config = lua.from_value_with(
        mlua::Value::Table(config),
        mlua::DeserializeOptions::new().deny_unsupported_types(false),
    )?;

    let model = llm::load_dynamic(
        config
            .model
            .architecture()
            .expect("invalid model architecture specified in config"),
        &config.model.path,
        llm::ModelParameters {
            prefer_mmap: config.model.prefer_mmap,
            n_context_tokens: config.model.context_token_length,
            ..Default::default()
        },
        llm::load_progress_callback_stdout,
    )?;

    let ui = lua.create_table()?;
    ui.set(
        "singleline",
        lua.create_function(move |_lua, func: mlua::Function| {
            let output = std::process::Command::new(std::env::current_exe()?)
                .arg(
                    serde_json::to_string(&window::Args {
                        width: config.window.width,
                        height: config.window.height,
                        style: config.style.clone(),
                    })
                    .map_err(|e| mlua::Error::external(e))?,
                )
                .output()?;

            let () = func
                .call((String::from_utf8(output.stdout).map_err(|e| mlua::Error::external(e))?,))?;
            Ok(())
        })?,
    )?;
    lua.globals().set("ui", ui)?;

    let input = lua.create_table()?;
    input.set(
        "key_sequence",
        lua.create_function({
            let enigo = enigo.clone();
            move |_, sequence: String| {
                let mut enigo = enigo.lock().unwrap();
                enigo.key_sequence(&sequence);
                Ok(())
            }
        })?,
    )?;
    lua.globals().set("input", input)?;

    let llm = lua.create_table()?;
    llm.set(
        "infer",
        lua.create_function(move |_, (prompt, callback): (String, mlua::Function)| {
            let mut session = model.start_session(Default::default());
            session
                .infer(
                    model.as_ref(),
                    &prompt,
                    &mut Default::default(),
                    &mut rand::thread_rng(),
                    |tok| {
                        callback.call((tok.to_string(),))?;
                        Ok::<_, mlua::Error>(())
                    },
                )
                .map_err(|e| mlua::Error::external(e.to_string()))?;

            Ok(())
        })?,
    )?;
    lua.globals().set("llm", llm)?;

    let device_state = device_query::DeviceState::new();
    let mut old_keycodes = HashSet::new();
    loop {
        let new_keycodes: HashSet<_> = hotkeys_to_listen_for
            .iter()
            .filter(|kcs| util::is_hotkey_pressed(&device_state, kcs))
            .cloned()
            .collect();

        for keycodes in new_keycodes.difference(&old_keycodes) {
            let () = lua
                .globals()
                .get::<_, mlua::Table>("internal")
                .unwrap()
                .get::<_, mlua::Function>("dispatch")
                .unwrap()
                .call((keycodes
                    .iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<String>>(),))
                .unwrap();
        }
        old_keycodes = new_keycodes;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn find_registered_hotkeys(
    prefix: Vec<Keycode>,
    table: mlua::Table,
) -> anyhow::Result<Vec<Vec<Keycode>>> {
    let mut output = vec![];
    for kv_result in table.pairs::<String, mlua::Value>() {
        let (k, v) = kv_result?;

        let mut prefix = prefix.clone();
        prefix.push(
            Keycode::from_str(&k)
                .map_err(|e| anyhow::anyhow!("failed to parse keycode {k} ({e})"))?,
        );
        match v {
            mlua::Value::Table(v) => {
                output.append(&mut find_registered_hotkeys(prefix, v)?);
            }
            mlua::Value::Function(_) => output.push(prefix),
            _ => anyhow::bail!("unexpected type for {v:?} at {k}"),
        }
    }

    Ok(output)
}
