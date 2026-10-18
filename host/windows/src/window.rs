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
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{CursorIcon, Fullscreen, Window, WindowId},
};

struct App<D: DataSource> {
    name: String,
    config: Config,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    presenter: Option<Presenter<D>>,
    started: Instant,
    pointer: (f32, f32),
    pointer_inside: bool,
    focused: bool,
    cursor: CursorIcon,
    buttons: u32,
    scale: f64,
    next_frame: Instant,
    failed: bool,
    minimized: bool,
    occluded: bool,
    frames: u64,
    paint_ms: f64,
    present_ms: f64,
    previous_frame: Option<Instant>,
    samples: Vec<(f64, f64, f64)>,
    frame_limit: Option<u64>,
    first_tap: Option<String>,
    pads: crate::pads::Pads,
}

impl<D: DataSource> Drop for App<D> {
    fn drop(&mut self) {
        // The module's GPU swapchain must release its HWND before the window.
        self.presenter.take();
    }
}

fn wheel_delta(delta: MouseScrollDelta, scale: f64) -> (f32, f32) {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => (-x * 40., -y * 40.),
        MouseScrollDelta::PixelDelta(at) => ((-at.x / scale) as f32, (-at.y / scale) as f32),
    }
}

fn next_frame_deadline(deadline: Instant, started: Instant, period: Duration) -> Instant {
    if started < deadline {
        // An input-triggered redraw does not move the display's cadence.
        deadline
    } else if started < deadline + period {
        // Keep timer wake-up latency from accumulating on every frame.
        deadline + period
    } else {
        // After a stall or suspension, resume without a burst of catch-up frames.
        started + period
    }
}

pub(super) fn run<D: DataSource + Default + 'static>(
    name: &str,
    plan: &[u8],
    compat: &str,
    hatches: Option<exact_linux::hatches::Install>,
) -> i32 {
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
    // Windows has no toolkit under this painter, so it shows the web's
    // neutral look (LLP 1115.000 D17), in a window as in a headless run.
    app::show_neutral_look();
    let mut config = Config::from_env(plan, compat);
    // The presenter makes the hatches' value after its first frame is shown.
    config.hatches = hatches;
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
        name: name.to_owned(),
        config,
        window: None,
        surface: None,
        presenter: None,
        started,
        pointer: (0., 0.),
        pointer_inside: false,
        focused: true,
        cursor: CursorIcon::Default,
        buttons: 0,
        scale: 1.,
        next_frame: started,
        failed: false,
        minimized: false,
        occluded: false,
        frames: 0,
        paint_ms: 0.,
        present_ms: 0.,
        previous_frame: None,
        samples: Vec::new(),
        frame_limit,
        first_tap,
        pads: Default::default(),
    };
    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("exact-windows: {e}");
        state.failed = true;
    }
    exact_linux::set_event_waker(None);
    if state.frames > 0 {
        eprintln!(
            "exact-windows: {} frames; mean render/present {:.2} ms; mean CPU upload {:.2} ms",
            state.frames,
            state.paint_ms / state.frames as f64,
            state.present_ms / state.frames as f64
        );
        let percentile = |column: fn(&(f64, f64, f64)) -> f64, fraction: f64| {
            let mut values: Vec<_> = state.samples.iter().map(column).collect();
            values.sort_by(f64::total_cmp);
            values[((values.len() - 1) as f64 * fraction).round() as usize]
        };
        if !state.samples.is_empty() {
            eprintln!("exact-windows: recent {} frames p50/p95 ms: interval {:.2}/{:.2}, render/present {:.2}/{:.2}, CPU upload {:.2}/{:.2}",
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
        self.minimized = window.is_minimized() == Some(true) || size.width == 0 || size.height == 0;
        p.surface_lifecycle(self.minimized || self.occluded, !self.focused);
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
        // A presented frame: its tasks, then the hatches' ticks and due
        // `after`s (LLP 1075.003.000.001 §2.4).
        p.animation_frame(now);
        p.run_commands(D::default);
        // The hatches' turn: what they asked of elements runs, then the
        // moments this turn's commits and the window's new size caused; an
        // overlay published here is in the frame painted below.
        p.hatch_turn();
        p.poll_images();
        // The OS draws the desktop cursor; this presenter's arrow is for DRM.
        p.set_pointer(None);
        if self.frames > 0 {
            p.sync_surfaces();
        }
        p.tick(now);
        if self.frames == 0 {
            eprintln!(
                "exact-windows: {}x{} pixels, scale {}, monitor {:?} mHz",
                window.inner_size().width,
                window.inner_size().height,
                self.scale,
                window
                    .current_monitor()
                    .and_then(|m| m.refresh_rate_millihertz())
            );
        }
        let paint = Instant::now();
        let RawWindowHandle::Win32(handle) =
            window.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("not a Windows window".into());
        };
        // SAFETY: App drops the presenter (and module swapchain) before the HWND.
        #[allow(unsafe_code)]
        let frame = unsafe {
            p.window_frame(
                handle.hwnd.get() as usize,
                handle.hinstance.map_or(0, |h| h.get() as usize),
            )
        }?;
        let paint_ms = paint.elapsed().as_secs_f64() * 1000.;
        self.paint_ms += paint_ms;
        let present = Instant::now();
        if let Some(frame) = frame {
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
                *out =
                    (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            }
            buffer.present().map_err(|e| e.to_string())?;
        }
        // A keyboard command can change the cursor while the mouse is still.
        // Re-resolve the painted hit after the frame, including unmounts and HUDs.
        let cursor = if self.pointer_inside && self.focused {
            // A CSS keyword is winit's cursor name (cursor-icon parses
            // CSS's); `auto` and `none` show the default arrow.
            p.cursor_at(self.pointer.0, self.pointer.1)
                .name()
                .parse::<CursorIcon>()
                .unwrap_or(CursorIcon::Default)
        } else {
            CursorIcon::Default
        };
        if self.cursor != cursor {
            window.set_cursor(cursor);
            self.cursor = cursor;
        }
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
        let period = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map_or(Duration::from_micros(16_667), |rate| {
                Duration::from_secs_f64(1000. / rate.max(1) as f64)
            });
        self.next_frame = if self.frame_limit.is_some()
            && std::env::var_os("EXACT_AGENT_WINDOW_UNPACED").is_some()
        {
            frame_started
        } else {
            next_frame_deadline(self.next_frame, frame_started, period)
        };
        events.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        Ok(())
    }
}

impl<D: DataSource + Default + 'static> ApplicationHandler for App<D> {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match events.create_window(
            Window::default_attributes()
                .with_title(&self.name)
                .with_fullscreen(
                    (self.frame_limit.is_some()
                        && std::env::var_os("EXACT_AGENT_WINDOW_FULLSCREEN").is_some())
                    .then_some(Fullscreen::Borderless(None)),
                )
                .with_inner_size(LogicalSize::new(self.config.size.0, self.config.size.1)),
        ) {
            Ok(value) => Rc::new(value),
            Err(e) => {
                self.fail(events, e);
                return;
            }
        };
        self.focused = window.has_focus();
        self.scale = window.scale_factor();
        self.config.scale = self.scale as f32;
        let size = window.inner_size();
        let viewport = (
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
        );
        match app::boot_presenter::<D>(&mut self.config, viewport) {
            Ok((mut p, warning)) => {
                p.surface_lifecycle(self.minimized || self.occluded, !self.focused);
                // The window's clock owns time (the timers RFC, D1)
                p.set_live_clock(Some(exact_linux::presenter::wall_since(self.started)));
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
            // The window is going: its hatch, then the app's, ends.
            if let Some(p) = &mut self.presenter {
                p.end_hatches();
            }
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
        let Some(p) = &mut self.presenter else { return };
        p.advance(self.started.elapsed().as_secs_f64() * 1000.);
        // The input's time: where the advance left the clock, a live runner's
        // sample past the time it was asked for
        let now = p.host().now();
        let (x, y) = self.pointer;
        match event {
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                p.surface_lifecycle(self.minimized || self.occluded, !focused);
                if !focused {
                    self.buttons = 0;
                    let _ = p.pointer_lost(now);
                    p.blur();
                    self.pads.forget();
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                p.surface_lifecycle(self.minimized || occluded, !self.focused);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_inside = true;
                self.pointer = (
                    position.x as f32 / self.scale as f32,
                    position.y as f32 / self.scale as f32,
                );
                let _ = p.pointer_move(self.pointer.0, self.pointer.1, now);
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_inside = false;
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
                let (dx, dy) = wheel_delta(delta, self.scale);
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
            if (!self.pointer_inside || !self.focused) && self.cursor != CursorIcon::Default {
                window.set_cursor(CursorIcon::Default);
                self.cursor = CursorIcon::Default;
            }
            window.request_redraw();
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let Some(p) = &mut self.presenter else { return };
        if self.minimized {
            events.set_control_flow(ControlFlow::Wait);
            return;
        }
        // Pads are polled while the window has focus (below: when it wakes for them).
        if self.focused {
            self.pads.poll(|index, connected, buttons, axes| {
                p.hardware_pad(index, connected, buttons, axes)
            });
        }
        if self.frame_limit.is_some() || p.wants_display_frames() || p.host().motion() {
            if Instant::now() >= self.next_frame {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            events.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else if let Some(due) = p.timer_due_ms() {
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
        // While focused the window wakes to poll the pads: often while one is
        // connected, and steadily for one that connects.
        if self.focused {
            let soon = self.pads.next_poll(Instant::now());
            if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= soon) {
                events.set_control_flow(ControlFlow::WaitUntil(soon));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_cadence_does_not_accumulate_timer_latency() {
        let start = Instant::now();
        for hz in [60, 120, 144] {
            let period = Duration::from_secs_f64(1.0 / hz as f64);
            let mut deadline = start;
            for frame in 1..=10_000 {
                deadline =
                    next_frame_deadline(deadline, deadline + Duration::from_micros(350), period);
                assert_eq!(deadline, start + period * frame);
            }
        }
    }
    #[test]
    fn early_redraws_preserve_cadence_and_stalls_resume_without_catch_up() {
        let start = Instant::now();
        let period = Duration::from_millis(10);
        let deadline = start + period;
        assert_eq!(next_frame_deadline(deadline, start, period), deadline);
        let late = deadline + Duration::from_secs(5);
        assert_eq!(next_frame_deadline(deadline, late, period), late + period);
        assert_eq!(
            next_frame_deadline(deadline, deadline + period, period),
            deadline + period * 2
        );
    }
    #[test]
    fn wheel_units_are_logical_pixels_with_browser_sign() {
        for scale in [1., 2.] {
            assert_eq!(
                wheel_delta(MouseScrollDelta::LineDelta(0.25, -1.), scale),
                (-10., 40.)
            );
            assert_eq!(
                wheel_delta(
                    MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                        3.5 * scale,
                        -12.25 * scale
                    )),
                    scale
                ),
                (-3.5, 12.25)
            );
        }
    }
}
