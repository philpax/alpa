use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Args {
    pub width: u32,
    pub height: u32,
}

pub(super) async fn main(args: &str) -> anyhow::Result<()> {
    use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
    use egui_winit_platform::{Platform, PlatformDescriptor};
    use winit::{event::Event, event_loop::ControlFlow};

    let args: Args = serde_json::from_str(args)?;

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
    let mut platform = Platform::new(PlatformDescriptor {
        physical_width: args.width,
        physical_height: args.height,
        scale_factor: window.scale_factor(),
        ..Default::default()
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
