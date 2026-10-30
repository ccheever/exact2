use exact_ink::counters;
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
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, PhysicalKey},
    platform::windows::{EventLoopBuilderExtWindows, WindowAttributesExtWindows},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{CursorIcon, Fullscreen, Theme, Window, WindowId},
};

struct App<D: DataSource> {
    service_causes: u8,
    proxy: EventLoopProxy<crate::accessibility::Event>,
    accessibility: Option<crate::accessibility::Access>,
    name: String,
    min_size: (f32, f32),
    config: Config,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    preparing: Option<exact_ink::device::d3d11::Startup>,
    jump_list_started: bool,
    direct: bool,
    presenter: Option<Presenter<D>>,
    timers: crate::timers::Timers,
    started: Instant,
    pointer: (f32, f32),
    pointer_inside: bool,
    focused: bool,
    cursor: CursorIcon,
    buttons: u32,
    clicks: crate::clicks::Clicks,
    scale: f64,
    refit_after_scale: bool,
    next_frame: Instant,
    redraw_pending: bool,
    failed: bool,
    minimized: bool,
    occluded: bool,
    frames: u64,
    measured_first_screen: bool,
    measure_first_composition: bool,
    paint_ms: f64,
    present_ms: f64,
    previous_frame: Option<Instant>,
    samples: Vec<(f64, f64, f64)>,
    frame_logs: Vec<String>,
    frame_limit: Option<u64>,
    first_tap: Option<String>,
    pads: crate::pad_poll::Pads,
    pads_active: bool,
    caret: crate::caret::Caret,
    edit_menu: Option<exact_linux::presenter::edit_menu::EditMenu>,
    ime: crate::ime::InputMethod,
    appearance: crate::appearance::Changes,
    modal: crate::modal::Schedule,
    desktop: Rc<crate::desktop::Desktop>,
}

impl<D: DataSource> Drop for App<D> {
    fn drop(&mut self) {
        crate::probe::save();
        if std::env::var("EXACT_AGENT_WINDOW_PROBE").as_deref() == Ok("on") {
            eprintln!("exact-windows: successful frames {}", self.frames);
        }
        self.timers.clear();
        self.accessibility.take();
        // The module's GPU swapchain must release its HWND before the window.
        self.presenter.take();
        self.caret.clear();
    }
}

fn key_names(physical: PhysicalKey, logical: &Key, text: Option<&str>) -> (String, String) {
    let code = match physical {
        PhysicalKey::Code(code) => format!("{code:?}").replace("Super", "Meta"),
        PhysicalKey::Unidentified(_) => "Unidentified".into(),
    };
    let key = match logical {
        Key::Character(s) => s.to_string(),
        Key::Named(k) => format!("{k:?}"),
        _ => text.unwrap_or("Unidentified").to_owned(),
    };
    (code, key)
}

fn system_theme<D: DataSource>(p: &mut Presenter<D>, theme: Theme) {
    let dark = theme == Theme::Dark;
    p.set_system_scheme(dark);
    let contrast = crate::appearance::contrast();
    p.set_contrast_palette(contrast);
    let mut preferences = p.host().runner().viewport().preferences;
    let custom = if contrast.is_some() {
        exact_runner::Contrast::Custom
    } else {
        exact_runner::Contrast::NoPreference
    };
    if preferences.dark != dark || preferences.contrast != custom {
        preferences.contrast = custom;
        preferences.dark = dark;
        if let Some(error) = p.set_preferences(preferences) {
            eprintln!("exact-windows: appearance: {error}");
        }
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

struct FrameSchedule {
    cadence: Option<Instant>,
    retry: Option<Instant>,
    present: Option<Instant>,
}

fn presentation_resumes(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::Occluded(false)
            | WindowEvent::Focused(true)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
    )
}

impl FrameSchedule {
    fn admit(&self, now: Instant) -> Instant {
        self.cadence
            .into_iter()
            .chain(self.present)
            .fold(now, Instant::max)
    }

    fn defer(&self, now: Instant, pending: &mut bool) -> bool {
        *pending = self.admit(now) > now;
        *pending
    }

    fn redraw(&self, now: Instant, requested: bool) -> Option<Instant> {
        let requested = if requested { Some(now) } else { self.retry }?;
        Some(requested.max(self.admit(now)))
    }
}

fn direct_device(painter: exact_linux::presenter::PainterChoice, device: Option<&str>) -> bool {
    painter == exact_linux::presenter::PainterChoice::Ink
        && exact_ink::device::direct_d3d11(device).is_some()
}

pub(crate) fn boot_phase(name: &str) {
    if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
        if let Ok(ms) = exact_ink::device::process_ms() {
            eprintln!("exact-windows: boot {name} at {ms:.3} ms (process)");
        }
    }
}

pub(super) fn run<D: DataSource + Default + 'static>(
    name: &str,
    opening: crate::Window,
    plan: &[u8],
    compat: &str,
    composed: exact_linux::Composed,
) -> i32 {
    let started = Instant::now();
    app::prepare_environment(compat);
    counters::initialize();
    boot_phase("host-entered");
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
    let device = std::env::var("EXACT_INK_DEVICE").ok();
    let direct = direct_device(
        exact_linux::presenter::PainterChoice::from_env(
            exact_linux::presenter::PainterChoice::from_baked(compat),
        ),
        device.as_deref(),
    );
    let preparing = if direct {
        match exact_ink::device::d3d11::Startup::start(
            exact_ink::device::direct_d3d11(device.as_deref()).unwrap(),
        ) {
            Ok(p) => Some(p),
            Err(error) => {
                eprintln!("exact-windows: {error}");
                return 1;
            }
        }
    } else {
        None
    };
    // A distributed app locates data beside its executable, never in its caller's cwd.
    if std::env::var_os("EXACT_ASSETS").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(root) = exe.parent() {
                std::env::set_var("EXACT_ASSETS", root);
            }
        }
    }
    if std::env::var_os("EXACT_GPU_RENDER").is_none() {
        std::env::set_var("EXACT_GPU_RENDER", "1");
    }
    // Windows has no toolkit under this painter, so it shows the web's
    // neutral look (LLP 1115.000 D17), in a window as in a headless run.
    app::show_neutral_look();
    if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some()
        && std::env::var("EXACT_AGENT_INK_FAULT").as_deref() == Ok("slow-config")
    {
        std::thread::sleep(Duration::from_millis(200));
    }
    let mut config = Config::from_env(plan, compat);
    // The presenter makes the hatches' value after its first frame is shown.
    config.hatches = composed.hatches;
    config.packages = composed.packages;
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
    if first_tap.is_some()
        && frame_limit.is_none()
        && std::env::var_os("EXACT_AGENT_WINDOW_READY").is_none()
    {
        eprintln!("exact-windows: EXACT_AGENT_WINDOW_TAP requires a measured window run");
        return 1;
    }
    if !config.windowed() {
        drop(preparing);
        return app::run_config::<D>(&mut config, started);
    }
    boot_phase("config-ready");
    if std::env::var_os("EXACT_SIZE").is_none() {
        config.size = (opening.width, opening.height);
    }
    let mut builder = EventLoop::<crate::accessibility::Event>::with_user_event();
    let ime = crate::ime::InputMethod::default();
    let desktop = crate::desktop::Desktop::install(name);
    let ime_message = ime.message_hook();
    let desktop_message = desktop.clone();
    // Winit keeps one hook; replacing it would lose composition completion on clicks.
    builder.with_msg_hook(move |message| {
        ime_message(message);
        desktop_message.message(message);
        false
    });
    let event_loop = match builder.build() {
        Ok(value) => value,
        Err(e) => {
            eprintln!("exact-windows: {e}");
            return 1;
        }
    };
    boot_phase("event-loop-ready");
    let proxy = event_loop.create_proxy();
    let wake = proxy.clone();
    desktop.set_wake(move || {
        let _ = wake.send_event(crate::accessibility::Event::Wake);
    });
    let wake = proxy.clone();
    exact_linux::set_event_waker(Some(Arc::new(move || {
        let _ = wake.send_event(crate::accessibility::Event::Wake);
    })));
    let timer_proxy = proxy.clone();
    let timers = crate::timers::Timers::new(move |fired| {
        let _ = timer_proxy.send_event(crate::accessibility::Event::Timer(fired));
    });
    let mut state = App::<D> {
        service_causes: 0,
        proxy,
        accessibility: None,
        name: name.to_owned(),
        min_size: (opening.min_width, opening.min_height),
        config,
        window: None,
        surface: None,
        preparing,
        jump_list_started: false,
        direct,
        presenter: None,
        timers,
        started,
        pointer: (0., 0.),
        pointer_inside: false,
        focused: true,
        cursor: CursorIcon::Default,
        clicks: Default::default(),
        buttons: 0,
        scale: 1.,
        next_frame: started,
        redraw_pending: false,
        failed: false,
        minimized: false,
        occluded: false,
        frames: 0,
        measured_first_screen: false,
        measure_first_composition: false,
        paint_ms: 0.,
        present_ms: 0.,
        previous_frame: None,
        samples: Vec::new(),
        frame_logs: Vec::new(),
        frame_limit,
        first_tap,
        pads: Default::default(),
        pads_active: false,
        caret: Default::default(),
        edit_menu: None,
        ime,
        appearance: Default::default(),
        modal: Default::default(),
        desktop,
        refit_after_scale: false,
    };
    if let Err(e) = event_loop.run_app(&mut state) {
        eprintln!("exact-windows: {e}");
        state.failed = true;
    }
    exact_linux::set_event_waker(None);
    // However the loop ended (the window closed, a bounded run, a failure): the store's writes, made on a thread of
    // their own, are on disk before the process ends, and what failed is said.
    if !exact_linux::host::drain_store_writes(std::time::Duration::from_secs(5)) {
        eprintln!("exact: the store's writes were not all on disk at exit");
    }
    if let Some(p) = &state.presenter {
        for line in exact_linux::host::take_store_warnings(p.host().runner().data_ref().app_id()) {
            eprintln!("exact: {line}");
        }
    }
    if state.frame_limit.is_some()
        && std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some()
        && !state.measured_first_screen
    {
        eprintln!("exact-windows: the first-screen readiness target was not painted");
        state.failed = true;
    }
    if state.frames > 0 {
        eprintln!(
            "exact-windows: {} frames; mean paint {:.2} ms; mean present {:.2} ms",
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
            eprintln!("exact-windows: recent {} frames p50/p95 ms: interval {:.2}/{:.2}, paint {:.2}/{:.2}, present {:.2}/{:.2}",
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
        self.caret.clear();
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
    fn deliver_timer(&mut self, fired: Arc<crate::timers::Fired>, now: f64) {
        if let Some(p) = &mut self.presenter {
            if fired.live(p.native_timer_generation()) {
                let due = p
                    .host()
                    .runner()
                    .native_timers()
                    .into_iter()
                    .find(|row| {
                        row.index == fired.row.index && row.generation == fired.row.generation
                    })
                    .map(|row| row.due_ms);
                if let Some(due) = due.filter(|due| *due > now) {
                    self.timers.defer(due, fired);
                    return;
                }
                if let Some(e) = p.fire_native_timer(fired.epoch, &fired.row, now) {
                    eprintln!("exact-windows: {e}");
                }
            }
        }
        fired.complete();
    }
    fn draw(&mut self, events: &ActiveEventLoop) -> Result<(), String> {
        if std::mem::take(&mut self.refit_after_scale) {
            if let Some(window) = &self.window {
                // Winit has applied its DPI size by the next redraw. It preserves
                // logical size even when that puts content below the desktop.
                if crate::placement::fit(window, self.min_size) {
                    self.resized();
                }
            }
        }
        let frame_started = Instant::now();
        if self.frames == 0 {
            boot_phase("first-draw-entered");
        }
        if self.minimized || self.occluded {
            return Ok(());
        }
        // A frame task can wake the loop from its own commit. Those wakes
        // must not advance the animation again before the next display tick.
        let continuous = self.frame_limit.is_some()
            || self
                .presenter
                .as_ref()
                .is_some_and(|p| p.wants_display_frames() || p.host().motion());
        let unpaced =
            self.frame_limit.is_some() && std::env::var_os("EXACT_AGENT_WINDOW_UNPACED").is_some();
        let schedule = FrameSchedule {
            cadence: (continuous && !unpaced).then_some(self.next_frame),
            retry: None,
            present: self
                .presenter
                .as_ref()
                .and_then(|p| p.painter_present_deadline()),
        };
        // A refused present defers the next paint. Ordinary D3D11 redraws acquire
        // the latency waitable before painting; they need no second timer delay.
        if schedule.defer(frame_started, &mut self.redraw_pending) {
            return Ok(());
        }
        let now = self.now();
        let (Some(window), surface, Some(p)) =
            (&self.window, &mut self.surface, &mut self.presenter)
        else {
            return Ok(());
        };
        if self.measure_first_composition {
            // Read before submitting another frame; the compositor has had the
            // intervening display period. Omitted in the probe-free interval run.
            match p.displayed_process_ms() {
                Ok(Some(ms)) => eprintln!(
                    "exact-windows: process to displayed first composition {ms:.3} ms (DXGI)"
                ),
                other => {
                    eprintln!("exact-windows: first composition timestamp unavailable: {other:?}")
                }
            }
            self.measure_first_composition = false;
        }
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        // A restore or DPI transition can expose the new client extent before
        // its resize event reaches this callback. Never present the old extent.
        let scale = window.scale_factor();
        let viewport = (
            size.width as f32 / scale as f32,
            size.height as f32 / scale as f32,
        );
        if self.scale != scale || p.viewport() != viewport {
            self.caret.clear();
            self.scale = scale;
            if let Some(error) = p.resize_scaled(viewport.0, viewport.1, scale as f32) {
                return Err(error);
            }
        }
        counters::service(counters::DRAW);
        let service_span = crate::probe::Span::new("draw service", counters::DRAW);
        p.pump(now);
        // A presented frame: its tasks, then the hatches' ticks and due
        // `after`s (LLP 1075.003.000.001 §2.4).
        p.animation_frame(now);
        p.run_commands(D::default);
        crate::orientation::apply(p);
        // The hatches' turn: what they asked of elements runs, then the
        // moments this turn's commits and the window's new size caused; an
        // overlay published here is in the frame painted below.
        p.hatch_turn();
        let prepared_at = frame_started.elapsed().as_secs_f64() * 1000.;
        p.poll_images();
        drop(service_span);
        // The OS draws the desktop cursor; this presenter's arrow is for DRM.
        p.set_pointer(None);
        if self.frames > 0 {
            p.sync_surfaces();
        }
        let images_at = frame_started.elapsed().as_secs_f64() * 1000.;
        p.tick(now);
        let tick_at = frame_started.elapsed().as_secs_f64() * 1000.;
        self.caret.advance(frame_started);
        p.set_caret_visible(self.caret.visible());
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
        // Bounded runs measure an unchanged redraw, as the C4 still protocol does.
        if self.frame_limit.is_some() {
            p.repaint();
        }
        let painting = p.dirty();
        let paint = Instant::now();
        if let Some(previous) = self.caret.take_previous_frame() {
            p.recycle_frame(previous);
        }
        let RawWindowHandle::Win32(handle) =
            window.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("not a Windows window".into());
        };
        let owned = exact_ink::device::d3d11::Window::from_owner(window.clone())
            .map_err(|e| e.to_string())?;
        let mut frame = p.window_frame(owned.clone())?;
        let caret_rect = (self.focused && p.last_frame_succeeded())
            .then(|| p.caret_frame())
            .flatten();
        if self.caret.presented(
            handle.hwnd.get(),
            caret_rect,
            self.scale as f32,
            frame.clone(),
            frame_started,
        ) {
            p.set_caret_visible(true);
            // A programmatic move while hidden starts a fresh visible phase.
            frame = p.window_frame(owned)?;
            self.caret.presented(
                handle.hwnd.get(),
                caret_rect,
                self.scale as f32,
                frame.clone(),
                frame_started,
            );
        }
        // Geometry and accessibility queries below can paint into a discarded
        // frame. Preserve work discovered after this frame's pixels were drawn.
        let repaint_owed = p.dirty();
        let direct_present_ms = if painting && frame.is_none() && p.painter.name == "ink" {
            p.last_frame_ms().map_or(0., |(_, present)| present)
        } else {
            0.
        };
        let paint_ms = (paint.elapsed().as_secs_f64() * 1000. - direct_present_ms).max(0.);
        self.paint_ms += paint_ms;
        let present = Instant::now();
        if let Some(frame) = frame {
            let surface = surface
                .as_mut()
                .ok_or("direct painter returned an unpresented frame")?;
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
            p.recycle_frame(frame);
        }
        if !p.last_frame_succeeded() {
            return Ok(());
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
        let present_ms = present.elapsed().as_secs_f64() * 1000. + direct_present_ms;
        let presented_at = frame_started.elapsed().as_secs_f64() * 1000.;
        let render_ms = p.last_frame_ms().map_or(0., |(render, _)| render);
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
        if self.frames == 0 {
            eprintln!(
                "exact-windows: first present returned at {:.3} ms after host start",
                self.started.elapsed().as_secs_f64() * 1000.
            );
        }
        // Activation can leave the baked picture unchanged. A clean frame need
        // not present again just to acknowledge that its data source is ready.
        p.first_pixel();
        let activated_at = frame_started.elapsed().as_secs_f64() * 1000.;
        // A screen's ready marker can exclude background requests that do not
        // contribute to it. Shelf's library-count appears with its seeded books.
        let screen_ready = std::env::var("EXACT_AGENT_WINDOW_READY").map_or_else(
            |_| !p.pending(),
            |target| p.host().kernel().find_by_test_id(&target).len() == 1,
        );
        if (self.frame_limit.is_some()
            || std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some()
            || std::env::var("EXACT_AGENT_WINDOW_PROBE").as_deref() == Ok("on"))
            && !self.measured_first_screen
            && p.last_frame_succeeded()
            && !p.dirty()
            && !p.data_activating()
            && screen_ready
        {
            if std::env::var("EXACT_AGENT_WINDOW_PROBE").as_deref() == Ok("off") {
                eprintln!("exact-windows: complete first screen submitted (probe off)");
            } else {
                let probe = Instant::now();
                let synchronized = exact_ink::device::compositor_sync_process_ms();
                let deadline = Instant::now() + Duration::from_secs(1);
                let displayed = loop {
                    let displayed = p.displayed_process_ms();
                    if displayed.is_ok() || Instant::now() >= deadline {
                        break displayed;
                    }
                    // The composition statistics can lag a DWM sync. Do not accept
                    // an older present as the complete screen's displayed frame.
                    if exact_ink::device::compositor_sync_process_ms().is_err() {
                        break displayed;
                    }
                };
                match displayed {
                    Ok(Some(ms)) => {
                        eprintln!(
                            "exact-windows: process to displayed first screen {ms:.3} ms (DXGI)"
                        )
                    }
                    Err(error) => {
                        eprintln!("exact-windows: displayed-frame timestamp unavailable: {error}")
                    }
                    Ok(None) => {}
                }
                match synchronized {
                    Ok(ms) => eprintln!(
                    "exact-windows: process to first-screen compositor sync {ms:.3} ms (DWM proxy)"
                ),
                    Err(error) => eprintln!("exact-windows: compositor sync: {error}"),
                }
                eprintln!(
                    "exact-windows: synchronization probe {:.3} ms",
                    probe.elapsed().as_secs_f64() * 1000.
                );
            }
            eprintln!(
                "exact-windows: first-screen painter {} adapter {:?}",
                p.painter.name, p.painter.adapter
            );
            self.measured_first_screen = true;
        }
        self.frames += 1;
        self.measure_first_composition = self.frames == 1
            && surface.is_none()
            && self.frame_limit.is_some()
            && std::env::var("EXACT_AGENT_WINDOW_PROBE").as_deref() != Ok("off");
        if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
            let line = format!("exact-windows: frame {} at {:.3} ms, {}x{}, scale {}, painter {}, caret {}, paint {:.3} ms, present {:.3} ms",
                self.frames, now, size.width, size.height, self.scale, p.painter.name,
                self.caret.visible(), paint_ms, present_ms);
            if self.frame_limit.is_some() {
                self.frame_logs.push(line);
            } else {
                eprintln!("{line}");
            }
        }
        let logged_at = frame_started.elapsed().as_secs_f64() * 1000.;
        if !self.jump_list_started && p.last_frame_succeeded() {
            self.jump_list_started = true;
            crate::jump_list::start(self.config.compat.clone());
        }
        if let Some(access) = &mut self.accessibility {
            access.sync(p, window, &self.name);
        }
        let accessible_at = frame_started.elapsed().as_secs_f64() * 1000.;
        self.ime.sync(window, p);
        let ime_at = frame_started.elapsed().as_secs_f64() * 1000.;
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
        if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
            let line = format!("exact-windows: phases {} prepare {prepared_at:.3}, images {images_at:.3}, tick {tick_at:.3}, presented {presented_at:.3}, logged {logged_at:.3}, activated {activated_at:.3}, accessibility {accessible_at:.3}, ime {ime_at:.3}, surfaces {:.3} ms from frame start; render {render_ms:.3} ms",
                self.frames, frame_started.elapsed().as_secs_f64() * 1000.);
            if self.frame_limit.is_some() {
                self.frame_logs.push(line);
            } else {
                eprintln!("{line}");
            }
        }
        if repaint_owed || p.dirty() {
            p.repaint();
        }
        if self.frame_limit.is_some_and(|limit| self.frames >= limit) {
            // Bounded measurements write their per-frame log after the timed run.
            // Synchronous file writes can stall even with the DWM probe disabled.
            for line in self.frame_logs.drain(..) {
                eprintln!("{line}");
            }
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
        if self.modal.active() {
            self.about_to_wait(events);
        }
        Ok(())
    }

    fn blink(&mut self, now: Instant) -> Result<(), String> {
        if self.minimized || self.occluded || !self.focused {
            return Ok(());
        }
        let (Some(window), Some(p)) = (&self.window, &mut self.presenter) else {
            return Ok(());
        };
        if p.dirty() || p.wants_display_frames() || p.host().motion() {
            return Ok(());
        }
        if !self.caret.advance(now) {
            return Ok(());
        }
        p.set_caret_visible(self.caret.visible());
        if self.surface.is_none() || !self.caret.can_patch() {
            p.repaint();
            window.request_redraw();
            return Ok(());
        }
        if self.caret.needs_picture() {
            let other = p.frame();
            if !p.last_frame_succeeded() || p.dirty() {
                window.request_redraw();
                return Ok(());
            }
            self.caret.compare(other);
        }
        let mut buffer = self
            .surface
            .as_mut()
            .expect("patch surface")
            .buffer_mut()
            .map_err(|e| e.to_string())?;
        if let Some(damage) = self.caret.paint(&mut buffer, window.inner_size().width) {
            buffer
                .present_with_damage(&[damage])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn show_edit_menu(&mut self) {
        let Some(menu) = self.edit_menu.take() else {
            return;
        };
        let (Some(window), Some(p)) = (&self.window, &mut self.presenter) else {
            return;
        };
        if !p.edit_menu_valid(&menu) {
            return;
        }
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let picked = crate::edit_menu::show(handle.hwnd.get(), &menu, self.scale as f32);
        p.advance(self.started.elapsed().as_secs_f64() * 1000.);
        crate::keyboard::sync(p);
        match picked {
            Ok(Some(action)) => p.edit_menu_action(&menu, action),
            Ok(None) => {}
            Err(error) => eprintln!("exact-windows: {error}"),
        }
        self.caret.activity(Instant::now());
        window.request_redraw();
    }
}

impl<D: DataSource + Default + 'static> ApplicationHandler<crate::accessibility::Event> for App<D> {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if counters::enabled() {
                self.service_causes |= counters::TIMER;
            }
        }
        let _span = crate::probe::Span::new("run loop wake", self.service_causes);
    }
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        boot_phase("resumed");
        let window = match events.create_window(
            Window::default_attributes()
                .with_no_redirection_bitmap(self.direct)
                .with_visible(false)
                .with_title(&self.name)
                .with_fullscreen(
                    (self.frame_limit.is_some()
                        && std::env::var_os("EXACT_AGENT_WINDOW_FULLSCREEN").is_some())
                    .then_some(Fullscreen::Borderless(None)),
                )
                .with_inner_size(LogicalSize::new(self.config.size.0, self.config.size.1))
                .with_min_inner_size(LogicalSize::new(self.min_size.0, self.min_size.1)),
        ) {
            Ok(value) => Rc::new(value),
            Err(e) => {
                self.fail(events, e);
                return;
            }
        };
        boot_phase("window-created");
        crate::placement::fit(&window, self.min_size);
        self.accessibility = Some(crate::accessibility::Access::new(
            events,
            &window,
            self.proxy.clone(),
        ));
        self.focused = window.has_focus();
        self.scale = window.scale_factor();
        self.config.scale = self.scale as f32;
        let size = window.inner_size();
        let viewport = (
            size.width as f32 / self.scale as f32,
            size.height as f32 / self.scale as f32,
        );
        self.config.clock_started = Some(self.started);
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                self.appearance
                    .install(handle.hwnd.get(), self.proxy.clone());
                self.modal.install(handle.hwnd.get(), self.proxy.clone());
                self.desktop.set_window(handle.hwnd.get());
                if let Err(error) = self.clicks.install(handle.hwnd.get()) {
                    self.fail(events, error);
                    return;
                }
            }
        }
        boot_phase("native-hooks-ready");
        let mut direct = false;
        let booted = if let Some(preparing) = self.preparing.take() {
            app::boot_presenter_with_device::<D>(&mut self.config, viewport, preparing)
        } else {
            app::boot_presenter::<D>(&mut self.config, viewport)
        };
        boot_phase("presenter-ready");
        match booted {
            Ok((mut p, warning)) => {
                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::Win32(_) = handle.as_raw() {
                        let owned =
                            match exact_ink::device::d3d11::Window::from_owner(window.clone()) {
                                Ok(owned) => owned,
                                Err(e) => {
                                    self.fail(events, e.to_string());
                                    return;
                                }
                            };
                        let attached = p.attach_painter_window(owned, [size.width, size.height]);
                        match attached {
                            Ok(attached) => direct = attached,
                            Err(e) => {
                                self.fail(events, e);
                                return;
                            }
                        }
                    }
                }
                boot_phase("composition-ready");
                p.host_mut().runner_mut().set_row_reuse(true);
                if let Some(scale) = self.appearance.text_scale() {
                    if let Some(error) = p.set_text_scale(scale) {
                        eprintln!("exact-windows: text size: {error}");
                    }
                }
                if let Some(theme) = window.theme() {
                    system_theme(&mut p, theme);
                }
                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                        p.set_clipboard(Box::new(crate::clipboard::Clipboard(handle.hwnd.get())));
                        let hwnd = handle.hwnd.get();
                        p.set_url_opener(move |url| crate::url::open(hwnd, url));
                    }
                }
                p.surface_lifecycle(self.minimized || self.occluded, !self.focused);
                p.set_window_active(self.focused);
                // The window's clock owns time (the timers RFC, D1)
                p.use_native_timers(true);
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
        if !direct {
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
        }
        boot_phase("before-show");
        window.set_visible(true);
        boot_phase("visible");
        window.request_redraw();
        self.window = Some(window);
        self.observe_monitor();
    }
    fn user_event(&mut self, events: &ActiveEventLoop, event: crate::accessibility::Event) {
        let _span = crate::probe::Span::new("user event", 0);
        if let crate::accessibility::Event::Timer(fired) = event {
            if counters::enabled() {
                self.service_causes |= counters::TIMER;
            }
            self.deliver_timer(fired, self.now());
            self.about_to_wait(events);
            return;
        }
        if matches!(event, crate::accessibility::Event::ModalWake) {
            if counters::enabled() {
                self.service_causes |= counters::TIMER;
            }
            // Requesting another WM_PAINT is insufficient inside a native
            // modal loop. Consume its deadline in this dispatched callback.
            let result = self.draw(events);
            if counters::enabled() {
                self.service_causes &= !counters::TIMER;
                self.service_causes |= counters::DRAW;
            }
            if let Err(error) = result {
                self.fail(events, error);
                return;
            }
            self.about_to_wait(events);
            return;
        }
        // Executor, image and hatch wakes need a display turn; polling the
        // executor alone does not service all three. Timers take the branch above.
        if matches!(event, crate::accessibility::Event::Wake) {
            if counters::enabled() {
                self.service_causes |= counters::WORKER;
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        if let crate::accessibility::Event::Access(event) = event {
            if let (Some(access), Some(p), Some(window)) =
                (&mut self.accessibility, &mut self.presenter, &self.window)
            {
                if event.window_id != window.id() {
                    return;
                }
                p.advance(self.started.elapsed().as_secs_f64() * 1000.);
                access.event(p, window, &self.name, event.window_event);
            }
        }
        self.about_to_wait(events);
    }
    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let _span = crate::probe::Span::new("window event", 0);
        if counters::enabled() {
            self.service_causes |= match event {
                WindowEvent::MouseWheel { .. } => counters::WHEEL,
                WindowEvent::RedrawRequested => counters::DRAW,
                _ => counters::WINDOW,
            };
        }
        if let (Some(access), Some(window)) = (&mut self.accessibility, &self.window) {
            access.process_event(window, &event);
        }
        if presentation_resumes(&event) {
            if let Some(p) = &mut self.presenter {
                p.reset_painter_present_deadline();
            }
        }
        if matches!(event, WindowEvent::CloseRequested) {
            // The window is going: its hatch, then the app's, ends.
            if let Some(p) = &mut self.presenter {
                p.end_hatches();
                p.end_packages();
            }
            events.exit();
            return;
        }
        if matches!(
            event,
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.observe_monitor();
        }
        if matches!(event, WindowEvent::ScaleFactorChanged { .. }) {
            // Winit applies the new pixel extent after this callback. Pairing
            // its new scale with the old extent can clamp a list's scroll
            // position against a viewport that will never be shown.
            self.refit_after_scale = true;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        if matches!(event, WindowEvent::Resized(_)) {
            self.resized();
            return;
        }
        if matches!(event, WindowEvent::RedrawRequested) {
            if let Err(e) = self.draw(events) {
                self.fail(events, e);
            }
            self.show_edit_menu();
            return;
        }
        let button_position = match &event {
            WindowEvent::MouseInput { state, button, .. } => self.clicks.position(*state, *button),
            _ => None,
        };
        let Some(p) = &mut self.presenter else { return };
        p.advance(self.started.elapsed().as_secs_f64() * 1000.);
        // The input's time: where the advance left the clock, a live runner's
        // sample past the time it was asked for
        let now = p.host().now();
        let (x, y) = self.pointer;
        match event {
            WindowEvent::ThemeChanged(theme) => system_theme(p, theme),
            WindowEvent::Moved(_) => self.ime.reposition(),
            WindowEvent::ModifiersChanged(_) => crate::keyboard::sync(p),
            WindowEvent::Focused(focused) => {
                self.clicks.clear();
                if !focused {
                    self.caret.clear();
                }
                if focused {
                    p.refresh_scrollbar_preferences();
                    p.set_window_active(true);
                    crate::keyboard::sync(p);
                }
                self.focused = focused;
                p.surface_lifecycle(self.minimized || self.occluded, !focused);
                if !focused {
                    self.buttons = 0;
                    let _ = p.pointer_lost(now);
                    p.set_window_active(false);
                    // Unfocused, the pads are not read: what they were is no longer known.
                    p.forget_pads();
                    self.pads.forget();
                }
            }
            WindowEvent::Occluded(occluded) => {
                if occluded {
                    self.caret.clear();
                }
                self.occluded = occluded;
                p.surface_lifecycle(self.minimized || occluded, !self.focused);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.buttons & 1 != 0 {
                    self.clicks.moved((position.x, position.y));
                }
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
                let (x, y) = if let Some((px, py)) = button_position {
                    self.pointer = (px as f32 / self.scale as f32, py as f32 / self.scale as f32);
                    self.pointer_inside = self.window.as_ref().is_some_and(|window| {
                        let size = window.inner_size();
                        px >= 0 && py >= 0 && (px as u32) < size.width && (py as u32) < size.height
                    });
                    self.pointer
                } else {
                    (x, y)
                };
                let down = state == ElementState::Pressed;
                if down {
                    self.caret.activity(Instant::now());
                    if button != MouseButton::Left {
                        self.clicks.clear();
                    }
                }
                let bit = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 4,
                    _ => 0,
                };
                let released = !down && self.buttons & bit != 0;
                if down {
                    self.buttons |= bit;
                } else {
                    self.buttons &= !bit;
                }
                match button {
                    MouseButton::Left => {
                        let _ = if down {
                            let clicks = self.clicks.down(
                                (f64::from(x) * self.scale, f64::from(y) * self.scale),
                                Instant::now(),
                            );
                            p.pointer_down_with_clicks(x, y, now, clicks)
                        } else {
                            p.pointer_up(x, y, now)
                        };
                    }
                    MouseButton::Right => {
                        p.pointer_aux(2, down, x, y, now);
                        if released && self.buttons == 0 {
                            self.edit_menu = p.edit_menu_at((x, y));
                        }
                    }
                    MouseButton::Middle => p.pointer_aux(4, down, x, y, now),
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.clicks.clear();
                crate::wheel::dispatch(p, (x, y), delta, self.scale);
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                self.clicks.clear();
                // A key already down as the window gains focus (winit's
                // synthetic press) is held, not pressed here: a browser fires
                // no keydown for it, and it is no keyboard input to WinUI's
                // focus visual. Its release still arrives as a key's.
                if is_synthetic && event.state == ElementState::Pressed {
                    let (code, _) = key_names(event.physical_key, &event.logical_key, None);
                    p.hold_modifier(&code, true);
                    return;
                }
                if event.state == ElementState::Pressed {
                    self.caret.activity(Instant::now());
                }
                // Unicode input (VK_PACKET) has text but no physical key.
                let (code, key) = key_names(
                    event.physical_key,
                    &event.logical_key,
                    event.text.as_deref(),
                );
                if self.ime.takes_keys() {
                    p.hold_modifier(&code, event.state == ElementState::Pressed);
                    return;
                }
                let (handled, menu) = if event.state == ElementState::Pressed {
                    p.edit_menu_key(&code, &key, event.repeat)
                } else {
                    (false, None)
                };
                if handled {
                    self.edit_menu = menu;
                } else {
                    p.hardware_key(
                        &code,
                        &key,
                        event.state == ElementState::Pressed,
                        event.repeat,
                    );
                }
            }
            WindowEvent::Ime(event) => {
                self.caret.activity(Instant::now());
                self.ime.event(p, event);
            }
            _ => return,
        }
        if let Some(window) = &self.window {
            self.ime.sync(window, p);
            if (!self.pointer_inside || !self.focused) && self.cursor != CursorIcon::Default {
                window.set_cursor(CursorIcon::Default);
                self.cursor = CursorIcon::Default;
            }
            window.request_redraw();
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        self.wait(events, |p| {
            (p.painter_retry_deadline(), p.painter_present_deadline())
        });
    }
}

impl<D: DataSource + Default + 'static> App<D> {
    fn observe_monitor(&self) {
        if counters::enabled() {
            let hz = self
                .window
                .as_ref()
                .and_then(|w| w.current_monitor())
                .and_then(|m| m.refresh_rate_millihertz())
                .unwrap_or(60_000);
            counters::refresh_rate(hz);
        }
    }

    fn wait(
        &mut self,
        events: &ActiveEventLoop,
        deadlines: impl FnOnce(&Presenter<D>) -> (Option<Instant>, Option<Instant>),
    ) {
        let causes = if counters::enabled() {
            std::mem::take(&mut self.service_causes)
        } else {
            0
        };
        counters::service(causes);
        let _span = crate::probe::Span::new("carrier service", causes);
        crate::probe::sample();
        let now = self.now();
        for fired in self.timers.take_due(now) {
            self.deliver_timer(fired, now);
        }
        if let Some(p) = self.presenter.as_mut() {
            p.timer_visibility(self.minimized || self.occluded);
            let now = self.started.elapsed().as_secs_f64() * 1000.0;
            if p.wake_due_ms().is_some_and(|due| due <= now) {
                if let Some(e) = p.advance(now) {
                    eprintln!("exact-windows: {e}");
                }
            }
            // Deliver nonvisual effects even when this turn needs no presentation
            if let Some(e) = p.pump(self.started.elapsed().as_secs_f64() * 1000.0) {
                eprintln!("exact-windows: {e}");
            }
            p.run_commands(D::default);
            crate::orientation::apply(p);
            p.hatch_turn();
            if let Some(e) = p.admit_idle_timer(self.started.elapsed().as_secs_f64() * 1000.0) {
                eprintln!("exact-windows: {e}");
            }
        }
        if self.appearance.take() {
            if let (Some(window), Some(p)) = (&self.window, &mut self.presenter) {
                system_theme(p, window.theme().unwrap_or(Theme::Light));
                if let Some(scale) = self.appearance.text_scale() {
                    if let Some(error) = p.set_text_scale(scale) {
                        eprintln!("exact-windows: text size: {error}");
                    }
                }
                window.request_redraw();
            }
        }
        if let Err(error) = self.blink(Instant::now()) {
            self.fail(events, error);
            return;
        }
        let Some(p) = &mut self.presenter else { return };
        let retirement = p.collect_painter_retirement(Instant::now());
        let pads_active = self.focused && !self.minimized && p.has_gamepad_input();
        if self.pads_active != pads_active {
            self.pads_active = pads_active;
            self.pads.forget();
            p.forget_pads();
        }
        if pads_active {
            let at = self.started.elapsed().as_secs_f64() * 1000.;
            self.pads.poll(
                Instant::now(),
                crate::pads::read,
                |index, connected, buttons, axes| {
                    crate::pads::register(index, connected, buttons, axes);
                    p.hardware_pad(index, connected, buttons, axes, at)
                },
            );
        }
        if let Err(e) = self.timers.update(
            p.native_timer_generation(),
            p.host().runner().native_timers(),
            self.started.elapsed().as_secs_f64() * 1000.0,
        ) {
            self.fail(events, e);
            return;
        }
        if let Some(due) = p.wake_due_ms() {
            let deadline = self.started + Duration::from_secs_f64((due / 1000.).max(0.));
            events.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            events.set_control_flow(ControlFlow::Wait);
        }
        let continuous =
            self.frame_limit.is_some() || p.wants_display_frames() || p.host().motion();
        let unpaced =
            self.frame_limit.is_some() && std::env::var_os("EXACT_AGENT_WINDOW_UNPACED").is_some();
        let (retry, present) = deadlines(p);
        let schedule = FrameSchedule {
            cadence: (continuous && !unpaced).then_some(self.next_frame),
            retry,
            present,
        };
        let now = Instant::now();
        let redraw = (!self.minimized && !self.occluded)
            .then(|| schedule.redraw(now, p.dirty() || continuous || self.redraw_pending))
            .flatten();
        if let Some(due) = redraw {
            if now >= due {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= due) {
                events.set_control_flow(ControlFlow::WaitUntil(due));
            }
        }
        if let Some(due) = self.timers.next_delivery() {
            let deadline = self.started + Duration::from_secs_f64(due / 1000.0);
            if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= deadline) {
                events.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
        if let Some(due) = retirement {
            if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= due) {
                events.set_control_flow(ControlFlow::WaitUntil(due));
            }
        }
        // Only an app with an input canvas needs controller discovery wakes.
        if pads_active {
            let soon = self.pads.next_poll(Instant::now());
            if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= soon) {
                events.set_control_flow(ControlFlow::WaitUntil(soon));
            }
        }
        if self.focused
            && !self.occluded
            && !self.minimized
            && !p.dirty()
            && !p.wants_display_frames()
            && !p.host().motion()
        {
            if let Some(soon) = self.caret.deadline() {
                if !matches!(events.control_flow(), ControlFlow::WaitUntil(t) if t <= soon) {
                    events.set_control_flow(ControlFlow::WaitUntil(soon));
                }
            }
        }
        self.modal.wait_until(match events.control_flow() {
            ControlFlow::WaitUntil(deadline) => Some(deadline),
            _ => None,
        });
    }
}

#[cfg(test)]
pub(crate) mod tests;
