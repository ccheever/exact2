//! What the host tells the app about the device and its user, after boot:
//! the date (LLP 1027.000.000), the display preferences (LLP 1061 D5; LLP
//! 1069.000 D1) and the page's facts (LLP 1069.000 D2). The presenter reads
//! no system setting itself. On Android the reader reads the device's and
//! tells them whole ([`Presenter::set_device`], LLP 1115.001 D3); an agent
//! sets them one at a time (`prefer`). Until either speaks, as on desktop
//! Linux, the app is told no preference, a light system, a visible page,
//! online, with focus (the app is the whole display: no other window can take
//! it), and no share sheet.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Report the place and seed together, including on the first frame.
    pub fn set_place(&mut self, place: &exact_runner::time::Place) -> Option<String> {
        if let Some(error) = self.host.set_place(place) {
            return Some(error);
        }
        self.after_commit()
    }

    /// A candidate has a new runner, but belongs to the same launch.
    pub(super) fn restore_time(&self, host: &mut Host<D>) -> Result<(), HostError> {
        host.set_scheme(self.dark());
        // The same launch keeps what the device said about itself.
        let runner = self.host.runner();
        let (preferences, page) = (runner.viewport().preferences, runner.page());
        if let Some(error) = host.set_preferences(preferences) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_page(page) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_root_font_size(runner.host_root_font_size()) {
            return Err(HostError::Layout(error));
        }
        if let Some(styles) = self.host.kernel().env().control_text_styles.clone() {
            host.set_control_text_styles(styles)
                .map_err(HostError::Layout)?;
        }
        if let Some(error) = host.set_display_scale(runner.display_scale()) {
            return Err(HostError::Layout(error));
        }
        let time = self.host.runner().wall_time();
        if let Some(error) = host.set_place(self.host.runner().place()) {
            return Err(HostError::Layout(error));
        }
        if let Some(error) = host.set_time(time.epoch_at_zero, time.utc_offset) {
            return Err(HostError::Layout(error));
        }
        Ok(())
    }

    /// The date, as the clock `performanceNow()` reads: Unix ms at clock zero (the
    /// runner's clock starts at boot) and the local zone's offset, in
    /// minutes east of UTC, as Apple and the web read theirs (LLP 1054 R12).
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> Option<String> {
        let error = self.host.set_time(epoch_at_zero, utc_offset);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// The local zone's offset now, told when it is not the runner's: the
    /// display loop asks before each advance, so a DST change or a new zone
    /// reaches the timer that fires after it (habits F6). An agent drive's
    /// offset is the drive's (`agent.rs` `retell_offset`), never this.
    pub fn follow_local_offset(&mut self) -> Option<String> {
        let time = self.host.runner().wall_time();
        let offset = crate::zone::local_offset_minutes();
        if time.epoch_at_zero <= 0.0 || offset == time.utc_offset {
            return None;
        }
        self.set_time(time.epoch_at_zero, offset)
    }

    /// `prefers-reduced-motion`, `-reduced-transparency`, `-contrast` and
    /// `-color-scheme`, as `exactViewport()` answers them: re-answered in
    /// one commit.
    pub fn set_preferences(&mut self, preferences: exact_runner::Preferences) -> Option<String> {
        let error = self.host.set_preferences(preferences);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// `visibilityState`, `onLine`, `canShare`, `canOpenFiles` and
    /// `hasFocus`, as `exactPage()` answers them: re-answered in one commit.
    pub fn set_page(&mut self, page: exact_runner::Page) -> Option<String> {
        let error = self.host.set_page(page);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// A live design batch or snapshot frame (LLP 1121 stage 4), in place;
    /// a colour the full wide table refused is reported on the build
    /// channel (LLP 1122 D4), or to stderr with none.
    pub fn apply_live(&mut self, frame: &str) -> Option<String> {
        // The trace's (EXACT_SCRUB_TRACE): when taken, and the design before,
        // so a frame the runner drops (an older one) is no line.
        let traced = scrub_trace().then(|| (unix_ms(), self.host.runner().design()));
        let (error, full, roles) = self.host.apply_live(frame);
        // Nothing kept from a paint under the old overrides stands (LLP 1121 stage 5).
        if roles {
            self.brush.colors_changed();
            self.dirty = true;
        }
        if full {
            self.report_unsupported(
                "batch",
                Some("a colour was refused (the wide colour table is full)"),
            );
        }
        if error.is_some() {
            return error;
        }
        let after = self.after_commit();
        if let Some((taken, before)) = traced {
            let applied = unix_ms();
            if self.host.runner().design() != before {
                trace_scrub(frame, taken, applied);
            }
        }
        after
    }

    /// App modes (LLP 1121 D5): `setMode` from the agent's `prefer mode`.
    pub fn set_modes(&mut self, pins: &[(&str, &str)]) -> Option<String> {
        let error = self.host.set_modes(pins);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// The root font size `rem` lengths follow (LLP 1069.000 D3): 16, a
    /// browser's `medium`, until the device's shell says its font scale
    /// ([`Presenter::set_device`]: 16 times it) or an agent a size.
    pub fn set_root_font_size(&mut self, px: f64) -> Option<String> {
        let error = self.host.set_root_font_size(px);
        if error.is_some() {
            return error;
        }
        self.after_commit()
    }

    /// `devicePosture` and the viewport segments (LLP 1078 D7): what the
    /// agent's `prefer posture` and `prefer segments` set on this host,
    /// which has no fold of its own; kept for `layout.env`.
    pub fn set_segments(
        &mut self,
        posture: exact_runner::Posture,
        cols: u32,
        rows: u32,
        rects: Vec<exact_kernel::Rect>,
    ) -> Option<String> {
        let error = self.host.set_segments(posture, cols, rows, rects.clone());
        if error.is_some() {
            return error;
        }
        self.segments = rects;
        self.dirty = true;
        self.after_commit()
    }

    /// The safe-area insets (top, right, bottom, left; points) the window's
    /// edges reserve: a host drawing under a status bar reports them here.
    pub fn set_safe_area(&mut self, insets: [f32; 4]) -> Option<String> {
        match self.host.set_safe_area(insets) {
            Ok(true) => self.after_commit(),
            Ok(false) => None,
            Err(e) => Some(e),
        }
    }

    /// The system's appearance, which `setScheme("system")` follows.
    pub fn set_system_scheme(&mut self, dark: bool) {
        self.scheme.1 = dark;
        self.apply_scheme();
    }

    /// Report an individual view's appearance (LLP 1062 D4).
    pub fn set_view_scheme(&mut self, view: ViewId, dark: bool) {
        self.host.set_view_scheme(view, dark);
        self.dirty = true;
    }

    pub(crate) fn app_scheme(&mut self, scheme: Option<bool>) {
        self.scheme.0 = scheme;
        self.apply_scheme();
    }

    /// What a `light-dark()` colour resolves to here (LLP 1034 D2).
    fn apply_scheme(&mut self) {
        let dark = self.scheme.0.unwrap_or(self.scheme.1);
        self.dirty |= self.brush.dark != dark;
        self.host.set_scheme(dark);
        self.brush.dark = dark;
        if let Some(error) = self.host.content_region_appearance(dark) {
            self.host.log(error);
        }
    }

    /// The scheme a `light-dark()` colour resolves to now.
    pub fn dark(&self) -> bool {
        self.brush.dark
    }

    /// A request for MIDI on the display's clock: this host has none (LLP
    /// 1125 D6). Under the agent's clock the runner answered it already.
    pub(super) fn midi_unavailable(&mut self) {
        if self.live_clock.is_none() {
            return;
        }
        let error = self
            .host
            .midi_host(r#"{"ports":{"state":"unavailable"}}"#)
            .or_else(|| self.after_commit());
        if let Some(e) = error {
            self.host.log(format!("midi: {e}"));
        }
    }

    /// `prefer extension` (LLP 1124 D7): the runner's, committed as `midi_agent`'s is.
    pub fn extension_agent(&mut self, request: &str) -> Option<Result<String, String>> {
        let reply = self.host.extension_agent(request)?;
        Some(match self.after_commit() {
            Some(e) => Err(e),
            None => reply,
        })
    }

    /// The agent's `prefer midi.*` and `tap midi` (LLP 1125 §4): `None` for
    /// another request, else the reply or the refusal.
    pub fn midi_agent(&mut self, request: &str) -> Option<Result<String, String>> {
        let reply = self.host.midi_agent(request)?;
        Some(match self.after_commit() {
            Some(e) => Err(e),
            None => reply,
        })
    }
}

/// EXACT_SCRUB_TRACE in the host's environment (`x_scrub_trace=1` on
/// Android): a line on stderr (logcat on Android) per scrub frame that
/// moved the design, as the Apple host prints one (PlanURL.swift): its
/// revision, the first token's value, and when (unix ms) the presenter took
/// it, its commit was made, and its frame was drawn (`turned`). Where frames
/// are said (the Android reader: [`scrub_frames_said`] at each display
/// frame, [`scrub_frame_drawn`] as its host's own turn hands it a stream or
/// none) a line waits for the next; one no frame takes within two (the
/// reader stopped, a frame skipped) says `turned none`, as one does when no
/// frame has come for two. Elsewhere a line is printed at once, its
/// commit's time for `turned`.
fn scrub_trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("EXACT_SCRUB_TRACE").is_some())
}

/// A traced frame: revision, taken, applied (unix ms), value, and when it
/// was applied by this process's clock (its age, which a wall clock set back
/// cannot stretch).
type Traced = (u64, f64, f64, String, std::time::Instant);

/// Two frames: a line waits no longer for its frame.
const FRAME_WAIT: std::time::Duration = std::time::Duration::from_millis(34);

/// Lines waiting for their frame, and when a frame was last said.
static PENDING: std::sync::Mutex<(Vec<Traced>, Option<std::time::Instant>)> =
    std::sync::Mutex::new((Vec::new(), None));
/// Whether this host says its frames.
static FRAMES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn unix_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// The lines, written outside the queue's lock; a failing stderr loses
/// them. `turned`: the frame's time, the commit's (`None`, a host that says
/// no frame), or unknown (`Some(None)`).
fn write_scrub(lines: &[Traced], turned: Option<Option<f64>>) {
    use std::io::Write;
    if lines.is_empty() {
        return;
    }
    let mut err = std::io::stderr().lock();
    for (rev, taken, applied, value, _) in lines {
        let turned = match turned {
            None => format!("{applied:.1}"),
            Some(Some(t)) => format!("{t:.1}"),
            Some(None) => "none".into(),
        };
        let _ = writeln!(
            err,
            "exact scrub: rev {rev} received {taken:.1} applied {applied:.1} turned {turned} value {value}"
        );
    }
}

fn pending() -> std::sync::MutexGuard<'static, (Vec<Traced>, Option<std::time::Instant>)> {
    PENDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn trace_scrub(frame: &str, taken: f64, applied: f64) {
    let Some(edit) = exact_runner::design::Edit::parse(frame) else {
        return;
    };
    let Some(mark) = edit.scrub else {
        return;
    };
    let now = std::time::Instant::now();
    let value = edit
        .tokens
        .first()
        .map_or("", |t| t.value.as_str())
        .to_string();
    let line = (mark.rev, taken, applied, value, now);
    if !FRAMES.load(std::sync::atomic::Ordering::Relaxed) {
        write_scrub(&[line], None);
        return;
    }
    // No frame said for two (the reader stopped): its frame is unknown, as
    // are those waiting longer than two; a few dozen wait at most.
    let (unsaid, stalled) = {
        let mut guard = pending();
        let (queue, said) = &mut *guard;
        let stalled = said.is_none_or(|at| now.duration_since(at) > FRAME_WAIT);
        let stale = queue
            .iter()
            .take_while(|l| stalled || now.duration_since(l.4) > FRAME_WAIT || queue.len() >= 64)
            .count();
        let unsaid: Vec<Traced> = queue.drain(..stale).collect();
        if !stalled {
            queue.push(line.clone());
        }
        (unsaid, stalled)
    };
    write_scrub(&unsaid, Some(None));
    if stalled {
        write_scrub(&[line], Some(None));
    }
}

/// The reader's display frame: this host says its frames.
pub fn scrub_frames_said() {
    if scrub_trace() {
        FRAMES.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The host's own turn handed the reader its stream (or none, a view of the
/// app's own drawing the change): the lines waiting, with its time, those
/// that waited longer than two frames unknown.
pub fn scrub_frame_drawn() {
    if !scrub_trace() {
        return;
    }
    FRAMES.store(true, std::sync::atomic::Ordering::Relaxed);
    let (now, turned) = (std::time::Instant::now(), unix_ms());
    let lines = {
        let mut guard = pending();
        guard.1 = Some(now);
        std::mem::take(&mut guard.0)
    };
    let (late, fresh): (Vec<Traced>, Vec<Traced>) = lines
        .into_iter()
        .partition(|l| now.duration_since(l.4) > FRAME_WAIT);
    write_scrub(&late, Some(None));
    write_scrub(&fresh, Some(Some(turned)));
}
