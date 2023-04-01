use std::{collections::HashSet, path::PathBuf, str::FromStr};

use anyhow::Context;
use config::Config;
use device_query::Keycode;
use directories::ProjectDirs;
use egui::{FontData, FontDefinitions, FontFamily};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use mlua::LuaSerdeExt;
use winit::{event::Event, event_loop::ControlFlow};

mod config;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    std::fs::create_dir_all(&config_dir).context("couldn't create config dir")?;

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

    let event_loop = {
        let mut builder = winit::event_loop::EventLoopBuilder::<WinitEvent>::with_user_event();

        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder
                .with_default_menu(false)
                .with_activate_ignoring_other_apps(false);
        }

        builder.build()
    };

    let window = winit::window::WindowBuilder::new()
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(true)
        .with_title("alpa")
        .with_visible(false)
        .with_active(false)
        .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
        .with_inner_size(winit::dpi::PhysicalSize {
            width: config.window.width,
            height: config.window.height,
        })
        .build(&event_loop)?;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: Default::default(),
    });
    let surface = unsafe { instance.create_surface(&window) }?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
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

    let size = window.inner_size();
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
    let mut platform = Platform::new(PlatformDescriptor {
        physical_width: size.width,
        physical_height: size.height,
        scale_factor: window.scale_factor(),
        font_definitions: FontDefinitions::default(),
        style: Default::default(),
    });

    set_style(&platform.context(), &config)?;

    let (events_tx, hotkeys_rx) = std::sync::mpsc::channel();
    let _hotkey_thread = std::thread::spawn({
        let ctx = platform.context();
        move || {
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
        }
    });

    // We use the egui_wgpu_backend crate as the render backend.
    let mut egui_rpass = RenderPass::new(&device, surface_format, 1);

    let start_time = std::time::Instant::now();
    let mut input = String::new();
    event_loop.run(move |event, _, control_flow| {
        // Pass the winit events to the platform integration.
        platform.handle_event(&event);

        match event {
            Event::RedrawRequested(..) => {
                platform.update_time(start_time.elapsed().as_secs_f64());

                let events: Vec<_> = hotkeys_rx.try_iter().collect();
                for event in events {
                    let () = lua
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

                // Draw the demo application.
                draw_ctx(&platform.context(), &mut input);

                // End the UI frame. We could now handle the output and draw the UI with the backend.
                let full_output = platform.end_frame(Some(&window));
                let paint_jobs = platform.context().tessellate(full_output.shapes);

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

struct EguiRepaintSinal(std::sync::Mutex<winit::event_loop::EventLoopProxy<WinitEvent>>);
impl epi::backend::RepaintSignal for EguiRepaintSinal {
    fn request_repaint(&self) {
        self.0
            .lock()
            .unwrap()
            .send_event(WinitEvent::RequestRedraw)
            .ok();
    }
}

fn set_style(ctx: &egui::Context, config: &Config) -> anyhow::Result<()> {
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

    ctx.set_visuals(visuals);

    // fonts
    if let Some(font_path) = &config.style.font {
        let mut fonts = FontDefinitions::default();

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

        ctx.set_fonts(fonts);
    }

    Ok(())
}

fn draw_ctx(ctx: &egui::Context, input: &mut String) {
    egui::CentralPanel::default()
        .show(ctx, |ui| {
            let input_widget = egui::TextEdit::singleline(input).lock_focus(true);
            let input_res = ui.add_sized(ui.available_size(), input_widget);

            if input_res.lost_focus() {
                println!("{:?}", *input);
            }
        })
        .inner
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

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("org", "philpax", "alpa").expect("couldn't get project dir")
}

fn config_dir() -> PathBuf {
    project_dirs().config_dir().to_owned()
}
