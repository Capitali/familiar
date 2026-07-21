//! `crates/hologram` — the wgpu/egui holographic Glass (ADR-0007).
//!
//! Minimal boilerplate: a winit window drives a wgpu surface; egui renders the
//! actual interface to an offscreen texture; a WGSL composite pass applies the
//! holographic effects (see `hologram.wgsl`) over that texture before presenting.

mod client;
mod glass;

use std::sync::Arc;
use std::time::Instant;

use egui_wgpu::wgpu;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const HOLOGRAM_SHADER: &str = include_str!("hologram.wgsl");

/// Backend selection (spec §1 / ADR-0007): Apple platforms route explicitly to Metal
/// to use Apple Silicon's unified memory and low-overhead command queues; every other
/// platform falls back through wgpu's own Vulkan/DX12/GL selection.
fn select_backends() -> wgpu::Backends {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::PRIMARY | wgpu::Backends::GL
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct HologramUniforms {
    time: f32,
    width: f32,
    height: f32,
    /// 0 = calm ambient motion; 1 = the familiar wants the human (question pending / alarm).
    /// Drives the aberration/glitch spike so attention reads as *punctuation* (brief §8).
    attention: f32,
}

/// GPU + egui state that only exists once a window (and thus a surface) exists.
struct Graphics {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // egui renders the actual UI content into this offscreen texture; the composite
    // pipeline then samples it while applying the holographic effects.
    ui_texture_view: wgpu::TextureView,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl Graphics {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: select_backends(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("create wgpu surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no suitable wgpu adapter (backend selection above may be too narrow)");

        // Downlevel-safe limits (spec §4), widened only for the max texture dimension
        // the adapter actually supports — no compute-pipeline features needed here.
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("familiar-hologram device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("request wgpu device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let ui_texture_view = Self::make_ui_texture(
            &device,
            surface_format,
            surface_config.width,
            surface_config.height,
        );

        let egui_ctx = egui::Context::default();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hologram uniforms"),
            size: std::mem::size_of::<HologramUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hologram ui sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (composite_pipeline, composite_bind_group_layout) =
            Self::make_composite_pipeline(&device, surface_format);

        Self {
            window,
            device,
            queue,
            surface,
            surface_config,
            ui_texture_view,
            egui_ctx,
            egui_winit,
            egui_renderer,
            composite_pipeline,
            composite_bind_group_layout,
            uniform_buffer,
            sampler,
        }
    }

    fn make_ui_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui ui render target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Alpha blending is explicit here (spec §4): holographic panels stack light
    /// rather than occlude, so the composite target blends translucently.
    fn make_composite_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hologram composite shader"),
            source: wgpu::ShaderSource::Wgsl(HOLOGRAM_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hologram bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hologram pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hologram composite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (pipeline, bind_group_layout)
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.ui_texture_view =
            Self::make_ui_texture(&self.device, self.surface_config.format, width, height);
    }

    fn render(
        &mut self,
        time_secs: f32,
        attention: f32,
        glass_state: &mut glass::GlassState,
        shared: &std::sync::Mutex<client::Shared>,
    ) -> Option<String> {
        // 1. Run egui, producing the actual UI content (T1-T5 from the design brief).
        let raw_input = self.egui_winit.take_egui_input(&self.window);
        let mut submitted = None;
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let g = shared.lock().unwrap_or_else(|p| p.into_inner());
            let stale = g.fetched_at.map(|t| t.elapsed().as_secs());
            submitted = glass::draw(
                ctx,
                glass_state,
                g.view.as_ref(),
                g.error.as_deref(),
                stale,
            );
        });
        self.egui_winit
            .handle_platform_output(&self.window, full_output.platform_output);

        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hologram frame encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &tris,
            &screen_descriptor,
        );

        // 2. Render the egui UI into the offscreen texture (not the surface directly).
        {
            let mut ui_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui ui pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.ui_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.egui_renderer
                .render(&mut ui_pass, &tris, &screen_descriptor);
        }
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        // 3. Uniform upkeep (spec §4): `time` advances every frame from a real clock
        // so the holographic animation never freezes.
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&HologramUniforms {
                time: time_secs,
                width: self.surface_config.width as f32,
                height: self.surface_config.height as f32,
                attention,
            }),
        );

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hologram bind group"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.ui_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // 4. Composite pass: sample the UI texture through the holographic WGSL pass
        // and present straight to the surface.
        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return submitted;
            }
            Err(_) => return submitted,
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hologram composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            composite_pass.set_pipeline(&self.composite_pipeline);
            composite_pass.set_bind_group(0, &bind_group, &[]);
            composite_pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
        submitted
    }
}

struct App {
    graphics: Option<Graphics>,
    start_time: Instant,
    client: client::Client,
    glass: glass::GlassState,
    /// Smoothed toward `glass.attention_target` each frame, so the spike eases rather than pops.
    attention: f32,
    last_frame: Instant,
}

impl App {
    fn new(port: u16) -> Self {
        Self {
            graphics: None,
            start_time: Instant::now(),
            client: client::Client::start(port),
            glass: glass::GlassState::default(),
            attention: 0.0,
            last_frame: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("the familiar — the Glass");
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create winit window"),
        );
        self.graphics = Some(Graphics::new(window));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        if graphics.window.id() != window_id {
            return;
        }

        let response = graphics
            .egui_winit
            .on_window_event(&graphics.window, &event);
        if response.consumed {
            graphics.window.request_redraw();
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => graphics.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let t = self.start_time.elapsed().as_secs_f32();
                let dt = self.last_frame.elapsed().as_secs_f32().min(0.1);
                self.last_frame = Instant::now();

                // A *new* question also cues the platform (dock bounce / taskbar flash):
                // presence + need → let the human know, once, without nagging (brief §2/§10).
                let question = {
                    let g = self.client.shared.lock().unwrap_or_else(|p| p.into_inner());
                    g.view.as_ref().map(|v| v.question.trim().to_string()).unwrap_or_default()
                };
                if !question.is_empty() && question != self.glass.cued_question {
                    self.glass.cued_question = question;
                    graphics
                        .window
                        .request_user_attention(Some(winit::window::UserAttentionType::Informational));
                } else if question.is_empty() {
                    self.glass.cued_question.clear();
                }

                self.attention += (self.glass.attention_target - self.attention) * (dt * 4.0).min(1.0);
                if let Some(text) =
                    graphics.render(t, self.attention, &mut self.glass, &self.client.shared)
                {
                    self.client.answer(&text);
                }
                if let Some((gate, open)) = self.glass.pending_gate.take() {
                    self.client.set_gate(&gate, open);
                }
                // Continuous rendering loop (spec §4): always request the next frame so
                // the holographic effects keep animating even when nothing else changed.
                graphics.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    // `hologram [--port N]` — the daemon's gossip/console port on this machine (default 47100).
    let mut port = 47_100u16;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--port") {
        if let Some(p) = args.get(i + 1).and_then(|s| s.parse().ok()) {
            port = p;
        }
    }
    let event_loop = EventLoop::new().expect("create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(port);
    event_loop.run_app(&mut app).expect("run winit event loop");
}
