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
    event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
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
    display_active: Arc<AtomicBool>,
    pointer: (f32, f32),
    buttons: u32,
    scale: f64,
    hidden: bool,
    failed: bool,
    frames: u64,
    paint_ms: f64,
    wheel_phases: [u64; 2],
    scrolling: bool,
    benchmark_complete: bool,
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
        display_active: Default::default(),
        pointer: (0., 0.),
        buttons: 0,
        scale: 1.,
        hidden: false,
        failed: false,
        frames: 0,
        paint_ms: 0.,
        wheel_phases: [0; 2],
        scrolling: false,
        benchmark_complete: false,
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
            let active = self.display_active.clone();
            let display = DisplayLink::new(move || {
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
                if self.display_active.load(Ordering::Acquire) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            Event::Wake => {}
            Event::Closed => events.exit(),
            Event::Request(line, reply) => {
                if let Some(p) = &mut self.presenter {
                    let request: serde_json::Value =
                        serde_json::from_str(&line).unwrap_or_default();
                    if request["platform"] == true && request.get("wheel").is_some() {
                        let _ = reply.send(serde_json::json!({"error":"platform wheel placement is not available in the benchmark agent; use ordinary wheel operations or the G1 process-targeted driver"}).to_string());
                        return;
                    }
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
            WindowEvent::MouseWheel { delta, phase, .. } => {
                self.wheel_phases = exact_ink::device::apple::wheel_phases();
                self.scrolling = self.wheel_phases != [0; 2]
                    && matches!(phase, TouchPhase::Started | TouchPhase::Moved);
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
                    self.scrolling = false;
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
        if self.layer.as_ref().is_some_and(Layer::retry_requested) {
            p.repaint();
        }
        let now = if self.config.agent {
            p.host().now()
        } else {
            self.started.elapsed().as_secs_f64() * 1000.
        };
        service(p, now, !self.config.agent, D::default);
        // Offscreen metadata can finish after the last visible image. Submit a
        // complete scene for the probe even when that completion owes no damage.
        if std::env::var_os("G1_OUTPUT").is_some()
            && !self.benchmark_complete
            && !p.images().pending()
            && !p.images().has_refusals()
        {
            p.repaint();
        }
        if !self.config.agent {
            p.admit_idle_timer(now);
        }
        let active = !self.hidden && (p.dirty() || !self.config.agent && p.needs_animation_frame());
        self.display_active.store(active, Ordering::Release);
        if let Some(display) = &mut self.display {
            if let Err(e) = display.set_running(!self.hidden && (active || self.scrolling)) {
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

fn service<D: DataSource>(p: &mut Presenter<D>, now: f64, live: bool, data: impl FnMut() -> D) {
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
    fn a_timer_that_changes_no_pixels_advances_without_a_display_frame() {
        let plan = contract::compile("component App\n  state enabled: bool = false\n  state count: number = 0\n  task clock mount\n    every(250, tick)\n  action tick\n    if enabled\n      count = count + 1\n  view\n    text \"Still\"\n").unwrap().encode();
        let (mut p, error) =
            Presenter::boot_with(&plan, (), (80., 80.), 1., ".".into(), PainterChoice::Cpu)
                .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        assert!(!p.dirty());
        let due = p.wake_due_ms().unwrap();
        service(&mut p, due + 1., true, || ());
        assert!(!p.dirty());
        assert!(!p.host().motion() && !p.wants_display_frames());
        assert!(p.wake_due_ms().unwrap() > due + 1.);
        let next = p.wake_due_ms();
        service(&mut p, due + 1000., false, || ());
        assert_eq!(p.wake_due_ms(), next, "the agent owns its clock");
    }
}
