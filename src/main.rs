use serde::{Deserialize, Serialize};

mod config;
mod util;

#[derive(Serialize, Deserialize)]
struct WindowArgs {
    width: u32,
    height: u32,
    style: config::Style,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Some(args) = std::env::args().nth(1) {
        let args = serde_json::from_str(&args)?;
        window::main(&args).await
    } else {
        runner::main().await
    }
}

mod runner {
    use crate::{config, util, WindowArgs};
    use anyhow::Context;
    use device_query::Keycode;
    use std::{collections::HashSet, str::FromStr};

    pub(super) async fn main() -> anyhow::Result<()> {
        use config::Config;
        use directories::ProjectDirs;
        use mlua::LuaSerdeExt;

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

        let ui = lua.create_table()?;
        ui.set(
            "singleline",
            lua.create_function(move |_lua, func: mlua::Function| {
                let output = std::process::Command::new(std::env::current_exe()?)
                    .arg(
                        serde_json::to_string(&WindowArgs {
                            width: config.window.width,
                            height: config.window.height,
                            style: config.style.clone(),
                        })
                        .map_err(|e| mlua::Error::external(e))?,
                    )
                    .output()?;

                let () = func.call((
                    String::from_utf8(output.stdout).map_err(|e| mlua::Error::external(e))?,
                ))?;
                Ok(())
            })?,
        )?;
        lua.globals().set("ui", ui)?;

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
}

mod window {
    use crate::{config, util, WindowArgs};
    use anyhow::Context;

    pub(super) async fn main(args: &WindowArgs) -> anyhow::Result<()> {
        use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
        use egui_winit_platform::{Platform, PlatformDescriptor};
        use winit::{event::Event, event_loop::ControlFlow};

        let event_loop = {
            let mut builder = winit::event_loop::EventLoopBuilder::<WinitEvent>::with_user_event();

            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                builder
                    .with_default_menu(false)
                    .with_activation_policy(ActivationPolicy::Accessory);
            }

            builder.build()
        };

        let window = winit::window::WindowBuilder::new()
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_title("alpa")
            .with_visible(true)
            .with_active(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .with_inner_size(winit::dpi::LogicalSize {
                width: args.width,
                height: args.height,
            })
            .build(&event_loop)?;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let size = window.inner_size();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("no adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.describe().srgb)
            .unwrap_or(surface_caps.formats[0]);
        let mut surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        // We use the egui_winit_platform crate as the platform.
        let (font_definitions, style) = get_style(&args.style)?;
        let mut platform = Platform::new(PlatformDescriptor {
            physical_width: args.width,
            physical_height: args.height,
            scale_factor: window.scale_factor(),
            font_definitions,
            style,
        });

        // We use the egui_wgpu_backend crate as the render backend.
        let mut egui_rpass = RenderPass::new(&device, surface_format, 1);

        let start_time = std::time::Instant::now();
        let mut input = String::new();

        event_loop.run(move |event, _window_target, control_flow| {
            // Pass the winit events to the platform integration.
            platform.handle_event(&event);

            match event {
                Event::RedrawRequested(..) => {
                    platform.update_time(start_time.elapsed().as_secs_f64());

                    let output_frame = match surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(wgpu::SurfaceError::Outdated) => {
                            // This error occurs when the app is minimized on Windows.
                            // Silently return here to prevent spamming the console with:
                            // "The underlying surface has changed, and therefore the swap chain must be updated"
                            return;
                        }
                        Err(e) => {
                            eprintln!("Dropped frame with error: {}", e);
                            return;
                        }
                    };
                    let output_view = output_frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    // Begin to draw the UI frame.
                    platform.begin_frame();

                    egui::CentralPanel::default().show(&platform.context(), |ui| {
                        let input_widget = egui::TextEdit::singleline(&mut input).lock_focus(true);
                        let input_res = ui.add_sized(ui.available_size(), input_widget);

                        input_res.request_focus();

                        ui.input(|i| {
                            if i.key_released(egui::Key::Escape) {
                                *control_flow = ControlFlow::Exit;
                            }

                            if i.key_released(egui::Key::Enter) {
                                print!("{input}");
                                *control_flow = ControlFlow::Exit;
                            }
                        });
                    });

                    // End the UI frame. We could now handle the output and draw the UI with the backend.
                    let full_output = platform.end_frame(Some(&window));
                    let paint_jobs = platform.context().tessellate(full_output.shapes);

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("encoder"),
                        });

                    // Upload all resources for the GPU.
                    let screen_descriptor = ScreenDescriptor {
                        physical_width: surface_config.width,
                        physical_height: surface_config.height,
                        scale_factor: window.scale_factor() as f32,
                    };
                    let tdelta: egui::TexturesDelta = full_output.textures_delta;
                    egui_rpass
                        .add_textures(&device, &queue, &tdelta)
                        .expect("add texture ok");
                    egui_rpass.update_buffers(&device, &queue, &paint_jobs, &screen_descriptor);

                    // Record all render passes.
                    egui_rpass
                        .execute(
                            &mut encoder,
                            &output_view,
                            &paint_jobs,
                            &screen_descriptor,
                            Some(wgpu::Color::BLACK),
                        )
                        .unwrap();
                    // Submit the commands.
                    queue.submit(std::iter::once(encoder.finish()));

                    // Redraw egui
                    output_frame.present();

                    egui_rpass
                        .remove_textures(tdelta)
                        .expect("remove texture ok");
                }
                Event::MainEventsCleared | Event::UserEvent(WinitEvent::RequestRedraw) => {
                    window.request_redraw();
                }
                Event::WindowEvent { event, .. } => match event {
                    winit::event::WindowEvent::Resized(size) => {
                        if size.width > 0 && size.height > 0 {
                            surface_config.width = size.width;
                            surface_config.height = size.height;
                            surface.configure(&device, &surface_config);
                        }
                    }
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                },
                _ => (),
            }
        });
    }

    enum WinitEvent {
        RequestRedraw,
    }

    struct EguiRepaintSignal(std::sync::Mutex<winit::event_loop::EventLoopProxy<WinitEvent>>);
    impl epi::backend::RepaintSignal for EguiRepaintSignal {
        fn request_repaint(&self) {
            self.0
                .lock()
                .unwrap()
                .send_event(WinitEvent::RequestRedraw)
                .ok();
        }
    }

    fn get_style(style: &config::Style) -> anyhow::Result<(egui::FontDefinitions, egui::Style)> {
        use egui::{FontData, FontDefinitions, FontFamily};
        let mut visuals = egui::Visuals::dark();

        // colors
        if let Some(bg_color) = &style.bg_color {
            let bg_color = util::hex_to_color32(bg_color);
            visuals.widgets.noninteractive.bg_fill = bg_color;
        }

        if let Some(input_bg_color) = &style.input_bg_color {
            let input_bg_color = util::hex_to_color32(input_bg_color);
            visuals.extreme_bg_color = input_bg_color;
        }

        if let Some(hovered_bg_color) = &style.hovered_bg_color {
            let hovered_bg_color = util::hex_to_color32(hovered_bg_color);
            visuals.widgets.hovered.bg_fill = hovered_bg_color;
        }

        if let Some(selected_bg_color) = &style.selected_bg_color {
            let selected_bg_color = util::hex_to_color32(selected_bg_color);
            visuals.widgets.active.bg_fill = selected_bg_color;
        }

        if let Some(text_color) = &style.text_color {
            let text_color = util::hex_to_color32(text_color);
            visuals.override_text_color = Some(text_color);
        }

        if let Some(stroke_color) = &style.stroke_color {
            let stroke_color = util::hex_to_color32(stroke_color);
            visuals.selection.stroke.color = stroke_color; // text input
            visuals.widgets.hovered.bg_stroke.color = stroke_color; // hover
            visuals.widgets.active.bg_stroke.color = stroke_color; // selection
        }

        // fonts
        let mut fonts = FontDefinitions::default();
        if let Some(font_path) = &style.font {
            fonts.font_data.insert(
                "custom_font".to_owned(),
                FontData::from_owned(std::fs::read(font_path)?),
            );

            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .context("no proportional font family")?
                .insert(0, "custom_font".to_owned());
            fonts
                .families
                .get_mut(&FontFamily::Monospace)
                .context("no monospace font family")?
                .push("custom_font".to_owned());
        }

        Ok((
            fonts,
            egui::Style {
                visuals,
                ..Default::default()
            },
        ))
    }
}
