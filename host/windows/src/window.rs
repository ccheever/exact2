use exact_linux::{
    app::{self, Config},
    Presenter,
};
use exact_runner::DataSource;
use std::{
    num::NonZeroU32,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, PhysicalKey},
    window::{Window, WindowId},
};

struct App<D: DataSource> {
    config: Config,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    presenter: Option<Presenter<D>>,
    started: Instant,
    pointer: (f32, f32),
    buttons: u32,
    scale: f64,
    next_frame: Instant,
    failed: bool,
    minimized: bool,
    frames: u64,
    paint_ms: f64,
    present_ms: f64,
    previous_frame: Option<Instant>,
    samples: Vec<(f64, f64, f64)>,
    frame_limit: Option<u64>,
    first_tap: Option<String>,
}

pub(super) fn run<D: DataSource + Default + 'static>(plan: &[u8], compat: &str) -> i32 {
    if app::print_baked_receipt(compat) {
        return 0;
    }
    if let Err(why) = exact_runner::delivery::refuse_analysis(compat) {
        eprintln!("exact-windows: {why}");
        return 1;
    }
    if exact_runner::Delivery::default().with_compat(compat).store != '0' {
        eprintln!("exact-windows: update-store delivery is not implemented; bake store level 0");
        return 1;
    }
    // A distributed app locates data beside its executable, never in its caller's cwd.
    if std::env::var_os("EXACT_ASSETS").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(root) = exe.parent() {
                std::env::set_var("EXACT_ASSETS", root);
            }
        }
    }
    if std::env::var_os("EXACT_PAINTER").is_none() {
        std::env::set_var("EXACT_PAINTER", "cpu");
    }
    if std::env::var_os("EXACT_GPU_RENDER").is_none() {
        std::env::set_var("EXACT_GPU_RENDER", "1");
    }
    let started = Instant::now();
    let mut config = Config::from_env(plan, compat);
    // Config has already stripped EXACT_AGENT_* for a production bake.
    let frame_limit = match std::env::var("EXACT_AGENT_WINDOW_FRAMES") {
        Ok(value) => match value.parse::<u64>() {
            Ok(count @ 1..=10_000) => Some(count),
            _ => {
                eprintln!("exact-windows: EXACT_AGENT_WINDOW_FRAMES requires 1..=10000");
                return 1;
            }
        },
        Err(_) => None,
    };
    let first_tap = std::env::var("EXACT_AGENT_WINDOW_TAP").ok();
    if first_tap.is_some() && frame_limit.is_none() {
        eprintln!("exact-windows: EXACT_AGENT_WINDOW_TAP requires a bounded window frame run");
        return 1;
    }
    if config.agent
        || config.smoke
        || config.shot.is_some()
        || std::env::var("EXACT_DISPLAY").as_deref() == Ok("headless")
    {
        return app::run_config::<D>(&mut config, started);
    }
    if std::env::var_os("EXACT_SIZE").is_none() {
        config.size = (1280.0, 720.0);
    }
    let event_loop = match EventLoop::<()>::with_user_event().build() {
        Ok(value) => value,
        Err(e) => {
            eprintln!("exact-windows: {e}");
            return 1;
        }
    };
    let proxy = event_loop.create_proxy();
    exact_linux::set_event_waker(Some(Arc::new(move || {
        let _ = proxy.send_event(());
    })));
    let mut state = App::<D> {
        config,
        window: None,
        surface: None,
        presenter: None,
        started,
        pointer: (0., 0.),
        buttons: 0,
        scale: 1.,
        next_frame: started,
        failed: false,
        minimized: false,
        frames: 0,
        paint_ms: 0.,
        present_ms: 0.,
        previous_frame: None,
        samples: Vec::new(),
        frame_limit,
        first_tap,
    };
    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("exact-windows: {e}");
        state.failed = true;
    }
    exact_linux::set_event_waker(None);
    if state.frames > 0 {
        eprintln!("exact-windows: {} frames; mean paint including GPU readback {:.2} ms; mean upload/present {:.2} ms",
            state.frames, state.paint_ms / state.frames as f64, state.present_ms / state.frames as f64);
        let percentile = |column: fn(&(f64, f64, f64)) -> f64, fraction: f64| {
            let mut values: Vec<_> = state.samples.iter().map(column).collect();
            values.sort_by(f64::total_cmp);
            values[((values.len() - 1) as f64 * fraction).round() as usize]
        };
        if !state.samples.is_empty() {
            eprintln!("exact-windows: recent {} frames p50/p95 ms: interval {:.2}/{:.2}, paint+readback {:.2}/{:.2}, upload/present {:.2}/{:.2}",
                state.samples.len(), percentile(|s| s.0, 0.5), percentile(|s| s.0, 0.95),
                percentile(|s| s.1, 0.5), percentile(|s| s.1, 0.95),
                percentile(|s| s.2, 0.5), percentile(|s| s.2, 0.95));
        }
    }
    i32::from(state.failed)
}

impl<D: DataSource + Default + 'static> App<D> {
    fn fail(&mut self, events: &ActiveEventLoop, error: impl std::fmt::Display) {
        eprintln!("exact-windows: {error}");
        self.failed = true;
        events.exit();
    }
    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.
    }
    fn resized(&mut self) {
        let (Some(window), Some(p)) = (&self.window, &mut self.presenter) else {
            return;
        };
        let size = window.inner_size();
        self.minimized = size.width == 0 || size.height == 0;
        self.scale = window.scale_factor();
        if !self.minimized {
            if let Some(error) = p.resize_scaled(
                size.width as f32 / self.scale as f32,
                size.height as f32 / self.scale as f32,
                self.scale as f32,
            ) {
                eprintln!("exact-windows: resize: {error}");
            }
            window.request_redraw();
        }
    }
    fn draw(&mut self, events: &ActiveEventLoop) -> Result<(), String> {
        let frame_started = Instant::now();
        if self.minimized {
            return Ok(());
        }
        let now = self.now();
        let (Some(window), Some(surface), Some(p)) =
            (&self.window, &mut self.surface, &mut self.presenter)
        else {
            return Ok(());
        };
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        p.pump(now);
        p.animation_frame(now);
        p.run_commands(D::default);
        p.poll_images();
        // The OS draws the desktop cursor; this presenter's arrow is for DRM.
        p.set_pointer(None);
        if self.frames > 0 {
            p.sync_surfaces();
        }
        p.tick(now);
        let paint = Instant::now();
        let frame = p.frame();
        let paint_ms = paint.elapsed().as_secs_f64() * 1000.;
        self.paint_ms += paint_ms;
        let present = Instant::now();
        surface.resize(width, height).map_err(|e| e.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|e| e.to_string())?;
        if buffer.len() != frame.data().len() / 4 {
            return Err(format!(
                "frame extent {}x{} disagrees with window {}x{}",
                frame.width(),
                frame.height(),
                width,
                height
            ));
        }
        for (out, pixel) in buffer.iter_mut().zip(frame.data().chunks_exact(4)) {
            *out = (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
        }
        buffer.present().map_err(|e| e.to_string())?;
        let present_ms = present.elapsed().as_secs_f64() * 1000.;
        self.present_ms += present_ms;
        if let Some(previous) = self.previous_frame.replace(frame_started) {
            let sample = (
                (frame_started - previous).as_secs_f64() * 1000.,
                paint_ms,
                present_ms,
            );
            if self.samples.len() < 4096 {
                self.samples.push(sample);
            } else {
                self.samples[(self.frames as usize - 1) % 4096] = sample;
            }
        }
        self.frames += 1;
        p.first_pixel();
        if let Some(target) = self.first_tap.take() {
            let keys = p.host().kernel().find_by_test_id(&target);
            if keys.len() != 1 {
                return Err(format!(
                    "window benchmark tap {target:?} must identify exactly one view"
                ));
            }
            let id = p.host().kernel().node_by_key(keys[0]).unwrap().id;
            p.tap(id)?;
        }
        p.sync_surfaces();
        if self.frame_limit.is_some_and(|limit| self.frames >= limit) {
            events.exit();
        }
        self.next_frame = frame_started + Duration::from_micros(16_667);
        events.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        Ok(())
    }
}

impl<D: DataSource + Default + 'static> ApplicationHandler for App<D> {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let metadata: serde_json::Value =
            serde_json::from_str(&self.config.compat).unwrap_or_default();
        let title = metadata["inputs"]["app"].as_str().unwrap_or("Exact");
        let window = match events.create_window(
            Window::default_attributes()
                .with_title(title)
                .with_inner_size(LogicalSize::new(self.config.size.0, self.config.size.1)),
        ) {
            Ok(value) => Rc::new(value),
            Err(e) => {
                self.fail(events, e);
                return;
            }
        };
        self.scale = window.scale_factor();
        self.config.scale = self.scale as f32;
        let size = window.inner_size();
        let viewport = (
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
        );
        match app::boot_presenter::<D>(&mut self.config, viewport) {
            Ok((p, warning)) => {
                if let Some(warning) = warning {
                    eprintln!("exact-windows: {warning}");
                }
                self.presenter = Some(p);
            }
            Err(e) => {
                self.fail(events, e);
                return;
            }
        }
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.fail(events, e);
                return;
            }
        };
        match softbuffer::Surface::new(&context, window.clone()) {
            Ok(surface) => self.surface = Some(surface),
            Err(e) => {
                self.fail(events, e);
                return;
            }
        }
        window.request_redraw();
        self.window = Some(window);
    }
    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            events.exit();
            return;
        }
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.resized();
            return;
        }
        if matches!(event, WindowEvent::RedrawRequested) {
            if let Err(e) = self.draw(events) {
                self.fail(events, e);
            }
            return;
        }
        let now = self.now();
        let Some(p) = &mut self.presenter else { return };
        p.advance(now);
        let (x, y) = self.pointer;
        match event {
            WindowEvent::Focused(false) => {
                self.buttons = 0;
                let _ = p.pointer_lost(now);
                p.blur();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (
                    position.x as f32 / self.scale as f32,
                    position.y as f32 / self.scale as f32,
                );
                let _ = p.pointer_move(self.pointer.0, self.pointer.1, now);
            }
            WindowEvent::CursorLeft { .. } => {
                // Windows captures pressed buttons. Preserve the drag until its
                // captured mouse-up, even outside the window; blur cancels it.
                if self.buttons == 0 {
                    let _ = p.pointer_lost(now);
                }
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
                match button {
                    MouseButton::Left => {
                        let _ = if down {
                            p.pointer_down(x, y, now)
                        } else {
                            p.pointer_up(x, y, now)
                        };
                    }
                    MouseButton::Right => p.pointer_aux(2, down, x, y, now),
                    MouseButton::Middle => p.pointer_aux(4, down, x, y, now),
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x * 40., -y * 40.),
                    MouseScrollDelta::PixelDelta(at) => (
                        -at.x as f32 / self.scale as f32,
                        -at.y as f32 / self.scale as f32,
                    ),
                };
                p.wheel_at(x, y, dx, dy);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let code = format!("{code:?}").replace("Super", "Meta");
                    let key = match event.logical_key {
                        Key::Character(s) => s.to_string(),
                        Key::Named(k) => format!("{k:?}"),
                        _ => String::new(),
                    };
                    p.hardware_key(
                        &code,
                        &key,
                        event.state == ElementState::Pressed,
                        event.repeat,
                    );
                }
            }
            _ => return,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let Some(p) = &self.presenter else { return };
        if self.minimized {
            events.set_control_flow(ControlFlow::Wait);
            return;
        }
        if self.frame_limit.is_some() || p.wants_display_frames() || p.host().motion() {
            if Instant::now() >= self.next_frame {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            events.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else if let Some(due) = p.host().timer_due_ms() {
            let deadline = self.started + Duration::from_secs_f64((due / 1000.).max(0.));
            if Instant::now() >= deadline {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            events.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            events.set_control_flow(ControlFlow::Wait);
        }
    }
}
