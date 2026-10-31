use exact_ink::counters;
use exact_ink::device::apple::{trace_mark, DisplayLink, Layer, Trace};
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
    event::{ElementState, MouseButton, MouseScrollDelta, StartCause, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, PhysicalKey},
    platform::macos::{EventLoopBuilderExtMacOS, MonitorHandleExtMacOS},
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
    live: bool,
    proxy: EventLoopProxy<Event>,
    window: Option<Rc<Window>>,
    layer: Option<Layer>,
    display: Option<DisplayLink>,
    presenter: Option<Presenter<D>>,
    first_frame: Option<exact_linux::presenter::SubmittedAppleFrame>,
    agent: Option<super::agent::Agent>,
    started: Instant,
    tick_pending: Arc<AtomicBool>,
    display_active: Arc<AtomicBool>,
    pointer: (f32, f32),
    buttons: u32,
    scale: f64,
    hidden: bool,
    failed: bool,
    frames: u64,
    paint_ms: f64,
    wheel_phases: [u64; 2],
    scrolling: Scrolling,
    benchmark_complete: bool,
    service_causes: u8,
}

impl<D: DataSource> Drop for App<D> {
    fn drop(&mut self) {
        self.display.take();
        self.presenter.take();
        self.layer.take();
    }
}

pub fn run<D: DataSource + Default + 'static>(name: &str, plan: &[u8], compat: &str) -> i32 {
    trace_mark(c"boot host entry", || 0.);
    let entry_trace = Trace::new(c"boot configuration and event loop");
    let started = Instant::now();
    app::prepare_environment(compat);
    counters::initialize();
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
        if let Some(resources) = std::env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.parent()?
                    .parent()
                    .map(|contents| contents.join("Resources"))
            })
            .filter(|path| path.is_dir())
        {
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
    let live = !config.agent || std::env::var_os("EXACT_AGENT_LIVE").is_some();
    if live {
        config.clock_started = Some(started);
    }
    let mut builder = EventLoop::<Event>::with_user_event();
    if std::env::var_os("G1_BOOT_ONLY").is_some() {
        builder.with_activate_ignoring_other_apps(false);
    }
    let events = match builder.build() {
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
        live,
        proxy,
        window: None,
        layer: None,
        display: None,
        presenter: None,
        first_frame: None,
        agent: None,
        started,
        tick_pending: Default::default(),
        display_active: Default::default(),
        pointer: (0., 0.),
        buttons: 0,
        scale: 1.,
        hidden: false,
        failed: false,
        frames: 0,
        paint_ms: 0.,
        wheel_phases: [0; 2],
        scrolling: Scrolling::default(),
        benchmark_complete: false,
        service_causes: 0,
    };
    drop(entry_trace);
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
    fn awaiting_first_present(&self) -> bool {
        self.frames > 0
            && self
                .layer
                .as_ref()
                .is_some_and(|layer| layer.presented_painter().is_none())
    }

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
        let old_scale = self.scale;
        self.scale = window.scale_factor();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let viewport = (
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
        );
        if old_scale != self.scale || p.host().viewport() != viewport {
            self.first_frame = None;
        }
        layer.resize([size.width, size.height], self.scale);
        if let Some(e) = p.resize_scaled(
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
            self.scale as f32,
        ) {
            eprintln!("resize: {e}");
        }
        self.update_display();
    }
    fn update_display(&self) {
        if let (Some(display), Some(monitor)) = (
            &self.display,
            self.window.as_ref().and_then(|w| w.current_monitor()),
        ) {
            counters::refresh_rate(monitor.refresh_rate_millihertz().unwrap_or(60_000));
            if let Err(e) = display.set_display(monitor.native_id()) {
                eprintln!("{e}");
            }
        }
    }
    fn draw(&mut self) -> Result<(), String> {
        let _trace = Trace::new(c"carrier draw");
        if self.hidden {
            return Ok(());
        }
        if !self.awaiting_first_present() {
            self.first_frame = None;
        }
        let Some(p) = &mut self.presenter else {
            return Ok(());
        };
        let now = if !self.live {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        let started = Instant::now();
        counters::service(counters::DRAW);
        let service_trace = Trace::with_value(c"draw service", f64::from(counters::DRAW) + 1.);
        prepare_draw(p, now, self.live)?;
        drop(service_trace);
        if !p.apple_painter_can_draw() {
            self.first_frame = None;
        }
        if p.dirty() && p.apple_painter_can_draw() {
            // Painting replaces the compose target (or the CPU startup device).
            // Only a successful submission below may become the next repeat.
            self.first_frame = None;
            if std::env::var_os("G1_OUTPUT").is_some() {
                self.benchmark_complete = !p.images().pending() && !p.images().has_refusals();
                let target = std::env::var("EXACT_BENCH_SCROLL_TARGET")
                    .unwrap_or_else(|_| "messages".into());
                let k = p.host().kernel();
                let offset = k
                    .find_by_test_id(&target)
                    .first()
                    .and_then(|key| k.node_by_key(*key))
                    .map_or(0., |node| p.scroll_of(node.id).1);
                exact_ink::device::apple::benchmark_frame(self.benchmark_complete, offset);
            }
            let frame = p.apple_window_frame()?;
            self.paint_ms += started.elapsed().as_secs_f64() * 1000.;
            let submitted = p.last_frame_succeeded();
            self.frames += u64::from(submitted);
            if self
                .layer
                .as_ref()
                .is_some_and(|layer| layer.presented_painter().is_none())
            {
                self.first_frame = frame;
            }
            if submitted && self.frames == 1 {
                eprintln!("exact-apple-painted: first submission {:.3} ms; device {:.3}, shaders {:.3} ms; {}x{} at {}x (not a displayed timestamp)", self.started.elapsed().as_secs_f64() * 1000., p.painter.device_ms, p.painter.shaders_ms, p.viewport().0, p.viewport().1, self.scale);
            }
        }
        p.sync_surfaces();
        Ok(())
    }
}

impl<D: DataSource + Default + 'static> ApplicationHandler<Event> for App<D> {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        let _trace = Trace::new(match cause {
            StartCause::Init => c"winit init",
            StartCause::Poll => c"winit poll",
            StartCause::ResumeTimeReached { .. } => c"winit timer wake",
            StartCause::WaitCancelled { .. } => c"winit event wake",
        });
        if counters::enabled() {
            self.service_causes = if matches!(cause, StartCause::ResumeTimeReached { .. }) {
                counters::TIMER
            } else {
                0
            };
        }
    }
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let setup = (|| -> Result<(), String> {
            let window_trace = Trace::new(c"boot create window");
            exact_ink::device::apple::benchmark_counters();
            let window = Rc::new(
                events
                    .create_window(
                        Window::default_attributes()
                            .with_title(format!("{} — benchmark carrier", self.name))
                            .with_inner_size(LogicalSize::new(
                                self.config.size.0,
                                self.config.size.1,
                            ))
                            .with_active(std::env::var_os("G1_BOOT_ONLY").is_none())
                            .with_visible(false),
                    )
                    .map_err(|e| e.to_string())?,
            );
            drop(window_trace);
            trace_mark(c"boot window created", || 0.);
            self.scale = window.scale_factor();
            self.config.scale = self.scale as f32;
            let size = window.inner_size();
            let viewport = (
                size.width as f32 / self.scale as f32,
                size.height as f32 / self.scale as f32,
            );
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
            layer.set_background(self.config.initial_window_background());
            window.set_visible(true);
            trace_mark(c"boot window shown", || 0.);
            let presenter_trace = Trace::new(c"boot presenter");
            let (mut p, warning) = if self.config.painter()
                == exact_linux::presenter::PainterChoice::Ink
                && std::env::var("EXACT_INK_DEVICE").as_deref() == Ok("metal")
            {
                let proxy = self.proxy.clone();
                let preparing = exact_ink::device::apple::Startup::start(Arc::new(move || {
                    let _ = proxy.send_event(Event::Wake);
                }))
                .map_err(|e| e.to_string())?;
                app::boot_presenter_with_metal::<D>(&mut self.config, viewport, preparing)?
            } else {
                app::boot_presenter::<D>(&mut self.config, viewport)?
            };
            drop(presenter_trace);
            if let Some(warning) = warning {
                return Err(warning);
            }
            p.host_mut().runner_mut().set_row_reuse(true);
            if self.live {
                p.set_live_clock(Some(exact_linux::presenter::wall_since(self.started)));
            }
            p.attach_apple_layer(layer.clone())?;
            trace_mark(c"boot layer attached", || 0.);
            trace_mark(c"boot device milliseconds", || p.painter.device_ms);
            trace_mark(c"boot shaders milliseconds", || p.painter.shaders_ms);
            let proxy = self.proxy.clone();
            let pending = self.tick_pending.clone();
            let active = self.display_active.clone();
            let display = DisplayLink::new(move || {
                trace_mark(c"display demand", || {
                    f64::from(active.load(Ordering::Acquire))
                });
                if active.load(Ordering::Acquire) && !pending.swap(true, Ordering::AcqRel) {
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
            self.draw()?;
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
                if counters::enabled() {
                    self.service_causes |= counters::LINK;
                }
                trace_mark(c"display wake", || 0.);
                self.tick_pending.store(false, Ordering::Release);
                if self.hidden {
                    return;
                }
                let unshown = self.awaiting_first_present();
                if !unshown {
                    self.first_frame = None;
                }
                if self.first_frame.is_some() {
                    let before = self.frames;
                    if let Err(e) = self.draw() {
                        self.fail(events, e);
                        return;
                    }
                    if self.frames != before {
                        return;
                    }
                }
                if let Some(frame) = &self.first_frame {
                    if self
                        .layer
                        .as_ref()
                        .is_some_and(Layer::first_present_dropped)
                    {
                        // With no new paint, repeat the latest submitted pixels.
                        let _trace = Trace::new(c"first frame repeat");
                        match self.presenter.as_mut().unwrap().repeat_apple_frame(frame) {
                            Ok(true) => {}
                            Ok(false) => self.first_frame = None,
                            Err(e) => self.fail(events, e),
                        }
                    }
                    return;
                }
                let dirty = self
                    .presenter
                    .as_ref()
                    .is_some_and(|p| display_needed(p, self.live, self.hidden, unshown));
                self.scrolling.tick(dirty);
                if dirty {
                    if unshown {
                        // GPU completion does not prove the compositor showed a frame.
                        self.presenter.as_mut().unwrap().repaint();
                    }
                    if let Some(window) = &self.window {
                        trace_mark(c"display requests redraw", || 0.);
                        window.request_redraw();
                    }
                }
            }
            Event::Wake => {
                if counters::enabled() {
                    self.service_causes |= counters::WORKER;
                }
                if self
                    .presenter
                    .as_ref()
                    .is_some_and(|p| p.apple_painter_starting())
                {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            Event::Closed => events.exit(),
            Event::Request(line, reply) => {
                if counters::enabled() {
                    self.service_causes |= counters::AGENT;
                }
                let _trace = Trace::new(c"agent request");
                if let Some(p) = &mut self.presenter {
                    let request: serde_json::Value =
                        serde_json::from_str(&line).unwrap_or_default();
                    if !matches!(request["op"].as_str(), Some("state" | "tree" | "logs")) {
                        self.first_frame = None;
                    }
                    if request["op"] == "state" && request["counters"] == true {
                        let _ = reply.send(exact_linux::ink::counter_state().to_string());
                        return;
                    }
                    if matches!(
                        request["op"].as_str(),
                        Some("tap" | "type" | "clock" | "prefer")
                    ) {
                        if let Some(layer) = &self.layer {
                            layer.resume_retries();
                        }
                    }
                    if request["platform"] == true && request.get("wheel").is_some() {
                        let _ = reply.send(serde_json::json!({"error":"platform wheel placement is not available in the benchmark agent; use ordinary wheel operations or the G1 process-targeted driver"}).to_string());
                        return;
                    }
                    if self.live && request["op"] == "tap" && request.get("wheel").is_some() {
                        let answer = live_wheel(p, &request, &mut self.scrolling)
                            .unwrap_or_else(|e| serde_json::json!({"error":e}));
                        let _ = reply.send(answer.to_string());
                        return;
                    }
                    if request["op"] == "quit" {
                        let _ = reply.send("{\"quit\":true}".into());
                        events.exit();
                        return;
                    }
                    let mut answer: serde_json::Value =
                        serde_json::from_str(&exact_linux::agent::handle_for_window(p, &line))
                            .unwrap_or_default();
                    if request["op"] == "state" {
                        answer["carrier"] = serde_json::json!({"name":"exact-apple-painted","benchmark":true,"shipping":false,"scale":self.scale,"frames":self.frames,"paintMs":self.paint_ms,"wheelPhase":self.wheel_phases[0],"momentumPhase":self.wheel_phases[1],"ime":false,"accessibility":false});
                    }
                    let _ = reply.send(answer.to_string());
                }
            }
        }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let _trace = Trace::new(c"winit window event");
        if counters::enabled() {
            self.service_causes |= match event {
                WindowEvent::MouseWheel { .. } => counters::WHEEL,
                WindowEvent::RedrawRequested => counters::DRAW,
                _ => counters::WINDOW,
            };
        }
        if matches!(
            event,
            WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::Focused(_)
                | WindowEvent::Occluded(_)
        ) {
            if let Some(layer) = &self.layer {
                layer.resume_retries();
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                events.exit();
                return;
            }
            WindowEvent::Moved(_) => {
                self.update_display();
                return;
            }
            WindowEvent::Resized(_) => {
                self.resized();
                if self.layer.as_ref().is_some_and(Layer::live_resize) {
                    if let Err(e) = self.draw() {
                        self.fail(events, e);
                    }
                }
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
        let now = if !self.live {
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
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let _trace = Trace::new(c"wheel input");
                self.wheel_phases = exact_ink::device::apple::wheel_phases();
                self.scrolling.wheel(
                    self.wheel_phases != [0; 2]
                        && matches!(phase, TouchPhase::Started | TouchPhase::Moved),
                );
                let [x, y] = self
                    .layer
                    .as_ref()
                    .and_then(Layer::wheel_position)
                    .unwrap_or([x, y]);
                let (dx, dy) = match delta {
                    MouseScrollDelta::PixelDelta(d) => (
                        d.x as f32 / self.scale as f32,
                        d.y as f32 / self.scale as f32,
                    ),
                    MouseScrollDelta::LineDelta(x, y) => (x * 16., y * 16.),
                };
                // AppKit already supplies momentum deltas; never synthesize a second fling.
                p.wheel_at(x, y, -dx, -dy);
                trace_mark(c"wheel dirty", || f64::from(p.dirty()));
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
                    self.scrolling.wheel(false);
                    let _ = p.pointer_lost(now);
                }
            }
            WindowEvent::Occluded(hidden) => {
                trace_mark(c"window hidden", || f64::from(hidden));
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
        let unshown = self.awaiting_first_present();
        let _trace = Trace::new(c"winit dispatch");
        let Some(p) = &mut self.presenter else {
            return;
        };
        if self.layer.as_ref().is_some_and(Layer::retry_requested) && self.first_frame.is_none() {
            p.repaint();
        }
        let now = if !self.live {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        let causes = if counters::enabled() {
            std::mem::take(&mut self.service_causes)
        } else {
            0
        };
        service(p, now, self.live, D::default, causes);
        if self.first_frame.is_none()
            && p.apple_painter_retry_deadline()
                .is_some_and(|due| Instant::now() >= due)
        {
            if let Err(e) = p.poll_apple_painter() {
                self.fail(events, e);
                return;
            }
        }
        // Offscreen metadata can finish after the last visible image. Submit a
        // complete scene for the probe even when that completion owes no damage.
        if std::env::var_os("G1_OUTPUT").is_some()
            && !self.benchmark_complete
            && !p.images().pending()
            && !p.images().has_refusals()
        {
            p.repaint();
        }
        if self.live {
            p.admit_idle_timer(now);
        }
        let active = display_needed(p, self.live, self.hidden, unshown);
        let running = !self.hidden && (active || self.scrolling.active);
        self.display_active.store(running, Ordering::Release);
        if let Some(display) = &mut self.display {
            if let Err(e) = display.set_running(running) {
                self.fail(events, e);
                return;
            }
        }
        let wake = if !self.live { None } else { p.wake_due_ms() };
        let wake = wake.map(|due| self.started + Duration::from_secs_f64(due.max(now) / 1000.));
        events.set_control_flow(
            wake.into_iter()
                .chain(p.apple_painter_retry_deadline())
                .min()
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
}

fn prepare_draw<D: DataSource + Default>(
    p: &mut Presenter<D>,
    now: f64,
    live: bool,
) -> Result<(), String> {
    p.pump(now);
    if live {
        p.animation_frame(now);
    }
    p.run_commands(D::default);
    p.hatch_turn();
    p.poll_images();
    p.set_pointer(None);
    p.tick(now);
    p.poll_apple_painter()
}

fn display_needed<D: DataSource>(
    p: &Presenter<D>,
    live: bool,
    hidden: bool,
    unshown: bool,
) -> bool {
    !hidden
        && p.apple_painter_can_draw()
        && (unshown || p.dirty() || live && p.needs_animation_frame())
}

#[derive(Default)]
struct Scrolling {
    active: bool,
    quiet_ticks: u8,
}
impl Scrolling {
    fn wheel(&mut self, active: bool) {
        self.active = active;
        self.quiet_ticks = 0;
    }
    fn tick(&mut self, dirty: bool) {
        if self.active && !dirty {
            self.quiet_ticks += 1;
            if self.quiet_ticks == 3 {
                self.active = false;
            }
        } else {
            self.quiet_ticks = 0;
        }
    }
}

// Live drives leave damage for the display link; the headless agent reads pixels.
fn live_wheel<D: DataSource>(
    p: &mut Presenter<D>,
    request: &serde_json::Value,
    scrolling: &mut Scrolling,
) -> Result<serde_json::Value, &'static str> {
    let pair = |v: &serde_json::Value| {
        let v = v.as_array().filter(|v| v.len() == 2)?;
        let pair = (v[0].as_f64()? as f32, v[1].as_f64()? as f32);
        (pair.0.is_finite() && pair.1.is_finite()).then_some(pair)
    };
    let (dx, dy) = pair(&request["wheel"]).ok_or("wheel needs two finite deltas")?;
    let (x, y) = pair(&request["at"]).ok_or("live wheel needs at:[x,y] in viewport points")?;
    scrolling.wheel(matches!(
        request["phase"].as_str(),
        Some("began" | "changed")
    ));
    p.wheel_at(x, y, dx, dy);
    Ok(serde_json::json!({"wheel":[dx,dy],"clock":p.host().now(),"live":true}))
}

fn service<D: DataSource>(
    p: &mut Presenter<D>,
    now: f64,
    live: bool,
    data: impl FnMut() -> D,
    causes: u8,
) {
    counters::service(causes);
    let _trace = Trace::with_value(c"carrier service", f64::from(causes) + 1.);
    // Timers can expire without a frame. Leaving their deadline in the past
    // makes WaitUntil wake continuously even though the display link is stopped.
    if live {
        p.advance(now);
    }
    p.pump(now);
    p.run_commands(data);
    p.hatch_turn();
    p.poll_images();
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_linux::presenter::PainterChoice;

    #[test]
    fn the_first_frame_after_layout_matches_the_display_tick_frame() {
        let plan = contract::compile(
            r##"component App
  view
    scroll width=160 height=120 background-color="#23394f"
      column width=160 gap=8 padding=8
        text "First frame" color="#f3e7ca" font-size=18
        box width=75 height=35 background-color="#e83a5b" border-radius=7
        text "Kept rows 012345" color="white"
        box width=100 height=60 background-color="#4488cc"
"##,
        )
        .unwrap()
        .encode();
        for choice in [PainterChoice::Cpu, PainterChoice::Ink] {
            let make = || {
                let (mut p, warning) =
                    Presenter::boot_with(&plan, (), (160., 120.), 2., ".".into(), choice).unwrap();
                assert!(warning.is_none(), "{warning:?}");
                p.host_mut().runner_mut().set_row_reuse(true);
                p
            };
            let mut early = make();
            prepare_draw(&mut early, 0., false).unwrap();
            let first = early.frame();
            assert!(early.last_frame_succeeded());

            let mut on_tick = make();
            service(&mut on_tick, 0., false, || (), 0);
            assert!(display_needed(&on_tick, false, false, false));
            prepare_draw(&mut on_tick, 0., false).unwrap();
            let expected = on_tick.frame();
            assert!(on_tick.last_frame_succeeded());
            assert_eq!(first.data(), expected.data(), "{choice:?}: first frame");
            assert!(first
                .data()
                .chunks_exact(4)
                .any(|p| p != &first.data()[..4]));
            early.repaint();
            prepare_draw(&mut early, 0., false).unwrap();
            assert_eq!(
                early.frame().data(),
                expected.data(),
                "{choice:?}: unshown retry"
            );
        }
    }

    #[test]
    fn an_unshown_boot_keeps_display_demand_after_two_completed_drawables() {
        let plan = contract::compile("component App\n  view\n    text \"Still\"\n")
            .unwrap()
            .encode();
        let (mut p, warning) =
            Presenter::boot_with(&plan, (), (80., 80.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(warning.is_none(), "{warning:?}");
        // Neither of the first two completed frames was selected for display.
        for _ in 0..2 {
            p.repaint();
            p.frame();
        }
        assert!(!p.dirty());
        for period in 2..8 {
            assert!(
                display_needed(&p, true, false, true),
                "stopped at period {period}"
            );
            assert!(
                !display_needed(&p, true, true, true),
                "hidden windows must sleep"
            );
            p.repaint();
            p.frame();
        }
        // A positive presented timestamp ends retries; the clean scene stays idle.
        assert!(!display_needed(&p, true, false, false));
        p.repaint();
        assert!(display_needed(&p, true, false, false));
    }

    #[test]
    fn agent_reads_leave_an_idle_window_clean_and_input_leaves_damage() {
        let plan = contract::compile("component App\n  state count: number = 0\n  action increment\n    count = count + 1\n  view\n    column\n      button testId=\"increment\" press=increment\n        text \"Increment\"\n      text count.toString()\n").unwrap().encode();
        let (mut p, error) =
            Presenter::boot_with(&plan, (), (120., 100.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        for op in ["state", "tree", "layout", "logs"] {
            let answer = exact_linux::agent::handle_for_window(
                &mut p,
                &serde_json::json!({"op":op}).to_string(),
            );
            assert!(!answer.contains("\"error\""), "{answer}");
            assert!(!p.dirty(), "{op} manufactured window damage");
        }
        let key = p.host().kernel().find_by_test_id("increment")[0];
        let id = p.host().kernel().node_by_key(key).unwrap().id;
        exact_linux::agent::handle_for_window(
            &mut p,
            &serde_json::json!({"op":"tap","id":id}).to_string(),
        );
        assert!(p.dirty(), "a tap leaves its changed frame for the window");
        exact_linux::agent::handle_for_window(&mut p, "{\"op\":\"layout\"}");
        assert!(p.dirty(), "geometry must preserve existing window damage");
        p.frame();
        assert!(!p.dirty());
    }

    #[test]
    fn a_lost_wheel_end_stops_after_three_clean_callbacks() {
        let mut scroll = Scrolling::default();
        scroll.wheel(true);
        for _ in 0..6 {
            scroll.tick(true);
        }
        assert!(scroll.active, "damage keeps the link running");
        scroll.tick(false);
        scroll.tick(false);
        assert!(scroll.active);
        scroll.wheel(true);
        scroll.tick(false);
        scroll.tick(false);
        assert!(scroll.active, "a new wheel resets the quiet count");
        scroll.tick(false);
        assert!(!scroll.active);
        scroll.wheel(true);
        scroll.wheel(false);
        assert!(!scroll.active, "an end stops immediately");
    }

    #[test]
    fn a_live_wheel_leaves_the_changed_scene_for_the_display_link() {
        let plan = contract::compile("component App\n  view\n    scroll width=80 height=80 testId=\"rows\"\n      view width=80 height=800 background-color=\"red\"\n").unwrap().encode();
        let (mut p, error) =
            Presenter::boot_with(&plan, (), (80., 80.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        assert!(!p.dirty());
        let before = p.host().now();
        let mut scrolling = Scrolling::default();
        live_wheel(
            &mut p,
            &serde_json::json!({"at":[40,40],"wheel":[0,20],"phase":"began"}),
            &mut scrolling,
        )
        .unwrap();
        assert!(
            p.dirty(),
            "input must not consume the window's pending frame"
        );
        assert!(scrolling.active);
        assert_eq!(p.host().now(), before, "input must not advance the clock");
        let rows = p.host().kernel().find_by_test_id("rows")[0];
        let id = p.host().kernel().node_by_key(rows).unwrap().id;
        assert_eq!(p.scroll_of(id).1, 20.);
        live_wheel(
            &mut p,
            &serde_json::json!({"at":[40,40],"wheel":[0,0],"phase":"ended"}),
            &mut scrolling,
        )
        .unwrap();
        assert!(!scrolling.active);
        assert!(live_wheel(&mut p, &serde_json::json!({"wheel":[0,20]}), &mut scrolling).is_err());
    }

    #[test]
    fn a_timer_that_changes_no_pixels_advances_without_a_display_frame() {
        let plan = contract::compile("component App\n  state enabled: bool = false\n  state count: number = 0\n  task clock mount\n    every(250, tick)\n  action tick\n    if enabled\n      count = count + 1\n  view\n    text \"Still\"\n").unwrap().encode();
        let (mut p, error) =
            Presenter::boot_with(&plan, (), (80., 80.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        assert!(!p.dirty());
        let due = p.wake_due_ms().unwrap();
        service(&mut p, due + 1., true, || (), 0);
        assert!(!p.dirty());
        assert!(!p.host().motion() && !p.wants_display_frames());
        assert!(p.wake_due_ms().unwrap() > due + 1.);
        let next = p.wake_due_ms();
        service(&mut p, due + 1000., false, || (), 0);
        assert_eq!(p.wake_due_ms(), next, "the agent owns its clock");
    }
}
