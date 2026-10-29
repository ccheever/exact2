use exact_ink::device::apple::{DisplayLink, Layer};
use exact_linux::{
    app::{self, Config},
    Presenter,
};
use exact_runner::DataSource;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, PhysicalKey},
    platform::macos::MonitorHandleExtMacOS,
    window::{Window, WindowId},
};

pub enum Event {
    Tick,
    Wake,
    Request(String, SyncSender<String>),
    Closed,
}

struct App<D: DataSource> {
    name: String,
    config: Config,
    proxy: EventLoopProxy<Event>,
    window: Option<Rc<Window>>,
    layer: Option<Layer>,
    display: Option<DisplayLink>,
    presenter: Option<Presenter<D>>,
    agent: Option<super::agent::Agent>,
    started: Instant,
    tick_pending: Arc<AtomicBool>,
    pointer: (f32, f32),
    buttons: u32,
    scale: f64,
    hidden: bool,
    failed: bool,
    frames: u64,
    paint_ms: f64,
    wheel_phases: [u64; 2],
}

impl<D: DataSource> Drop for App<D> {
    fn drop(&mut self) {
        self.display.take();
        self.presenter.take();
        if let Some(layer) = self.layer.take() {
            layer.update();
        }
    }
}

pub fn run<D: DataSource + Default + 'static>(name: &str, plan: &[u8], compat: &str) -> i32 {
    let started = Instant::now();
    app::prepare_environment(compat);
    if app::print_baked_receipt(compat) {
        return 0;
    }
    if let Err(error) = exact_runner::delivery::refuse_analysis(compat) {
        eprintln!("{error}");
        return 1;
    }
    if std::env::var_os("EXACT_INK_DEVICE").is_none() {
        std::env::set_var("EXACT_INK_DEVICE", "metal");
    }
    if std::env::var_os("EXACT_PAINTER").is_none() {
        std::env::set_var("EXACT_PAINTER", "ink");
    }
    app::show_neutral_look();
    let mut config = Config::from_env(plan, compat);
    if std::env::var_os("EXACT_ASSETS").is_none() {
        if let Some(resources) = std::env::current_exe().ok().and_then(|exe| exe.parent()?.parent().map(|contents| contents.join("Resources"))).filter(|path| path.is_dir()) {
            config.assets = resources;
        }
    }
    if let Some(path) = std::env::var_os("EXACT_AGENT_SOCKET") {
        if !path.is_empty() {
            config.agent = true;
        }
    }
    config.size = (
        std::env::var("EXACT_WINDOW_WIDTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(800.),
        std::env::var("EXACT_WINDOW_HEIGHT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(800.),
    );
    if !config.agent {
        config.clock_started = Some(started);
    }
    let events = match EventLoop::<Event>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let proxy = events.create_proxy();
    let wake = proxy.clone();
    exact_linux::set_event_waker(Some(Arc::new(move || {
        let _ = wake.send_event(Event::Wake);
    })));
    let mut state = App::<D> {
        name: name.into(),
        config,
        proxy,
        window: None,
        layer: None,
        display: None,
        presenter: None,
        agent: None,
        started,
        tick_pending: Default::default(),
        pointer: (0., 0.),
        buttons: 0,
        scale: 1.,
        hidden: false,
        failed: false,
        frames: 0,
        paint_ms: 0.,
        wheel_phases: [0; 2],
    };
    if let Err(error) = events.run_app(&mut state) {
        eprintln!("{error}");
        state.failed = true;
    }
    exact_linux::set_event_waker(None);
    if state.frames > 0 {
        eprintln!(
            "exact-apple-painted: {} frames, mean paint {:.3} ms (benchmark carrier)",
            state.frames,
            state.paint_ms / state.frames as f64
        );
    }
    if !exact_linux::host::drain_store_writes(Duration::from_secs(5)) {
        eprintln!("painted carrier: store writes did not drain");
        state.failed = true;
    }
    i32::from(state.failed)
}

impl<D: DataSource + Default + 'static> App<D> {
    fn fail(&mut self, events: &ActiveEventLoop, error: impl std::fmt::Display) {
        eprintln!("exact-apple-painted: {error}");
        self.failed = true;
        events.exit();
    }
    fn resized(&mut self) {
        let (Some(window), Some(layer), Some(p)) = (&self.window, &self.layer, &mut self.presenter)
        else {
            return;
        };
        let size = window.inner_size();
        self.scale = window.scale_factor();
        if size.width == 0 || size.height == 0 {
            return;
        }
        layer.resize([size.width, size.height], self.scale);
        if let Some(e) = p.resize_scaled(
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
            self.scale as f32,
        ) {
            eprintln!("resize: {e}");
        }
        if let (Some(display), Some(monitor)) = (&self.display, window.current_monitor()) {
            if let Err(e) = display.set_display(monitor.native_id()) {
                eprintln!("{e}");
            }
        }
    }
    fn draw(&mut self) -> Result<(), String> {
        if self.hidden {
            return Ok(());
        }
        let Some(p) = &mut self.presenter else {
            return Ok(());
        };
        if let Some(layer) = &self.layer {
            layer.update();
        }
        let now = if self.config.agent {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        let started = Instant::now();
        p.pump(now);
        if !self.config.agent {
            p.animation_frame(now);
        }
        p.run_commands(D::default);
        p.hatch_turn();
        p.poll_images();
        p.set_pointer(None);
        p.tick(now);
        if p.dirty() {
            if std::env::var_os("G1_OUTPUT").is_some() {
                let target = std::env::var("EXACT_BENCH_SCROLL_TARGET")
                    .unwrap_or_else(|_| "messages".into());
                let k = p.host().kernel();
                let offset = k
                    .find_by_test_id(&target)
                    .first()
                    .and_then(|key| k.node_by_key(*key))
                    .map_or(0., |node| p.scroll_of(node.id).1);
                exact_ink::device::apple::benchmark_frame(!p.images().pending(), offset);
            }
            p.apple_window_frame()?;
            self.paint_ms += started.elapsed().as_secs_f64() * 1000.;
            self.frames += 1;
            if self.frames == 1 {
                eprintln!("exact-apple-painted: first submission {:.3} ms; device {:.3}, shaders {:.3} ms; {}x{} at {}x (not a displayed timestamp)", self.started.elapsed().as_secs_f64() * 1000., p.painter.device_ms, p.painter.shaders_ms, p.viewport().0, p.viewport().1, self.scale);
            }
        }
        p.sync_surfaces();
        Ok(())
    }
}

impl<D: DataSource + Default + 'static> ApplicationHandler<Event> for App<D> {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let setup = (|| -> Result<(), String> {
            let window = Rc::new(
                events
                    .create_window(
                        Window::default_attributes()
                            .with_title(format!("{} — benchmark carrier", self.name))
                            .with_inner_size(LogicalSize::new(
                                self.config.size.0,
                                self.config.size.1,
                            ))
                            .with_visible(false),
                    )
                    .map_err(|e| e.to_string())?,
            );
            self.scale = window.scale_factor();
            self.config.scale = self.scale as f32;
            let size = window.inner_size();
            let viewport = (
                size.width as f32 / self.scale as f32,
                size.height as f32 / self.scale as f32,
            );
            let (mut p, warning) = app::boot_presenter::<D>(&mut self.config, viewport)?;
            if let Some(warning) = warning {
                return Err(warning);
            }
            p.host_mut().runner_mut().set_row_reuse(true);
            if !self.config.agent {
                p.set_live_clock(Some(exact_linux::presenter::wall_since(self.started)));
            }
            let proxy = self.proxy.clone();
            let layer = Layer::new(
                window.clone(),
                [size.width, size.height],
                self.scale,
                move || {
                    let _ = proxy.send_event(Event::Wake);
                },
            )
            .map_err(|e| e.to_string())?;
            p.attach_apple_layer(layer.clone())?;
            let proxy = self.proxy.clone();
            let pending = self.tick_pending.clone();
            let display = DisplayLink::new(move || {
                if !pending.swap(true, Ordering::AcqRel) {
                    let _ = proxy.send_event(Event::Tick);
                }
            })
            .map_err(|e| e.to_string())?;
            if let Some(monitor) = window.current_monitor() {
                display
                    .set_display(monitor.native_id())
                    .map_err(|e| e.to_string())?;
            }
            self.display = Some(display);
            self.layer = Some(layer);
            self.presenter = Some(p);
            self.window = Some(window.clone());
            window.set_visible(true);
            window.request_redraw();
            if self.config.agent {
                let ready = serde_json::json!({"ready":true,"boot":self.started.elapsed().as_secs_f64()*1000.,"views":self.presenter.as_ref().unwrap().node_count(),"error":null,"benchmarkCarrier":true}).to_string();
                self.agent = Some(super::agent::Agent::start(self.proxy.clone(), ready)?);
            }
            Ok(())
        })();
        if let Err(error) = setup {
            self.fail(events, error);
        }
    }
    fn user_event(&mut self, events: &ActiveEventLoop, event: Event) {
        match event {
            Event::Tick => {
                self.tick_pending.store(false, Ordering::Release);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Event::Wake => {}
            Event::Closed => events.exit(),
            Event::Request(line, reply) => {
                if let Some(p) = &mut self.presenter {
                    let request: serde_json::Value =
                        serde_json::from_str(&line).unwrap_or_default();
                    if request["op"] == "quit" {
                        let _ = reply.send("{\"quit\":true}".into());
                        events.exit();
                        return;
                    }
                    let mut answer: serde_json::Value =
                        serde_json::from_str(&exact_linux::agent::handle(p, &line))
                            .unwrap_or_default();
                    if request["op"] == "state" {
                        answer["carrier"] = serde_json::json!({"name":"exact-apple-painted","benchmark":true,"shipping":false,"scale":self.scale,"frames":self.frames,"paintMs":self.paint_ms,"wheelPhase":self.wheel_phases[0],"momentumPhase":self.wheel_phases[1],"ime":false,"accessibility":false});
                    }
                    let _ = reply.send(answer.to_string());
                    // Geometry and screenshots may have consumed the damage.
                    // The window still owes the driver's new frame.
                    p.repaint();
                }
            }
        }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                events.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
                self.resized();
                return;
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
            WindowEvent::RedrawRequested => {
                if let Some(w) = &self.window {
                    if w.scale_factor() != self.scale {
                        self.resized();
                    }
                }
                if let Err(e) = self.draw() {
                    self.fail(events, e);
                }
                return;
            }
            _ => {}
        }
        let Some(p) = &mut self.presenter else {
            return;
        };
        let now = if self.config.agent {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        p.advance(now);
        let (x, y) = self.pointer;
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (
                    position.x as f32 / self.scale as f32,
                    position.y as f32 / self.scale as f32,
                );
                let _ = p.pointer_move(self.pointer.0, self.pointer.1, now);
            }
            WindowEvent::CursorLeft { .. } if self.buttons == 0 => {
                let _ = p.pointer_lost(now);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                let bit = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 4,
                    _ => 0,
                };
                if down {
                    self.buttons |= bit;
                } else {
                    self.buttons &= !bit;
                }
                if button == MouseButton::Left {
                    let _ = if down {
                        p.pointer_down(x, y, now)
                    } else {
                        p.pointer_up(x, y, now)
                    };
                } else if bit != 0 {
                    p.pointer_aux(bit, down, x, y, now);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.wheel_phases = exact_ink::device::apple::wheel_phases();
                let (dx, dy) = match delta {
                    MouseScrollDelta::PixelDelta(d) => (
                        d.x as f32 / self.scale as f32,
                        d.y as f32 / self.scale as f32,
                    ),
                    MouseScrollDelta::LineDelta(x, y) => (x * 16., y * 16.),
                };
                // AppKit already supplies momentum deltas; never synthesize a second fling.
                p.wheel_at(x, y, -dx, -dy);
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                let code = match event.physical_key {
                    PhysicalKey::Code(c) => format!("{c:?}").replace("Super", "Meta"),
                    _ => "Unidentified".into(),
                };
                let key = match event.logical_key {
                    Key::Character(s) => s.to_string(),
                    Key::Named(k) => format!("{k:?}"),
                    _ => event.text.map(|s| s.to_string()).unwrap_or_default(),
                };
                if is_synthetic && event.state == ElementState::Pressed {
                    p.hold_modifier(&code, true);
                } else {
                    p.hardware_key(
                        &code,
                        &key,
                        event.state == ElementState::Pressed,
                        event.repeat,
                    );
                }
            }
            WindowEvent::Focused(focused) => {
                p.set_window_active(focused);
                if !focused {
                    self.buttons = 0;
                    let _ = p.pointer_lost(now);
                }
            }
            WindowEvent::Occluded(hidden) => {
                self.hidden = hidden;
                p.surface_lifecycle(hidden, false);
                if !hidden {
                    p.repaint();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if let Some(layer) = &self.layer {
            layer.update();
        }
        let Some(p) = &mut self.presenter else {
            return;
        };
        let now = if self.config.agent {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        p.pump(now);
        p.run_commands(D::default);
        p.hatch_turn();
        p.poll_images();
        if !self.config.agent {
            p.admit_idle_timer(now);
        }
        let active = !self.hidden
            && (p.dirty() || !self.config.agent && (p.wants_display_frames() || p.host().motion()));
        if let Some(display) = &mut self.display {
            if let Err(e) = display.set_running(active) {
                self.fail(events, e);
                return;
            }
        }
        let wake = if self.config.agent {
            None
        } else {
            p.wake_due_ms()
        };
        events.set_control_flow(wake.map_or(ControlFlow::Wait, |due| {
            ControlFlow::WaitUntil(self.started + Duration::from_secs_f64(due.max(now) / 1000.))
        }));
    }
}
