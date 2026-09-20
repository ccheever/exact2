//! Native window and input only. The renderer remains usable from wasm with caller-owned devices.
use super::{Options, Result, prepare, readback};
#[path = "telemetry.rs"]
mod telemetry;
use clod_format::Reader;
use clod_view::{
    Mode, Renderer, View,
    scene::{Camera, Scene},
    wgpu,
};
use glam::{Quat, Vec3};
use serde_json::json;
use std::{sync::Arc, time::Instant};
use telemetry::{Counters, retain};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub fn run(options: Options) -> Result<()> {
    let bytes = std::fs::read(&options.file).map_err(|e| e.to_string())?;
    let reader = Reader::new(&bytes).map_err(|e| e.to_string())?;
    let scene = Scene::layout(&reader, &options.layout)?;
    let baseline = prepare::baseline(&reader);
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        options,
        bytes,
        baseline,
        scene,
        state: None,
        failure: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(e) = app.failure {
        return Err(e);
    }
    Ok(())
}
struct App {
    options: Options,
    bytes: Vec<u8>,
    baseline: Vec<clod_view::Baseline>,
    scene: Scene,
    state: Option<State>,
    failure: Option<String>,
}
struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    cluster: Renderer,
    naive: Renderer,
    blit: wgpu::util::TextureBlitter,
    refresh_hz: f64,
    paused: bool,
    dragging: bool,
    cursor: Option<(f64, f64)>,
    orbit: [f32; 2],
    t: f32,
    direction: f32,
    last_tick: Instant,
    last_present: Option<Instant>,
    intervals: Vec<f64>,
    gpu: Vec<[f64; 4]>,
    cpu: Vec<f64>,
    frames: u32,
    warmup: u32,
    last_title: Instant,
    reported: bool,
    timeouts: u32,
    counters: Counters,
    hud_counters: Counters,
}
impl App {
    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Cluster LOD · loading")
                        .with_inner_size(winit::dpi::PhysicalSize::new(
                            self.options.width,
                            self.options.height,
                        )),
                )
                .map_err(|e| e.to_string())?,
        );
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(
            adapter.request_device(&clod_view::device_descriptor(self.options.timing())),
        )
        .map_err(|e| e.to_string())?;
        let size = window.inner_size();
        self.options.width = size.width;
        self.options.height = size.height;
        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or("no surface configuration")?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .ok_or("no sRGB surface format")?;
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        let reader = Reader::new(&self.bytes).map_err(|e| e.to_string())?;
        let mut cluster = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            None,
            &self.scene,
            [size.width, size.height],
            Mode::Cluster,
        )?;
        cluster.enable_gpu_selection(&reader, self.options.capacity)?;
        let mut naive = Renderer::new(
            device.clone(),
            queue,
            &reader,
            Some(&self.baseline),
            &self.scene,
            [size.width, size.height],
            Mode::Naive,
        )?;
        for r in [&mut cluster, &mut naive] {
            r.shadows = self.options.shadows;
            r.ao = self.options.ao;
            r.culling = self.options.cull;
        }
        let refresh_hz = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|v| *v > 0)
            .ok_or("display refresh rate unavailable; pacing cannot be measured")?
            as f64
            / 1000.0;
        println!(
            "{}",
            json!({"command":"demo_start","adapter":adapter.get_info().name,"refresh_hz":refresh_hz,"present_mode":"Fifo","size":[size.width,size.height],"layout":self.options.layout,"mode":format!("{:?}",self.options.mode),"path_sampling":if self.options.exit {"uniform full path by frame"} else {"wall clock ping-pong"},"warmup_frames":10})
        );
        let blit = wgpu::util::TextureBlitter::new(&device, config.format);
        let counters = Counters::new(&device, self.options.frames);
        let hud_counters = Counters::new(&device, 1);
        self.state = Some(State {
            counters,
            hud_counters,
            window,
            surface,
            config,
            cluster,
            naive,
            blit,
            refresh_hz,
            paused: false,
            dragging: false,
            cursor: None,
            orbit: [0.0; 2],
            t: self.options.times[0],
            direction: 1.0,
            last_tick: Instant::now(),
            last_present: None,
            intervals: Vec::new(),
            gpu: Vec::new(),
            cpu: Vec::new(),
            frames: 0,
            warmup: 10,
            last_title: Instant::now(),
            reported: false,
            timeouts: 0,
        });
        self.state.as_ref().unwrap().window.request_redraw();
        Ok(())
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let s = self.state.as_mut().ok_or("window missing")?;
        if s.config.width == 0 || s.config.height == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let delta = now.duration_since(s.last_tick).as_secs_f32();
        s.last_tick = now;
        if self.options.exit {
            s.t = s.frames as f32 / self.options.frames.saturating_sub(1).max(1) as f32;
        } else if !s.paused {
            s.t += delta / self.options.seconds * s.direction;
            if s.t >= 1.0 {
                s.t = 1.0;
                s.direction = -1.0;
            }
            if s.t <= 0.0 {
                s.t = 0.0;
                s.direction = 1.0;
            }
        }
        let t = s.t;
        let scene = &mut self.scene;
        scene.fit_shadow(t);
        let mut camera = self.options.camera(scene, t);
        if s.paused && s.orbit != [0.0; 2] {
            let forward = camera
                .matrix
                .inverse()
                .project_point3(Vec3::new(0.0, 0.0, 0.5))
                - camera.eye;
            let target =
                camera.eye + forward.normalize() * camera.eye.distance(scene.hero).max(0.5);
            let direction = camera.eye - target;
            let rotated = Quat::from_rotation_z(s.orbit[0])
                * Quat::from_axis_angle(direction.cross(Vec3::Z).normalize(), s.orbit[1])
                * direction;
            camera = Camera::perspective(
                target + rotated,
                target,
                self.options.width as f32 / self.options.height as f32,
                self.options.fov,
                0.002,
                scene.radius * 12.0 + 100.0,
            );
        }
        let output = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                s.timeouts += 1;
                s.window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&s.cluster.device, &s.config);
                s.window.request_redraw();
                return Ok(());
            }
            other => return Err(format!("surface acquisition: {other:?}")),
        };
        let start = Instant::now();
        let renderer = if self.options.mode == Mode::Cluster {
            &mut s.cluster
        } else {
            &mut s.naive
        };
        let frame = renderer.render_present(
            scene,
            &camera,
            self.options.view,
            self.options.thresholds[0],
        )?;
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        s.blit.copy(
            &renderer.device,
            &mut encoder,
            &renderer.color_view(),
            &output.texture.create_view(&Default::default()),
        );
        if self.options.exit && s.warmup == 0 {
            s.counters.encode(renderer, &mut encoder, s.frames);
        }
        s.hud_counters.collect(&renderer.device, false, 1)?;
        let hud_copy = !s.hud_counters.pending();
        if hud_copy {
            s.hud_counters.encode(renderer, &mut encoder, 0);
        }
        renderer.queue.submit([encoder.finish()]);
        if hud_copy {
            s.hud_counters.map();
        }
        let cpu = start.elapsed().as_secs_f64() * 1000.0;
        s.window.pre_present_notify();
        renderer.queue.present(output);
        let presented = Instant::now();
        let times = if self.options.timing() {
            // Instrumented mode resolves only after Metal fragment samples complete.
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            readback::timestamps(renderer, &frame)?
                .ok_or("no GPU timestamps")
                .map(Some)?
        } else {
            None
        };
        if s.warmup > 0 {
            s.warmup -= 1;
            s.last_present = Some(presented);
        } else {
            if let Some(last) = s.last_present {
                retain(
                    &mut s.intervals,
                    presented.duration_since(last).as_secs_f64() * 1000.0,
                );
            }
            s.last_present = Some(presented);
            if let Some(times) = times {
                retain(&mut s.gpu, times);
            }
            retain(&mut s.cpu, cpu);
            s.frames += 1;
        }
        if s.last_title.elapsed().as_secs_f32() > 0.25 {
            let timing = times
                .map(|t| {
                    format!(
                        "GPU {:.2} ms (select {:.2})",
                        t.iter().sum::<f64>(),
                        t[2] + t[3]
                    )
                })
                .unwrap_or_else(|| "GPU timing off".into());
            s.window.set_title(&format!("Cluster LOD | {:?} {:?} | {} | t {:.3} | {:.2} px | {} | CPU {:.2} ms | overflow main/shadow {}/{} | {:.0} Hz{}",self.options.mode,self.options.view,self.options.layout,t,self.options.thresholds[0],timing,cpu,s.hud_counters.latest[0],s.hud_counters.latest[1],s.refresh_hz,if s.paused {" | PAUSED"}else{""}));
            s.last_title = Instant::now();
        }
        if self.options.exit && s.frames >= self.options.frames {
            s.counters.map();
            s.counters.collect(&renderer.device, true, s.frames)?;
            let overflow = s.counters.totals;
            self.report();
            event_loop.exit();
            if overflow != [0; 2] {
                return Err(format!(
                    "demo dropped geometry: main={} shadow={}",
                    overflow[0], overflow[1]
                ));
            }
        } else {
            s.window.request_redraw();
        }
        Ok(())
    }
    fn report(&mut self) {
        if let Some(s) = self.state.as_mut() {
            if s.reported {
                return;
            }
            s.reported = true;
            let period = 1000.0 / s.refresh_hz;
            let dropped: u64 = s
                .intervals
                .iter()
                .map(|ms| ((ms / period + 0.5).floor() as u64).saturating_sub(1))
                .sum();
            let late = s.intervals.iter().filter(|ms| **ms > period * 1.5).count();
            let gpu: Vec<_> = s.gpu.iter().map(|t| t.iter().sum()).collect();
            let select: Vec<_> = s.gpu.iter().map(|t| t[2] + t[3]).collect();
            println!(
                "{}",
                json!({"command":"demo","instrumented":self.options.timing(),"overflow":s.counters.totals[0],"shadow_overflow":s.counters.totals[1],"overflow_frames_checked":s.counters.samples,"retained_sample_limit":1000,"asset":self.options.file.file_stem().map(|s|s.to_string_lossy()),"mode":format!("{:?}",self.options.mode).to_lowercase(),"layout":self.options.layout,"size":[self.options.width,self.options.height],"frames":s.frames,"intervals":s.intervals.len(),"refresh_hz":s.refresh_hz,"refresh_period_ms":period,"frame_interval_ms":distribution(&s.intervals),"dropped_frames":dropped,"late_intervals":late,"surface_timeouts":s.timeouts,"gpu_ms":distribution(&gpu),"gpu_select_ms":distribution(&select),"cpu_ms":distribution(&s.cpu),"pixel_readbacks":0,"png_encodes":0})
            );
        }
    }
    fn layout(&mut self, name: &str) -> Result<()> {
        let reader = Reader::new(&self.bytes).map_err(|e| e.to_string())?;
        self.scene = Scene::layout(&reader, name)?;
        self.options.layout = name.into();
        let s = self.state.as_mut().ok_or("window missing")?;
        let mut cluster = Renderer::new(
            s.cluster.device.clone(),
            s.cluster.queue.clone(),
            &reader,
            None,
            &self.scene,
            [self.options.width, self.options.height],
            Mode::Cluster,
        )?;
        cluster.enable_gpu_selection(&reader, self.options.capacity)?;
        let naive = Renderer::new(
            s.naive.device.clone(),
            s.naive.queue.clone(),
            &reader,
            Some(&self.baseline),
            &self.scene,
            [self.options.width, self.options.height],
            Mode::Naive,
        )?;
        s.cluster = cluster;
        s.naive = naive;
        for r in [&mut s.cluster, &mut s.naive] {
            r.shadows = self.options.shadows;
            r.ao = self.options.ao;
            r.culling = self.options.cull;
        }
        s.t = 0.0;
        s.orbit = [0.0; 2];
        s.last_tick = Instant::now();
        Ok(())
    }
    fn event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<()> {
        let Some(s) = self.state.as_mut() else {
            return Ok(());
        };
        match event {
            WindowEvent::CloseRequested => {
                self.report();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.frame(event_loop)?,
            WindowEvent::Resized(size) => {
                s.config.width = size.width;
                s.config.height = size.height;
                if size.width > 0 && size.height > 0 {
                    self.options.width = size.width;
                    self.options.height = size.height;
                    s.cluster.resize(size.width, size.height)?;
                    s.naive.resize(size.width, size.height)?;
                    s.surface.configure(&s.cluster.device, &s.config);
                    s.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => s.dragging = state == ElementState::Pressed,
            WindowEvent::CursorMoved { position, .. } => {
                if s.paused
                    && s.dragging
                    && let Some((x, y)) = s.cursor
                {
                    s.orbit[0] -= (position.x - x) as f32 * 0.005;
                    s.orbit[1] = (s.orbit[1] + (position.y - y) as f32 * 0.005).clamp(-1.2, 1.2);
                }
                s.cursor = Some((position.x, position.y));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let PhysicalKey::Code(key) = event.physical_key else {
                    return Ok(());
                };
                match key {
                    KeyCode::Escape => {
                        self.report();
                        event_loop.exit();
                    }
                    KeyCode::Space => {
                        s.paused = !s.paused;
                        s.orbit = [0.0; 2];
                    }
                    KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                        s.paused = true;
                        s.t = (s.t
                            + if key == KeyCode::ArrowRight {
                                0.005
                            } else {
                                -0.005
                            })
                        .clamp(0.0, 1.0);
                        s.orbit = [0.0; 2];
                    }
                    KeyCode::KeyC | KeyCode::KeyD | KeyCode::KeyT => {
                        let view = match key {
                            KeyCode::KeyC => View::Clusters,
                            KeyCode::KeyD => View::Depth,
                            _ => View::Triangles,
                        };
                        self.options.view = if self.options.view == view {
                            View::Lit
                        } else {
                            view
                        };
                        self.options.mode = Mode::Cluster;
                    }
                    KeyCode::KeyN => {
                        self.options.mode = if self.options.mode == Mode::Naive {
                            Mode::Cluster
                        } else {
                            Mode::Naive
                        };
                        self.options.view = View::Lit;
                    }
                    KeyCode::BracketLeft => {
                        self.options.thresholds[0] = (self.options.thresholds[0] / 2.0).max(0.125)
                    }
                    KeyCode::BracketRight => {
                        self.options.thresholds[0] = (self.options.thresholds[0] * 2.0).min(16.0)
                    }
                    KeyCode::Digit1 => self.layout("single")?,
                    KeyCode::Digit2 => self.layout("ring:12")?,
                    KeyCode::Digit3 => self.layout("avenue:25")?,
                    KeyCode::Digit4 => self.layout("grid:400")?,
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none()
            && let Err(e) = self.initialize(event_loop)
        {
            self.failure = Some(e);
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Err(e) = self.event(event_loop, event) {
            self.failure = Some(e);
            event_loop.exit();
        }
    }
}
fn distribution(values: &[f64]) -> serde_json::Value {
    if values.is_empty() {
        return json!(null);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        sorted[((sorted.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(sorted.len() - 1)]
    };
    json!({"p50":percentile(0.5),"p95":percentile(0.95),"p99":percentile(0.99),"max":sorted.last()})
}
