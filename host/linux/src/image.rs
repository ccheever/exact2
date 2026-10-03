//! Bounded native raster demand. Metadata preparation and decoding are separate;
//! a replacement publishes its pixels and natural geometry together.
use exact_kernel::{Kernel, NodeType, PropId, ViewId};
use exact_raster::{
    Demand, Gate, PixelSize, Priority, RasterKey, RasterLease, Refusal, RequestId, RequestStatus,
    Stats, ViewKey, SESSION_BYTES, SUBSCRIPTIONS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "image/allocation_tests.rs"]
mod allocation_tests;
#[path = "image/assets.rs"]
mod assets;
#[path = "image/bitmap.rs"]
mod bitmap;
#[cfg(test)]
#[path = "image/control_tests.rs"]
mod control_tests;
#[cfg(test)]
#[path = "image/decode_tests.rs"]
mod decode_tests;
#[path = "image/jpeg.rs"]
mod jpeg_decode;
#[path = "image/png.rs"]
mod png_decode;
#[path = "image/workers.rs"]
mod workers;
pub use assets::{AssetResolver, Assets};
pub use bitmap::Bitmap;
use workers::{Backend, Prepared, SourceOwner};

struct View {
    key: ViewKey,
    source: String,
    displayed_source: String,
    source_id: Option<Arc<SourceOwner>>,
    request: Option<(RequestId, PixelSize)>,
    accepted: Option<RequestId>,
    requested_for: (f32, f32),
    lease: Option<RasterLease>,
    desired: (f32, f32),
    visible: bool,
    refusal: Option<Refusal>,
    symbol_size: Option<f32>,
}

/// One generation's bounded live demand, sharing its budget with replacements.
pub struct Images {
    assets: Arc<Assets>,
    backend: Arc<Backend>,
    generation: u64,
    views: BTreeMap<ViewId, View>,
    /// Shared immutable pictures. Removing a view never refunds another owner.
    pub bitmaps: BTreeMap<ViewId, Arc<Bitmap>>,
    /// Current loaded views and NATURAL pixel sizes; never traversal history.
    pub loaded: Vec<(String, (u32, u32))>,
    decode_enabled: bool,
    deferred: usize,
    /// The image nodes in preorder, as of a kernel epoch: the walk
    /// [`Images::sync_visible`] needs, redone only after a commit.
    pub(crate) order: Option<(u64, Vec<ViewId>)>,
}

/// Accepted pixels and natural size, or explicit empty-source removal.
pub type Report = (ViewId, Option<(f32, f32)>);

impl Images {
    /// An embedded generation beneath the asset root.
    pub fn new(assets: PathBuf) -> Self {
        Self::with_assets(Assets::embedded(assets))
    }
    pub(crate) fn with_assets(assets: Assets) -> Self {
        Self::make(assets, Backend::new())
    }
    fn make(assets: Assets, backend: Arc<Backend>) -> Self {
        Self {
            generation: backend.generation(),
            assets: Arc::new(assets),
            backend,
            views: BTreeMap::new(),
            bitmaps: BTreeMap::new(),
            loaded: Vec::new(),
            decode_enabled: true,
            deferred: 0,
            order: None,
        }
    }
    pub(crate) fn candidate(&self, assets: Assets) -> Self {
        Self::make(assets, self.backend.clone())
    }
    /// Size the decoded-image budget to the screen, as the Apple host does
    /// (`RasterLoader.viewportBudget`): eight viewport-sized RGBA bitmaps,
    /// 32–192 MiB, so the pictures a scroll leaves stay cached for its return
    /// instead of decoding again (on a 3x phone the 32 MiB floor held about
    /// one screen of the heavy list's photos).
    pub fn fit(&self, viewport: (f32, f32), scale: f32) {
        let pixels = f64::from(viewport.0 * scale) * f64::from(viewport.1 * scale);
        let floor = exact_raster::SESSION_BYTES as f64;
        // EXACT_IMAGE_VIEWPORTS: how many viewports of pictures to keep
        // decoded (8, Apple's RasterLoader rule, unless a host says).
        let viewports = std::env::var("EXACT_IMAGE_VIEWPORTS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
            .unwrap_or(8.0);
        let cap = (192.0f64 * 1024.0 * 1024.0).max(pixels * 4.0 * viewports);
        let budget = if pixels.is_finite() && pixels > 0.0 {
            (pixels * 4.0 * viewports).clamp(floor, cap)
        } else {
            floor
        };
        self.backend.session.set_budget(budget as u64);
    }

    /// The shared session ledger, including allocations retained by old owners.
    pub fn stats(&self) -> Stats {
        self.backend.session.stats()
    }
    /// Where an embedded relative source resolves; selected assets expose bytes.
    pub fn resolve(&self, source: &str) -> Option<PathBuf> {
        self.assets.path(source)
    }
    /// The embedded root, retained for diagnostics.
    pub fn assets(&self) -> &Path {
        self.assets.root()
    }

    /// Reconcile mounted nodes. The presenter additionally supplies visibility.
    pub fn sync(&mut self, kernel: &Kernel, live: &[ViewId]) -> Vec<Report> {
        self.sync_visible(kernel, live, 1., |_| true)
    }
    pub(crate) fn sync_visible(
        &mut self,
        kernel: &Kernel,
        live: &[ViewId],
        scale: f32,
        visible: impl Fn(ViewId) -> bool,
    ) -> Vec<Report> {
        let mut reports = Vec::new();
        let mut seen = BTreeSet::new();
        self.deferred = 0;
        for id in live {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            if node.node_type != NodeType::Image {
                continue;
            }
            if seen.len() >= SUBSCRIPTIONS {
                self.deferred += 1;
                continue;
            }
            seen.insert(*id);
            let packed = u64::from(node.key.index) | (u64::from(node.key.generation) << 32);
            let key = ViewKey {
                view: packed,
                generation: self.generation,
            };
            let source = node.props.str(PropId::ImageSource).unwrap_or("");
            if self.views.get(id).is_some_and(|view| view.key != key) {
                self.remove(*id);
            }
            let view = self.views.entry(*id).or_insert_with(|| View {
                key,
                source: String::new(),
                displayed_source: String::new(),
                source_id: None,
                request: None,
                accepted: None,
                requested_for: (0., 0.),
                lease: None,
                desired: (0., 0.),
                visible: true,
                refusal: None,
                symbol_size: None,
            });
            view.desired = (node.frame.width * scale, node.frame.height * scale);
            view.visible = visible(*id);
            // LLP 1035.004.000: symbols are an empty em square on Linux,
            // never a file request and never the previously accepted raster.
            if source.starts_with("symbol:") {
                if let Some((request, _)) = view.request.take() {
                    self.backend.cancel(request);
                }
                view.source_id = None;
                view.accepted = None;
                view.lease = None;
                view.refusal = None;
                view.displayed_source.clear();
                self.bitmaps.remove(id);
                let size = node
                    .computed_style(exact_kernel::StyleMask::INHERITED)
                    .font_size;
                if view.source != source || view.symbol_size != Some(size) {
                    reports.push((*id, (size > 0.).then_some((size, size))));
                    view.source = source.to_owned();
                    view.symbol_size = Some(size);
                }
                continue;
            }
            if view.symbol_size.take().is_some() {
                reports.push((*id, None));
            }
            if !view.visible {
                if let Some((request, _)) = view.request.take() {
                    self.backend.cancel(request);
                }
                view.lease = None;
                self.bitmaps.remove(id);
            }
            if source.len() > 4096 {
                if let Some((request, _)) = view.request.take() {
                    self.backend.cancel(request);
                }
                view.source.clear();
                view.source_id = None;
                view.refusal = Some(Refusal::DecodeFailed);
                self.deferred += 1;
                continue; // Deliberately keep only the previously accepted picture.
            }
            if view.source != source {
                if let Some((request, _)) = view.request.take() {
                    self.backend.cancel(request);
                }
                view.source = source.to_owned();
                view.source_id = None;
                view.refusal = None;
                // The old lease, bitmap and natural size stay until acceptance.
                if source.is_empty() {
                    view.lease = None;
                    self.bitmaps.remove(id);
                    reports.push((*id, None));
                }
            }
        }
        let gone: Vec<_> = self
            .views
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.remove(id);
        }
        reports.extend(self.poll());
        reports
    }
    fn remove(&mut self, id: ViewId) {
        if let Some(view) = self.views.remove(&id) {
            if let Some((request, _)) = view.request {
                self.backend.cancel(request);
            }
            self.backend.session.retire_view(view.key);
        }
        self.bitmaps.remove(&id);
    }
    /// Consume bounded completions and retry metadata/decode admission once.
    pub fn poll(&mut self) -> Vec<Report> {
        self.backend.drain_wake();
        let mut reports = Vec::new();
        for (id, view) in &mut self.views {
            if view.source.is_empty() || view.symbol_size.is_some() {
                continue;
            }
            if view.source_id.is_none() {
                match self
                    .backend
                    .source(self.generation, &view.source, &self.assets)
                {
                    Ok(source) => {
                        view.source_id = Some(source);
                        view.refusal = None;
                    }
                    Err(e) => {
                        view.refusal = Some(e);
                        continue;
                    }
                }
            }
            let source = view.source_id.as_ref().unwrap().id;
            let header = match self.backend.prepared(source) {
                Some(Prepared::Ready(header)) => header,
                Some(Prepared::Failed(e)) => {
                    view.refusal = Some(e);
                    continue;
                }
                _ => continue,
            };
            if !view.visible || !self.decode_enabled {
                continue;
            }
            // Ask for the intended bucket first: a cache hit requires no new
            // output/scratch reservation. Only a real budget wait downsizes.
            let mut admission_budget = SESSION_BYTES;
            if let Some((request, pixels)) = view.request {
                match self.backend.session.status(request) {
                    Some(RequestStatus::Ready) => {
                        if let Some(lease) = (view.accepted != Some(request))
                            .then(|| self.backend.session.take_ready(request))
                            .flatten()
                        {
                            if let Some(bitmap) = lease.payload::<Arc<Bitmap>>() {
                                self.bitmaps.insert(*id, bitmap.clone());
                                let natural = bitmap.natural();
                                reports.push((*id, Some((natural.0 as f32, natural.1 as f32))));
                                view.lease = Some(lease);
                                view.accepted = Some(request);
                                view.displayed_source = view.source.clone();
                            }
                        }
                    }
                    Some(RequestStatus::Failed(e)) => {
                        view.refusal = Some(e);
                        self.backend.cancel(request);
                        view.request = None;
                        continue;
                    }
                    Some(RequestStatus::WaitingBudget) => {
                        let stats = self.backend.session.stats();
                        admission_budget =
                            SESSION_BYTES - stats.resident_bytes - stats.reserved_bytes;
                        let smaller = plan(header, view.desired, admission_budget);
                        if smaller.as_ref().is_ok_and(|p| p.pixels != pixels) {
                            self.backend.cancel(request);
                            view.request = None;
                        }
                    }
                    None => {
                        self.backend.forget(request);
                        view.request = None;
                    }
                    _ => {}
                }
            }
            if let Some((request, _)) = view.request {
                // A changed offered size replaces demand, retaining old pixels.
                if resize_changes_decode(header, view.requested_for, view.desired) {
                    self.backend.cancel(request);
                    view.request = None;
                }
            }
            if view.request.is_none() && !matches!(view.refusal, Some(Refusal::DecodeFailed)) {
                let decode = match plan(header, view.desired, admission_budget) {
                    Ok(plan) => plan,
                    Err(e) => {
                        view.refusal = Some(e);
                        continue;
                    }
                };
                let demand = Demand {
                    view: view.key,
                    key: RasterKey {
                        source,
                        generation: self.generation,
                        pixels: decode.pixels,
                        variant: 1,
                    },
                    metadata: header.metadata,
                    cost: decode.cost,
                    priority: Priority::Visible,
                };
                match self.backend.request(demand) {
                    Ok(request) => {
                        view.request = Some((request, decode.pixels));
                        view.requested_for = view.desired;
                        view.refusal = None;
                    }
                    Err(e) => view.refusal = Some(e),
                }
            }
        }
        self.loaded = self
            .views
            .iter()
            .filter_map(|(id, view)| {
                self.bitmaps
                    .get(id)
                    .map(|b| (view.displayed_source.clone(), b.natural()))
            })
            .collect();
        reports
    }
    /// Pending work, including capacity waits; this is not an animation request.
    pub fn pending(&self) -> bool {
        self.views.values().any(|v| {
            !v.source.is_empty()
                && match v
                    .source_id
                    .as_ref()
                    .and_then(|source| self.backend.prepared(source.id))
                {
                    Some(Prepared::Ready(_)) => {
                        v.visible
                            && self.decode_enabled
                            && match v.request {
                                Some((id, _)) => match self.backend.session.status(id) {
                                    Some(
                                        RequestStatus::Queued
                                        | RequestStatus::WaitingBudget
                                        | RequestStatus::Decoding,
                                    ) => true,
                                    Some(RequestStatus::Ready) => v.accepted != Some(id),
                                    _ => false,
                                },
                                None => matches!(
                                    v.refusal,
                                    None | Some(Refusal::QueueFull | Refusal::Budget)
                                ),
                            }
                    }
                    Some(Prepared::Failed(_)) => false,
                    _ => true,
                }
        })
    }
    /// Wait only for metadata/integrity; decode capacity never rejects activation.
    pub(crate) fn prepare_metadata(
        &mut self,
        kernel: &Kernel,
        live: &[ViewId],
        timeout: Duration,
    ) -> bool {
        self.decode_enabled = false;
        self.sync(kernel, live);
        let start = Instant::now();
        loop {
            self.poll();
            if self.views.values().all(|v| {
                v.source.is_empty()
                    || matches!(
                        v.source_id
                            .as_ref()
                            .and_then(|source| self.backend.prepared(source.id)),
                        Some(Prepared::Ready(_) | Prepared::Failed(_))
                    )
            }) {
                return true;
            }
            if start.elapsed() >= timeout {
                return false;
            }
            self.pause_wait(
                timeout
                    .saturating_sub(start.elapsed())
                    .min(Duration::from_millis(20)),
            );
        }
    }
    pub(crate) fn enable_decode(&mut self) {
        self.decode_enabled = true;
    }
    /// Wait for available work until the deadline, without spinning on capacity.
    pub fn wait(&mut self, timeout: Duration) -> Vec<Report> {
        let start = Instant::now();
        let mut reports = self.poll();
        while self.pending() && start.elapsed() < timeout {
            self.pause_wait(
                timeout
                    .saturating_sub(start.elapsed())
                    .min(Duration::from_millis(20)),
            );
            reports.extend(self.poll());
        }
        reports
    }
    fn pause_wait(&self, duration: Duration) {
        let lock = self.backend.revision.lock().unwrap();
        drop(self.backend.changed.wait_timeout(lock, duration).unwrap());
    }
    /// Retire this generation. Old worker/backing ownership remains charged.
    pub fn reset(&mut self) {
        self.backend.retire(self.generation);
        self.views.clear();
        self.bitmaps.clear();
        self.loaded.clear();
        self.generation = self.backend.generation();
    }
    pub(crate) fn wake_fd(&self) -> std::os::fd::RawFd {
        self.backend.wake_fd()
    }
    /// Raster-only counters; encoded resolver caches and GPU driver memory differ.
    pub fn diagnostics(&self) -> serde_json::Value {
        let s = self.stats();
        let g = Gate::process().stats();
        let mut reasons = BTreeMap::<String, usize>::new();
        for reason in self.views.values().filter_map(|v| v.refusal) {
            *reasons.entry(format!("{reason:?}")).or_default() += 1;
        }
        let waiting = self
            .views
            .values()
            .filter(|v| {
                v.request.is_some_and(|(id, _)| {
                    self.backend.session.status(id) == Some(RequestStatus::WaitingBudget)
                })
            })
            .count();
        serde_json::json!({"residentBytes":s.resident_bytes,"reservedBytes":s.reserved_bytes,
            "pinnedBytes":s.pinned_bytes,"coldBytes":s.cold_bytes,"retiringBytes":s.retiring_bytes,
            "peakBytes":s.peak_bytes,"running":s.running,"processRunning":g.running,
            "ready":s.ready,"deliveryCells":s.delivery_cells,"pendingJobs":s.pending_jobs,
            "subscribers":s.subscribers,"coldEntries":s.cold_entries,"sourceEntries":self.backend.sources(),
            "liveDiagnostics":self.loaded.len(),"deferred":self.deferred,
            "budgetWaits":waiting,"refusalReasons":reasons,
            "refused":self.views.values().filter(|v| v.refusal.is_some()).count()})
    }
}
impl Drop for Images {
    fn drop(&mut self) {
        self.backend.retire(self.generation);
    }
}

fn plan(
    header: png_decode::Header,
    desired: (f32, f32),
    available: u64,
) -> Result<png_decode::DecodePlan, Refusal> {
    let natural = header.metadata.natural;
    let scale = if desired.0 > 0. || desired.1 > 0. {
        (desired.0 / natural.width as f32)
            .max(desired.1 / natural.height as f32)
            .min(1.)
    } else {
        1.
    }
    .min(2048. / natural.width.max(natural.height) as f32);
    let longest = natural.width.max(natural.height);
    let bucket = ((longest as f32 * scale).ceil().max(1.) as u32)
        .div_ceil(128)
        .saturating_mul(128)
        .min(longest);
    let mut pixels = (
        (u64::from(natural.width) * u64::from(bucket)).div_ceil(u64::from(longest)) as u32,
        (u64::from(natural.height) * u64::from(bucket)).div_ceil(u64::from(longest)) as u32,
    );
    loop {
        let candidate = png_decode::DecodePlan::new(header, pixels);
        if candidate
            .as_ref()
            .is_ok_and(|p| p.peak_bytes() <= available)
            || pixels == (1, 1)
        {
            return candidate;
        }
        pixels = (pixels.0.div_ceil(2), pixels.1.div_ceil(2));
    }
}

fn resize_changes_decode(
    header: png_decode::Header,
    previous: (f32, f32),
    next: (f32, f32),
) -> bool {
    let (Ok(before), Ok(after)) = (
        plan(header, previous, SESSION_BYTES),
        plan(header, next, SESSION_BYTES),
    ) else {
        return false;
    };
    let before = before.pixels.width.max(before.pixels.height);
    let after = after.pixels.width.max(after.pixels.height);
    u64::from(after) * 4 > u64::from(before) * 5 || u64::from(after) * 2 < u64::from(before)
}
