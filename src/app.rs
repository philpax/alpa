use std::str::FromStr;
use std::sync;

use egui::Key;

use crate::config::Config;

#[derive(Clone, Copy, Debug)]
pub enum HotkeyEvent {
    Open(egui::Pos2),
}

pub struct AppChannels {
    hotkeys_rx: sync::mpsc::Receiver<HotkeyEvent>,
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

    _hotkey_thread: std::thread::JoinHandle<()>,
    _config: Config,
}

impl App {
    pub fn new(ctx: egui::Context, config: Config) -> Self {
        let (events_tx, hotkeys_rx) = sync::mpsc::channel();
        let hotkey_thread = std::thread::spawn({
            let config = config.clone();
            let hotkeys: Vec<_> = config
                .general
                .hotkey
                .iter()
                .map(|hk| device_query::Keycode::from_str(hk).unwrap())
                .collect();

            move || {
                let device_state = device_query::DeviceState::new();

                loop {
                    // global hotkeys
                    if crate::util::is_hotkey_pressed(&device_state, &hotkeys) {
                        let (x, y) = device_state.query_pointer().coords;
                        events_tx
                            .send(HotkeyEvent::Open(egui::Pos2::new(x as f32, y as f32)))
                            .unwrap();
                        ctx.request_repaint();
                    }

                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        });

        Self {
            state: AppState::First,
            app_channels: AppChannels { hotkeys_rx },
            request_focus: false,

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
            match event {
                HotkeyEvent::Open(pos) => self.set_state(
                    AppState::Opened(Opened {
                        pos,
                        ..Opened::default()
                    }),
                    frame,
                ),
            }
        }

        let state = self.get_new_state(ctx);
        self.set_state(state.unwrap(), frame);
    }
}
