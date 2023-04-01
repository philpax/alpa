use std::{collections::HashSet, fs, path::PathBuf, str::FromStr};

use anyhow::Context;
use config::Config;
use device_query::Keycode;
use directories::ProjectDirs;
use eframe::egui;
use egui::{FontData, FontDefinitions, FontFamily, Key, Pos2, Vec2};
use mlua::LuaSerdeExt;
use std::sync;

mod config;
mod util;

fn main() -> anyhow::Result<()> {
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
    let config_dir = config_dir();
    fs::create_dir_all(&config_dir).context("couldn't create config dir")?;

    let config_path = config_dir.join("config.lua");
    if !config_path.exists() {
        std::fs::write(&config_path, include_str!("../resources/config.lua"))?;
    }

    lua.load(&std::fs::read_to_string(&config_path)?)
        .set_name(config_path.to_string_lossy())?
        .eval::<()>()?;

    let config_table: mlua::Table = lua.globals().get("config")?;

    let hotkeys_to_listen_for = find_registered_hotkeys(vec![], config_table.get("hotkeys")?)?
        .into_iter()
        .collect::<HashSet<_>>();

    let config: Config = lua.from_value_with(
        mlua::Value::Table(config_table),
        mlua::DeserializeOptions::new().deny_unsupported_types(false),
    )?;

    eframe::run_native(
        "alpa",
        eframe::NativeOptions {
            transparent: true,
            resizable: false,
            always_on_top: true,
            decorated: false,
            initial_window_size: Some(Vec2 {
                x: config.window.width as f32,
                y: config.window.height as f32,
            }),
            initial_window_pos: Some(Pos2::ZERO),
            ..eframe::NativeOptions::default()
        },
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();

            // colors
            if let Some(bg_color) = &config.style.bg_color {
                let bg_color = util::hex_to_color32(bg_color);
                visuals.widgets.noninteractive.bg_fill = bg_color;
            }

            if let Some(input_bg_color) = &config.style.input_bg_color {
                let input_bg_color = util::hex_to_color32(input_bg_color);
                visuals.extreme_bg_color = input_bg_color;
            }

            if let Some(hovered_bg_color) = &config.style.hovered_bg_color {
                let hovered_bg_color = util::hex_to_color32(hovered_bg_color);
                visuals.widgets.hovered.bg_fill = hovered_bg_color;
            }

            if let Some(selected_bg_color) = &config.style.selected_bg_color {
                let selected_bg_color = util::hex_to_color32(selected_bg_color);
                visuals.widgets.active.bg_fill = selected_bg_color;
            }

            if let Some(text_color) = &config.style.text_color {
                let text_color = util::hex_to_color32(text_color);
                visuals.override_text_color = Some(text_color);
            }

            if let Some(stroke_color) = &config.style.stroke_color {
                let stroke_color = util::hex_to_color32(stroke_color);
                visuals.selection.stroke.color = stroke_color; // text input
                visuals.widgets.hovered.bg_stroke.color = stroke_color; // hover
                visuals.widgets.active.bg_stroke.color = stroke_color; // selection
            }

            cc.egui_ctx.set_visuals(visuals);

            // fonts
            if let Some(font_path) = &config.style.font {
                let mut fonts = FontDefinitions::default();

                fonts.font_data.insert(
                    "custom_font".to_owned(),
                    FontData::from_owned(fs::read(font_path).unwrap()),
                );

                fonts
                    .families
                    .get_mut(&FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "custom_font".to_owned());
                fonts
                    .families
                    .get_mut(&FontFamily::Monospace)
                    .unwrap()
                    .push("custom_font".to_owned());

                cc.egui_ctx.set_fonts(fonts);
            }

            Box::new(App::new(
                cc.egui_ctx.clone(),
                config,
                lua,
                hotkeys_to_listen_for,
            ))
        }),
    )
    .unwrap();

    Ok(())
}

fn find_registered_hotkeys(
    prefix: Vec<Keycode>,
    table: mlua::Table,
) -> anyhow::Result<Vec<Vec<Keycode>>> {
    let mut output = vec![];
    for kv_result in table.pairs::<String, mlua::Value>().into_iter() {
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

pub struct AppChannels {
    hotkeys_rx: sync::mpsc::Receiver<Vec<Keycode>>,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct Opened {
    pos: egui::Pos2,
    input: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AppState {
    First,
    Unopened,
    Opened(Opened),
}

pub struct App {
    state: AppState,
    app_channels: AppChannels,
    request_focus: bool,
    lua: mlua::Lua,

    _hotkey_thread: std::thread::JoinHandle<()>,
    _config: Config,
}

impl App {
    pub fn new(
        ctx: egui::Context,
        config: Config,
        lua: mlua::Lua,
        hotkeys_to_listen_for: HashSet<Vec<Keycode>>,
    ) -> Self {
        let (events_tx, hotkeys_rx) = sync::mpsc::channel();
        let hotkey_thread = std::thread::spawn(move || {
            let device_state = device_query::DeviceState::new();

            let mut old_keycodes = HashSet::new();
            loop {
                let new_keycodes: HashSet<_> = hotkeys_to_listen_for
                    .iter()
                    .filter(|kcs| crate::util::is_hotkey_pressed(&device_state, kcs))
                    .cloned()
                    .collect();

                for keycodes in new_keycodes.difference(&old_keycodes) {
                    events_tx.send(keycodes.clone()).unwrap();
                    ctx.request_repaint();
                }

                old_keycodes = new_keycodes;

                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        Self {
            state: AppState::First,
            app_channels: AppChannels { hotkeys_rx },
            request_focus: false,
            lua,

            _hotkey_thread: hotkey_thread,
            _config: config,
        }
    }

    fn get_new_state(&mut self, ctx: &egui::Context) -> anyhow::Result<AppState> {
        match self.state.clone() {
            AppState::First => Ok(AppState::Unopened),
            AppState::Unopened => Ok(AppState::Unopened),
            AppState::Opened(opened) => self.process_opened(opened, ctx),
        }
    }

    fn process_opened(&mut self, opened: Opened, ctx: &egui::Context) -> anyhow::Result<AppState> {
        if ctx.input(|r| r.key_released(Key::Escape)) {
            return Ok(AppState::Unopened);
        }

        egui::CentralPanel::default()
            .show(ctx, |ui| self.draw_opened_central(ui, opened))
            .inner
    }

    fn draw_opened_central(
        &mut self,
        ui: &mut egui::Ui,
        mut opened: Opened,
    ) -> anyhow::Result<AppState> {
        let input_widget = egui::TextEdit::singleline(&mut opened.input).lock_focus(true);
        let input_res = ui.add_sized(ui.available_size(), input_widget);
        if self.request_focus {
            input_res.request_focus();
            self.request_focus = false;
        }

        if input_res.lost_focus() {
            println!("{:?}", opened.input);
            Ok(AppState::Unopened)
        } else {
            Ok(AppState::Opened(opened))
        }
    }

    fn set_state(&mut self, state: AppState, frame: &mut eframe::Frame) {
        if self.state != state {
            match &state {
                AppState::First => {}
                AppState::Unopened => {
                    frame.set_visible(false);
                }
                AppState::Opened(opened) => {
                    frame.set_window_pos(opened.pos);
                    frame.set_visible(true);
                    self.request_focus = true;
                }
            }
        }
        self.state = state;
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let events: Vec<_> = self.app_channels.hotkeys_rx.try_iter().collect();
        for event in events {
            let () = self
                .lua
                .globals()
                .get::<_, mlua::Table>("internal")
                .unwrap()
                .get::<_, mlua::Function>("dispatch")
                .unwrap()
                .call((event
                    .into_iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<String>>(),))
                .unwrap();
        }

        let state = self.get_new_state(ctx);
        self.set_state(state.unwrap(), frame);
    }
}

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("org", "philpax", "alpa").expect("couldn't get project dir")
}

fn config_dir() -> PathBuf {
    project_dirs().config_dir().to_owned()
}
