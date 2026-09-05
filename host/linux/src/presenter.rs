//! The presenter: what a painter holds beyond the kernel — scroll offsets,
//! images, focus, the pointer — and the operations that touch it: frames,
//! hit-testing, presses, wheels, typing, the clock, screenshots.
//!
//! @ref LLP 1015 §4; LLP 1010 §3 (scroll chaining: the web's
//! `overscroll-behavior: auto`); LLP 1012 (the five host-side operations)
//!
//! Scroll offsets are host state, never plan state (LLP 1010). The window
//! is a viewport over a document: the page scrolls when the roots' extent
//! exceeds it. A press is a hit at a point — the deepest painted box under
//! it, then up to the nearest node with a `press` handler, the path a click
//! takes in a browser. A wheel goes to the innermost scroll container under
//! the point that can take its dominant axis, else to the page.

use crate::gpu::Gpu;
use crate::host::{Host, HostError};
use crate::image::AssetResolver;
use crate::image::{Assets, Images};
use crate::paint::{
    content_size, effective_overflow, Backend, Frame, PaintedBox, Painter, Rect4, Scene,
};
use crate::raster::Raster;
use crate::text::{Measurer, Shared, TextEngine};
use exact_kernel::{NodeType, Overflow, PropId, ViewId};
use exact_plan::{EventKind, Plan};
use exact_runner::agent::{num, quote};
use exact_runner::{DataSource, Event};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;
use tiny_skia::Pixmap;

/// The presenter: one host, its painter, and the host state.
pub struct Presenter<D: DataSource> {
    host: Host<D>,
    text: Shared,
    brush: Painter,
    viewport: (f32, f32),
    scroll: BTreeMap<ViewId, (f32, f32)>,
    page: (f32, f32),
    images: Images,
    assets: Assets,
    /// The binary's `compat.json` (LLP 1030 D3a), once handed over: a
    /// reload boots a fresh runner, which is told again.
    compat: String,
    focus: Option<ViewId>,
    pointer: Option<(f32, f32)>,
    boxes: Vec<PaintedBox>,
    dirty: bool,
    /// A failed painter's blank fallback cannot bless an update generation.
    last_frame_succeeded: bool,
    /// Which painter was asked for (`Auto` may change its mind after a
    /// failed frame).
    choice: PainterChoice,
    /// How long the font scan took at boot, milliseconds (the one cost that
    /// is the machine's, not the app's).
    pub fonts_ms: f64,
    /// The painter, for the report.
    pub painter: PainterInfo,
    /// The executor for a request that leaves the process (LLP 1016 D2).
    executor: crate::executor::Executor,
    /// The update store, once the app opened one (LLP 1026 D9; `app.rs`).
    updates: Option<Box<dyn crate::delivery::Store>>,
    /// The commands the last commits' actions asked for, for the loop that
    /// runs them (`run_commands`).
    commands: Vec<exact_runner::Command>,
    /// Boot resolves every initially referenced asset before first pixel.
    /// During that transaction its integrity refusal is returned as a boot
    /// error; later refusals are journaled without retitling a live session.
    booting: bool,
}

/// Two decimals, the agent API's precision.
fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

/// Which backend paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PainterChoice {
    /// The GPU when there is an adapter, else the CPU with a note on stderr.
    Auto,
    /// vello over wgpu; a boot error when there is no adapter.
    Gpu,
    /// tiny-skia.
    Cpu,
}

impl PainterChoice {
    /// `EXACT_PAINTER`: `gpu`, `cpu`, or unset (auto).
    pub fn from_env() -> PainterChoice {
        match std::env::var("EXACT_PAINTER").as_deref() {
            Ok("gpu") => PainterChoice::Gpu,
            Ok("cpu") => PainterChoice::Cpu,
            _ => PainterChoice::Auto,
        }
    }
}

/// What the painter is, for the smoke's report.
#[derive(Debug, Clone)]
pub struct PainterInfo {
    /// `"gpu"` or `"cpu"`.
    pub name: &'static str,
    /// The adapter and API, on the GPU.
    pub adapter: Option<String>,
    /// Device creation, milliseconds, on the GPU.
    pub device_ms: f64,
    /// Shader compilation, milliseconds, on the GPU.
    pub shaders_ms: f64,
    /// Whether the shaders came from the pipeline cache on disk.
    pub cached: bool,
}

fn cpu_info() -> PainterInfo {
    PainterInfo {
        name: "cpu",
        adapter: None,
        device_ms: 0.0,
        shaders_ms: 0.0,
        cached: false,
    }
}

fn open_backend(choice: PainterChoice) -> Result<(Box<dyn Backend>, PainterInfo), String> {
    let cpu = || (Box::new(Raster::new()) as Box<dyn Backend>, cpu_info());
    match choice {
        PainterChoice::Cpu => Ok(cpu()),
        PainterChoice::Gpu | PainterChoice::Auto => match Gpu::new() {
            Ok(g) => {
                let info = PainterInfo {
                    name: "gpu",
                    adapter: Some(format!("{} ({})", g.adapter, g.api)),
                    device_ms: g.device_ms,
                    shaders_ms: g.shaders_ms,
                    cached: g.cached,
                };
                Ok((Box::new(g), info))
            }
            Err(e) if choice == PainterChoice::Auto => {
                // A note, not an error (the smoke reads stderr for errors).
                eprintln!("painting on the CPU: no GPU ({e})");
                Ok(cpu())
            }
            Err(e) => Err(format!("no GPU: {e}")),
        },
    }
}

impl<D: DataSource> Presenter<D> {
    /// Boot the app under a viewport (points) at a device scale, with its
    /// asset root. The boot error, if any, is reported beside the presenter
    /// (the tree is what booted).
    pub fn boot(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        Presenter::boot_with(
            plan,
            data,
            viewport,
            scale,
            assets,
            PainterChoice::from_env(),
        )
    }

    /// Boot with a chosen painter (`boot` reads `EXACT_PAINTER`).
    pub fn boot_with(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
        choice: PainterChoice,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        Self::boot_with_assets(
            plan,
            data,
            viewport,
            scale,
            Assets::embedded(assets),
            choice,
        )
    }

    /// Boot from entry zero or one selected generation. The selected asset
    /// roster is complete: absent names cannot fall through to `root`.
    pub(crate) fn boot_selected(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        root: PathBuf,
        selected: Option<AssetResolver>,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let assets = match selected {
            Some(set) => Assets::selected(root, set),
            None => Assets::embedded(root),
        };
        Self::boot_with_assets(
            plan,
            data,
            viewport,
            scale,
            assets,
            PainterChoice::from_env(),
        )
    }

    fn boot_with_assets(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: Assets,
        choice: PainterChoice,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let t = std::time::Instant::now();
        let decoded = Plan::decode(plan).map_err(HostError::Plan)?;
        let text = TextEngine::shared_for_assets(&decoded, &assets);
        if let Some(reason) = assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        let fonts_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (backend, painter) = open_backend(choice).map_err(HostError::Painter)?;
        let (mut host, error) = Host::boot(
            plan,
            data,
            Box::new(Measurer(text.clone())),
            viewport.0,
            viewport.1,
        )?;
        let mut images = Images::with_assets(assets.clone());
        if assets.is_selected() {
            if let Some(error) = error {
                return Err(HostError::Layout(error));
            }
            let mut reports = images.sync(host.kernel(), &host.preorder());
            reports.extend(images.wait(Duration::from_secs(1)));
            let mut layout_error = None;
            for (view, size) in reports {
                layout_error = layout_error.or(host.set_intrinsic(view, size));
            }
            if let Some(reason) = assets.take_refusal() {
                return Err(HostError::Asset(reason));
            }
            if images.pending() {
                return Err(HostError::Layout(
                    "selected images did not finish preparing".into(),
                ));
            }
            if let Some(error) = layout_error {
                return Err(HostError::Layout(error));
            }
        }
        // Initial selected layout, assets and intrinsic sizes accepted.
        // Only now may the app's queued requests reach its executor.
        let executor = crate::executor::Executor::start(&host.grants());
        if let Some(note) = executor.note() {
            host.log(note.to_string());
        }
        let mut p = Presenter {
            host,
            executor,
            brush: Painter::new(text.clone(), scale, backend),
            text,
            viewport,
            scroll: BTreeMap::new(),
            page: (0.0, 0.0),
            images,
            assets,
            compat: String::new(),
            focus: None,
            pointer: None,
            boxes: Vec::new(),
            dirty: true,
            last_frame_succeeded: false,
            choice,
            fonts_ms,
            painter,
            updates: None,
            commands: Vec::new(),
            booting: true,
        };
        let e = p.after_commit();
        p.booting = false;
        if let Some(reason) = p.assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        Ok((p, error.or(e)))
    }

    /// Attach the update store (LLP 1026 D9): what it has to say reaches
    /// the runner now (`state.delivery`, the `delivery` resource) and after
    /// every check; a boot note it left goes to the journal.
    pub fn set_updates(&mut self, updates: Option<Box<dyn crate::delivery::Store>>) {
        self.updates = updates;
        if let Some(note) = self.updates.as_mut().and_then(|u| u.take_note()) {
            self.host.log(note);
        }
        self.sync_delivery();
    }

    /// The store's wake, for the display loop's poll set.
    pub fn update_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.updates.as_ref().map(|u| u.fd())
    }

    /// First pixel (LLP 1026 D11): the selection that booted is good.
    pub fn first_pixel(&mut self) {
        if self.dirty || !self.last_frame_succeeded {
            return;
        }
        if let Some(u) = self.updates.as_mut() {
            u.boot_succeeded();
        }
        self.sync_delivery();
    }

    /// Check the stream's head now, on the store's thread; the outcome
    /// arrives through `poll_update`. `false` with no store, or a check
    /// already running.
    pub fn check_update(&mut self) -> bool {
        match &self.updates {
            Some(u) => u.check(),
            None => false,
        }
    }

    /// A finished check, if one landed: its line to stderr and the journal
    /// (`exact update: …`), the store's facts into the runner.
    pub fn poll_update(&mut self) -> bool {
        let Some(line) = self.updates.as_mut().and_then(|u| u.take_line()) else {
            return false;
        };
        eprintln!("exact update: {line}");
        self.host.log(format!("exact update: {line}"));
        self.sync_delivery();
        true
    }

    /// Apply the staged bundle now with carry (`deliveryActivate`, LLP 1030
    /// D7): its assets stand in for the root's by name, its plan restarts
    /// the app. `Ok(false)` when nothing is staged.
    pub fn activate_update(&mut self, data: D) -> Result<bool, HostError> {
        let Some(updates) = self.updates.as_ref() else {
            return Ok(false);
        };
        let Some(candidate) = updates.prepare_activation().map_err(HostError::Asset)? else {
            return Ok(false);
        };
        let assets = Assets::selected(self.assets.root().to_path_buf(), candidate.assets.clone());
        let decoded = Plan::decode(&candidate.plan).map_err(HostError::Plan)?;
        let text = TextEngine::shared_for_assets(&decoded, &assets);
        let carried = self.host.carry();
        let (mut host, error) = Host::boot_with(
            &candidate.plan,
            data,
            Box::new(Measurer(text.clone())),
            self.viewport.0,
            self.viewport.1,
            Some(&carried),
        )?;
        if let Some(error) = error {
            return Err(HostError::Layout(error));
        }
        let mut delivery = host.runner().delivery().with_compat(&self.compat);
        updates.status_into(&mut delivery);
        updates.staged_stream_into(&mut delivery);
        delivery.seq = candidate.seq;
        delivery.staged = false;
        if let Some(error) = host.set_delivery(delivery) {
            return Err(HostError::Layout(error));
        }
        let mut images = Images::with_assets(assets.clone());
        let mut reports = images.sync(host.kernel(), &host.preorder());
        reports.extend(images.wait(Duration::from_secs(1)));
        if images.pending() {
            return Err(HostError::Layout(
                "selected images did not finish preparing".into(),
            ));
        }
        for (view, size) in reports {
            if let Some(error) = host.set_intrinsic(view, size) {
                return Err(HostError::Layout(error));
            }
        }
        if let Some(reason) = assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        self.updates
            .as_mut()
            .unwrap()
            .commit_activation(candidate.entry, candidate.seq)
            .map_err(HostError::Asset)?;
        self.updates.as_mut().unwrap().boot_started();
        self.host = host;
        self.text = text.clone();
        self.brush.text = text;
        self.assets = assets;
        self.images = images;
        self.executor = crate::executor::Executor::start(&self.host.grants());
        self.scroll.clear();
        self.page = (0.0, 0.0);
        self.focus = None;
        self.pointer = None;
        self.commands.clear();
        self.host.log("exact update: activated the staged bundle");
        self.sync_delivery();
        self.after_commit();
        Ok(true)
    }

    /// The store's facts into the runner (LLP 1030 D7): a `delivery`
    /// resource is answered again, and the picture follows.
    fn sync_delivery(&mut self) {
        let Some(u) = &self.updates else {
            return;
        };
        let mut delivery = self.host.runner().delivery().clone();
        u.status_into(&mut delivery);
        if let Some(e) = self.host.set_delivery(delivery) {
            eprintln!("exact: {e}");
        }
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
    }

    /// Run the commands the last commits asked for (LLP 1005 §3): the
    /// delivery pair are the store's (LLP 1030 D7); `setScheme` has no
    /// appearance to set on this painter; anything else is named.
    pub fn run_commands(&mut self, mut data: impl FnMut() -> D) {
        for c in std::mem::take(&mut self.commands) {
            match c.name.as_str() {
                "deliveryCheck" => {
                    if !self.check_update() {
                        eprintln!("exact update: no store, or a check is already running");
                    }
                }
                "deliveryActivate" => match self.activate_update(data()) {
                    Ok(true) => {}
                    Ok(false) => eprintln!("exact update: nothing is staged"),
                    Err(e) => eprintln!("exact update: activate: {e}"),
                },
                "setScheme" => {}
                other => eprintln!("exact: unknown command {other}"),
            }
        }
    }

    /// The dev loop's restart: boot the new plan with state carried; every
    /// picture, offset, and focus goes (LLP 1007 §6).
    pub fn reload(&mut self, plan: &[u8], data: D) -> Result<Option<String>, HostError> {
        let decoded = Plan::decode(plan).map_err(HostError::Plan)?;
        // Fonts are candidate state too. Keep the running plan's catalog and
        // caches untouched until its runner has booted successfully.
        let candidate_text = TextEngine::shared_for_assets(&decoded, &self.assets);
        if let Some(reason) = self.assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        let carried = self.host.carry();
        let (host, error) = Host::boot_with(
            plan,
            data,
            Box::new(Measurer(candidate_text.clone())),
            self.viewport.0,
            self.viewport.1,
            Some(&carried),
        )?;
        self.host = host;
        // A fresh runner knows nothing of the binary's delivery facts (LLP
        // 1030 D7): the compat file and the store's status again, before
        // anything reads a frame — a reload is not a launch, and the facts
        // must not read as the embedded answer after one.
        if !self.compat.is_empty() {
            if let Some(e) = self.host.set_delivery_from_compat(&self.compat) {
                eprintln!("exact: {e}");
            }
        }
        if let Some(u) = &self.updates {
            let mut delivery = self.host.runner().delivery().clone();
            u.status_into(&mut delivery);
            if let Some(e) = self.host.set_delivery(delivery) {
                eprintln!("exact: {e}");
            }
        }
        self.text = candidate_text.clone();
        self.brush.text = candidate_text;
        self.executor = crate::executor::Executor::start(&self.host.grants());
        self.scroll.clear();
        self.page = (0.0, 0.0);
        self.images.reset();
        self.focus = None;
        let e = self.after_commit();
        Ok(error.or(e))
    }

    /// The host.
    pub fn host(&self) -> &Host<D> {
        &self.host
    }

    /// The text engine.
    pub fn text(&self) -> &Shared {
        &self.text
    }

    /// The images.
    pub fn images(&self) -> &Images {
        &self.images
    }

    /// The viewport, points.
    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// The page's scroll offset.
    pub fn page(&self) -> (f32, f32) {
        self.page
    }

    /// The focused input.
    pub fn focus(&self) -> Option<ViewId> {
        self.focus
    }

    /// Whether the picture is stale.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// The last frame's (encode + render, readback) milliseconds, on the GPU.
    pub fn last_frame_ms(&self) -> Option<(f64, f64)> {
        self.brush.last_frame_ms()
    }

    /// Where the pointer is drawn (`None` draws none).
    pub fn set_pointer(&mut self, pointer: Option<(f32, f32)>) {
        if self.pointer != pointer {
            self.pointer = pointer;
            self.dirty = true;
        }
    }

    /// The viewport changed.
    pub fn resize(&mut self, width: f32, height: f32) -> Option<String> {
        self.viewport = (width, height);
        let e = self.host.resize(width, height);
        self.dirty = true;
        self.clamp_scroll();
        e
    }

    /// After anything that may have committed: images follow the tree,
    /// offsets stay in range, focus stays on a live input, the picture is
    /// stale.
    fn after_commit(&mut self) -> Option<String> {
        self.dirty = true;
        // What the commit asked the host to run goes to the executor (LLP
        // 1016 D2); the reply comes back through `pump`. Its commands wait
        // for the loop (`run_commands`).
        for r in self.host.take_requests() {
            self.executor.run(r);
        }
        self.commands.extend(self.host.take_commands());
        let live = self.host.preorder();
        let reports = self.images.sync(self.host.kernel(), &live);
        let mut error = None;
        for (view, size) in reports {
            error = error.or(self.host.set_intrinsic(view, size));
        }
        if !self.booting {
            error = error.or_else(|| {
                self.assets
                    .take_refusal()
                    .map(|reason| format!("selected asset refused: {reason}"))
            });
        }
        if let Some(f) = self.focus {
            if self.host.kernel().node(f).is_none() {
                self.focus = None;
            }
        }
        self.clamp_scroll();
        error
    }

    /// Loads that arrived since the last call: their sizes reach the kernel.
    /// Whether anything changed.
    pub fn poll_images(&mut self) -> bool {
        let reports = self.images.poll();
        self.apply_reports(reports)
    }

    /// Wait for every load in flight (bounded).
    pub fn wait_images(&mut self, timeout: Duration) -> bool {
        let reports = self.images.wait(timeout);
        self.apply_reports(reports)
    }

    fn apply_reports(&mut self, reports: Vec<crate::image::Report>) -> bool {
        let any = !reports.is_empty();
        for (view, size) in reports {
            if let Some(e) = self.host.set_intrinsic(view, size) {
                eprintln!("exact: {e}");
            }
        }
        if any {
            self.dirty = true;
            self.clamp_scroll();
        }
        any
    }

    /// The document's extent: the roots' frames, never smaller than the
    /// viewport (`fitDocument`, LLP 1010 §3).
    fn document(&self) -> (f32, f32) {
        let kernel = self.host.kernel();
        let mut size = self.viewport;
        for root in self.host.roots() {
            if let Some(n) = kernel.node(root) {
                size.0 = size.0.max(n.frame.x + n.frame.width);
                size.1 = size.1.max(n.frame.y + n.frame.height);
            }
        }
        size
    }

    fn clamp_scroll(&mut self) {
        let kernel = self.host.kernel();
        let mut gone = Vec::new();
        for (id, off) in self.scroll.iter_mut() {
            match kernel.node(*id) {
                Some(n) => {
                    let (cw, ch) = content_size(&n, kernel);
                    off.0 = off.0.clamp(0.0, (cw - n.frame.width).max(0.0));
                    off.1 = off.1.clamp(0.0, (ch - n.frame.height).max(0.0));
                }
                None => gone.push(*id),
            }
        }
        for id in gone {
            self.scroll.remove(&id);
        }
        let doc = self.document();
        self.page.0 = self.page.0.clamp(0.0, (doc.0 - self.viewport.0).max(0.0));
        self.page.1 = self.page.1.clamp(0.0, (doc.1 - self.viewport.1).max(0.0));
    }

    /// Paint a frame: the pixels, with every box recorded for `layout` and
    /// hit-testing.
    pub fn frame(&mut self) -> Pixmap {
        let roots = self.host.roots();
        let host = &self.host;
        let presented = |id: ViewId| host.presented(id);
        let scene = Scene {
            kernel: host.kernel(),
            roots: &roots,
            presented: &presented,
            scroll: &self.scroll,
            page: self.page,
            images: &self.images.bitmaps,
            focus: self.focus,
            pointer: self.pointer,
        };
        let mut painted = self.brush.paint(&scene, self.viewport);
        if let Err(e) = &painted {
            if self.choice == PainterChoice::Auto && self.brush.backend() == "gpu" {
                // The GPU failed a frame (a lost device, a readback with no
                // answer): the CPU paints from here on, this frame first.
                eprintln!("exact: paint: {e}; painting on the CPU from here");
                self.brush.replace_backend(Box::new(Raster::new()));
                self.painter = cpu_info();
                painted = self.brush.paint(&scene, self.viewport);
            }
        }
        self.last_frame_succeeded = painted.is_ok();
        let (pixmap, boxes) = match painted {
            Ok(Frame { pixmap, boxes }) => (pixmap, boxes),
            Err(e) => {
                // A frame nobody could paint: a blank picture, and the last
                // frame's boxes kept, so input still lands where things were.
                eprintln!("exact: paint: {e}");
                let w = ((self.viewport.0 * self.brush.scale).round() as u32).max(1);
                let h = ((self.viewport.1 * self.brush.scale).round() as u32).max(1);
                let mut blank = Pixmap::new(w, h).expect("a viewport has pixels");
                blank.fill(tiny_skia::Color::WHITE);
                (blank, std::mem::take(&mut self.boxes))
            }
        };
        self.boxes = boxes;
        self.dirty = false;
        pixmap
    }

    /// Every node's painted box, in paint order (a fresh frame when stale).
    pub fn boxes(&mut self) -> &[PaintedBox] {
        if self.dirty {
            let _ = self.frame();
        }
        &self.boxes
    }

    fn box_of(&mut self, id: ViewId) -> Option<PaintedBox> {
        self.boxes().iter().find(|b| b.id == id).copied()
    }

    /// The agent's `layout`: every node's box in the viewport (scroll
    /// folded in), scroll containers with their offsets, by id.
    pub fn layout_json(&mut self) -> String {
        let clock = self.host.now();
        let (vw, vh) = self.viewport;
        let mut boxes: Vec<PaintedBox> = self.boxes().to_vec();
        boxes.sort_by_key(|b| b.id);
        let mut s = String::new();
        // The page's environment (LLP 1012 §1): no safe area and no
        // software keyboard on this host — every value is zero.
        let _ = write!(
            s,
            "{{\"clock\":{},\"viewport\":{{\"w\":{},\"h\":{}}},\"env\":{{\"safe-area-inset-top\":0,\"safe-area-inset-right\":0,\"safe-area-inset-bottom\":0,\"safe-area-inset-left\":0,\"keyboard-inset-height\":0}},\"nodes\":[",
            num(clock),
            num(r2(vw)),
            num(r2(vh))
        );
        for (i, b) in boxes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "{{\"id\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
                b.id,
                num(r2(b.rect.0)),
                num(r2(b.rect.1)),
                num(r2(b.rect.2)),
                num(r2(b.rect.3))
            );
            if let Some((sx, sy)) = b.scroll {
                let _ = write!(s, ",\"sx\":{},\"sy\":{}", num(r2(sx)), num(r2(sy)));
            }
            s.push('}');
        }
        s.push_str("]}");
        s
    }

    /// The deepest painted box under a point (viewport points), through
    /// every clip.
    pub fn hit(&mut self, x: f32, y: f32) -> Option<ViewId> {
        self.boxes()
            .iter()
            .rev()
            .find(|b| b.contains(x, y))
            .map(|b| b.id)
    }

    /// The nearest node at or above `id` with a handler for `kind`.
    fn handler_target(&self, id: ViewId, kind: EventKind) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let mut at = Some(id);
        while let Some(n) = at {
            let node = kernel.node(n)?;
            if node.props.bool(PropId::Disabled) == Some(true) {
                return None;
            }
            if self.host.runner().handlers_of(n).contains(&kind) {
                return Some(n);
            }
            at = node.parent;
        }
        None
    }

    /// A press at a point, the path a click takes: hit, then up to a
    /// `press` handler; focus follows the click (an input takes it, anything
    /// else drops it). Returns the node pressed, if any.
    pub fn press_at(&mut self, x: f32, y: f32, now_ms: f64) -> Option<ViewId> {
        let hit = self.hit(x, y)?;
        let kernel = self.host.kernel();
        let focus = kernel
            .node(hit)
            .filter(|node| node.node_type == NodeType::TextInput)
            .filter(|node| node.props.bool(PropId::Disabled) != Some(true))
            .map(|_| hit);
        if self.focus != focus {
            self.focus = focus;
            self.dirty = true;
        }
        let target = self.handler_target(hit, EventKind::Press)?;
        if let Some(e) = self.host.dispatch_at(target, Event::Press, now_ms) {
            eprintln!("exact: {e}");
        }
        let e = self.after_commit();
        if let Some(e) = e {
            eprintln!("exact: {e}");
        }
        Some(target)
    }

    /// The agent's `tap`: a press at the node's center through the same
    /// path a pointer takes.
    pub fn tap(&mut self, id: ViewId) -> Result<String, String> {
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (x, y) = (b.rect.0 + b.rect.2 / 2.0, b.rect.1 + b.rect.3 / 2.0);
        let now = self.host.now();
        self.press_at(x, y, now);
        Ok(format!(
            "{{\"tapped\":{id},\"at\":[{},{}]}}",
            num(r2(x)),
            num(r2(y))
        ))
    }

    /// A wheel at a point (the web's sign: a positive `dy` scrolls down),
    /// LLP 1010 §3: the innermost scroll container under the point that can
    /// take the dominant axis takes what it can of both; otherwise the page.
    pub fn wheel_at(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let mut at = self.hit(x, y);
        let kernel = self.host.kernel();
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else { break };
            let (ox, oy) = effective_overflow(&node);
            if ox == Overflow::Scroll || oy == Overflow::Scroll {
                let (cw, ch) = content_size(&node, kernel);
                let max = (
                    (cw - node.frame.width).max(0.0),
                    (ch - node.frame.height).max(0.0),
                );
                let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
                let take_x = ox == Overflow::Scroll
                    && dx != 0.0
                    && max.0 > 0.0
                    && ((dx > 0.0 && off.0 < max.0) || (dx < 0.0 && off.0 > 0.0));
                let take_y = oy == Overflow::Scroll
                    && dy != 0.0
                    && max.1 > 0.0
                    && ((dy > 0.0 && off.1 < max.1) || (dy < 0.0 && off.1 > 0.0));
                let dominant = if dy.abs() >= dx.abs() { take_y } else { take_x };
                if dominant {
                    let nx = if take_x {
                        (off.0 + dx).clamp(0.0, max.0)
                    } else {
                        off.0
                    };
                    let ny = if take_y {
                        (off.1 + dy).clamp(0.0, max.1)
                    } else {
                        off.1
                    };
                    self.scroll.insert(id, (nx, ny));
                    self.dirty = true;
                    return;
                }
            }
            at = node.parent;
        }
        let doc = self.document();
        let max = (
            (doc.0 - self.viewport.0).max(0.0),
            (doc.1 - self.viewport.1).max(0.0),
        );
        let next = (
            (self.page.0 + dx).clamp(0.0, max.0),
            (self.page.1 + dy).clamp(0.0, max.1),
        );
        if next != self.page {
            self.page = next;
            self.dirty = true;
        }
    }

    /// The agent's wheel: over the node's center.
    pub fn wheel(&mut self, id: ViewId, dx: f32, dy: f32) -> Result<String, String> {
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (x, y) = (b.rect.0 + b.rect.2 / 2.0, b.rect.1 + b.rect.3 / 2.0);
        self.wheel_at(x, y, dx, dy);
        Ok(format!(
            "{{\"tapped\":{id},\"wheel\":[{},{}],\"at\":[{},{}]}}",
            num(dx as f64),
            num(dy as f64),
            num(r2(x)),
            num(r2(y))
        ))
    }

    /// Set an input's value as typing does: focused, the value replaced,
    /// one `change` heard by the runner.
    pub fn type_text(&mut self, id: ViewId, text: &str) -> Result<String, String> {
        let kernel = self.host.kernel();
        let node = kernel.node(id).ok_or_else(|| format!("no view {id}"))?;
        if node.node_type != NodeType::TextInput {
            return Err(format!("view {id} is not an input"));
        }
        if node.props.bool(PropId::Disabled) == Some(true) {
            return Err(format!("view {id} is disabled"));
        }
        self.focus = Some(id);
        let now = self.host.now();
        let error = self
            .host
            .dispatch_at(id, Event::Change(text.to_string()), now);
        let e = self.after_commit();
        if let Some(e) = error.or(e) {
            return Err(e);
        }
        let value = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_string))
            .unwrap_or_default();
        let mut s = format!("{{\"typed\":{id},\"value\":");
        quote(&value, &mut s);
        s.push('}');
        Ok(s)
    }

    /// A key for the focused input: a character appended, a backspace, or
    /// nothing. The runner hears one `change` with the new value.
    pub fn key(&mut self, ch: Option<char>, backspace: bool, now_ms: f64) {
        let Some(id) = self.focus else { return };
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        if node.props.bool(PropId::Disabled) == Some(true) {
            return;
        }
        let mut value = node.props.str(PropId::Value).unwrap_or("").to_string();
        match (ch, backspace) {
            (Some(c), _) => value.push(c),
            (None, true) => {
                value.pop();
            }
            _ => return,
        }
        if let Some(e) = self.host.dispatch_at(id, Event::Change(value), now_ms) {
            eprintln!("exact: {e}");
        }
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
    }

    /// Drop focus.
    pub fn blur(&mut self) {
        if self.focus.take().is_some() {
            self.dirty = true;
        }
    }

    /// The executor's replies into the runner (LLP 1016 D2), each a
    /// commit: the display loop calls this when the executor's fd is
    /// readable, the agent when it waits. `None` when nothing was queued.
    pub fn pump(&mut self, now_ms: f64) -> Option<String> {
        let outcomes = self.executor.drain();
        if outcomes.is_empty() {
            return None;
        }
        let e = self.host.fulfill_all(outcomes, now_ms);
        let after = self.after_commit();
        e.or(after)
    }

    /// The executor's wake: readable when a reply is queued (for `poll`).
    pub fn executor_fd(&self) -> std::os::unix::io::RawFd {
        self.executor.fd()
    }

    /// Whether a request is in flight.
    pub fn pending(&self) -> bool {
        self.host.runner().has_pending()
    }

    /// Move the clock: timers fire, motion is seeked to where the clock
    /// landed (LLP 1012 `clock`). Returns the landing time and the error.
    pub fn clock(&mut self, to_ms: f64) -> (f64, Option<String>) {
        let e = self.host.advance(to_ms);
        let landed = self.host.now();
        self.host.tick(landed);
        let after = self.after_commit();
        (landed, e.or(after))
    }

    /// The binary's delivery facts (LLP 1030 D7), from its `compat.json`:
    /// a `delivery` resource is answered again, and the picture follows.
    pub fn set_delivery_from_compat(&mut self, json: &str) -> Option<String> {
        self.compat = json.to_string();
        let e = self.host.set_delivery_from_compat(json);
        let after = self.after_commit();
        e.or(after)
    }

    /// The runner's clock (timers), from the presenter's loop.
    pub fn advance(&mut self, now_ms: f64) -> Option<String> {
        let e = self.host.advance(now_ms);
        let after = self.after_commit();
        e.or(after)
    }

    /// A motion frame.
    pub fn tick(&mut self, now_ms: f64) {
        self.host.tick(now_ms);
        self.dirty = true;
    }

    /// The pixels, as a PNG at `path`.
    pub fn screenshot(&mut self, path: &str) -> Result<String, String> {
        let frame = self.frame();
        let png = frame.encode_png().map_err(|e| format!("png: {e}"))?;
        std::fs::write(path, png).map_err(|e| format!("write {path}: {e}"))?;
        let mut s = String::from("{\"screenshot\":");
        quote(path, &mut s);
        let _ = write!(
            s,
            ",\"w\":{},\"h\":{}}}",
            num(r2(self.viewport.0)),
            num(r2(self.viewport.1))
        );
        Ok(s)
    }

    /// The box of a node, if painted.
    pub fn rect_of(&mut self, id: ViewId) -> Option<Rect4> {
        self.box_of(id).map(|b| b.rect)
    }

    /// A scroll container's offset.
    pub fn scroll_of(&self, id: ViewId) -> (f32, f32) {
        self.scroll.get(&id).copied().unwrap_or((0.0, 0.0))
    }

    /// How many nodes are live.
    pub fn node_count(&self) -> usize {
        self.host.kernel().live_count()
    }
}
