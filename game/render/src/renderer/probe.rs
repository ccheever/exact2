//! A look frame's instruments (LLP 1127.000 D1, D4): a record of each draw as it
//! is encoded, a count of each particle draw's contributing fragments, taken by
//! the particle draw itself (`quad_coverage.rs`), an occlusion query around each
//! draw of the forward and translucent passes on native hosts, the cull's own
//! account of each item (cull.wgsl `probe`), and copies of what the culls kept.
//! Armed only for a look frame under the agent's clock. A live frame carries
//! none of it: each hook below is one `Option` check, and what a record holds is
//! built by a closure that only an armed probe calls.
//!
//! The particle counts are the same on every host. The occlusion queries only
//! fill the draw listing's `covered` column: they answer whether any sample of a
//! draw passed the depth test, not how many (Metal's visibility results are
//! Boolean in wgpu, vendor/wgpu-hal/src/metal/command.rs; WebGPU promises only
//! zero or not zero, https://www.w3.org/TR/webgpu/#occlusion), and the web has
//! none: wgpu 30's WebGPU backend does not pass a render pass's
//! `occlusion_query_set` to the browser (wgpu-30.0.1/src/backend/webgpu.rs,
//! `begin_render_pass`), which then refuses the frame's command buffer. A draw
//! past the queries' capacity, or on the web, reads as not measured.
use super::*;
use std::cell::{Cell, RefCell};

#[path = "probe_read.rs"]
mod read;
#[cfg(test)]
pub(crate) use read::GroupFact;
pub(crate) use read::{Facts, Readback, Reading};

/// WebGPU's largest query set.
const MAX_QUERIES: u32 = 4096;

/// The pass a draw belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Last frame's camera lists drawn as occluders, depth only (occlusion.rs).
    Occluders,
    /// Sun shadow cascade `n`, depth only.
    SunShadow(u8),
    /// Local (spot and point) shadow layer `n`, depth only.
    LocalShadow(u8),
    /// The opaque depth prepass.
    Prepass,
    /// The forward pass: opaque geometry, the sky, hulls, then translucency
    /// unless a split pass draws it.
    Forward,
    /// Translucency in its own pass (soft or counted particles, a scene copy).
    Translucent,
    /// Ambient occlusion, a hook's HDR post, bloom and the tone pass.
    Post,
}
impl Stage {
    pub fn name(self) -> String {
        match self {
            Stage::Occluders => "occluders".into(),
            Stage::SunShadow(n) => format!("sun shadow {n}"),
            Stage::LocalShadow(n) => format!("local shadow {n}"),
            Stage::Prepass => "depth prepass".into(),
            Stage::Forward => "forward".into(),
            Stage::Translucent => "translucent".into(),
            Stage::Post => "post".into(),
        }
    }
    /// Passes begun with the probe's query set.
    pub(crate) fn queried(self) -> bool {
        matches!(self, Stage::Forward | Stage::Translucent)
    }
}

/// What one draw drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drew {
    /// Culled draw group `group` in view `view` (0 the camera, 1–3 the
    /// cascades), with the pipeline variant the pass chose for it.
    Group {
        group: usize,
        view: u32,
        variant: Option<usize>,
    },
    /// A run of `count` quads, the frame's quad draw `draw`: particles,
    /// sprites or a placed child.
    Quads { draw: usize, count: u32 },
    /// One instance of a blended model, drawn in translucent order.
    Blended { batch: usize },
    /// Local shadow cull `cull`'s group `group` in its view `view`, with the
    /// pipeline variant the pass chose for it.
    Local {
        cull: usize,
        group: usize,
        view: u32,
        variant: Option<usize>,
    },
    /// A draw that is not an instanced batch: the sky, outline hulls, tone.
    Fixed(&'static str),
    /// Work listed as one entry, not draw by draw: a hook's own draws, the
    /// passes of bloom or ambient occlusion.
    Summary(&'static str),
}

/// One recorded draw and its occlusion query, if its pass carried one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Record {
    pub stage: Stage,
    pub drew: Drew,
    pub query: Option<u32>,
}

pub(crate) struct Probe {
    pub records: RefCell<Vec<Record>>,
    stage: Cell<Stage>,
    variant: Cell<Option<usize>>,
    pub queries: Option<wgpu::QuerySet>,
    capacity: u32,
    next: Cell<u32>,
    /// The particle draws' counts and the quad draws they count.
    pub counted: Option<(wgpu::Buffer, Vec<usize>)>,
    /// Why particle draws went uncounted this frame, if they did.
    pub uncounted: Option<&'static str>,
    /// The cull's account of each item (`Cull::encode_probe`).
    pub cull: Option<wgpu::Buffer>,
    /// Pixels to read back: false for a frame read only for its bounds.
    pub pixels: bool,
}

impl Probe {
    fn new(pixels: bool) -> Self {
        Self {
            records: RefCell::new(Vec::new()),
            stage: Cell::new(Stage::Occluders),
            variant: Cell::new(None),
            queries: None,
            capacity: 0,
            next: Cell::new(0),
            counted: None,
            uncounted: None,
            cull: None,
            pixels,
        }
    }
    /// Room for `draws` queried draws, once the frame's draws are prepared (at
    /// most WebGPU's 4096; later draws are recorded unmeasured). Native only.
    fn size(&mut self, device: &wgpu::Device, draws: usize) {
        if cfg!(target_arch = "wasm32") || draws == 0 {
            return;
        }
        self.capacity = (draws as u32).min(MAX_QUERIES);
        self.queries = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("game look coverage"),
            ty: wgpu::QueryType::Occlusion,
            count: self.capacity,
        }));
    }
    fn stage(&self, stage: Stage) {
        self.stage.set(stage);
    }
    /// The query set for a pass of `stage`: the forward and translucent passes'.
    fn set_for(&self, stage: Stage) -> Option<&wgpu::QuerySet> {
        self.queries.as_ref().filter(|_| stage.queried())
    }
    /// Record a draw about to be issued in `pass`, beginning its occlusion
    /// query when the pass carries the set. Pair with [`Probe::end`].
    fn begin(&self, pass: &mut wgpu::RenderPass<'_>, mut drew: Drew) -> bool {
        let stage = self.stage.get();
        if let Drew::Group { variant, .. } | Drew::Local { variant, .. } = &mut drew {
            *variant = self.variant.take();
        }
        let queried = stage.queried() && self.queries.is_some();
        let query = (queried && self.next.get() < self.capacity).then(|| {
            let index = self.next.get();
            self.next.set(index + 1);
            pass.begin_occlusion_query(index);
            index
        });
        self.records
            .borrow_mut()
            .push(Record { stage, drew, query });
        query.is_some()
    }
    pub fn end(pass: &mut wgpu::RenderPass<'_>, began: bool) {
        if began {
            pass.end_occlusion_query();
        }
    }
    /// Queries written this frame.
    pub fn used(&self) -> u32 {
        self.next.get()
    }
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Arm the next encode's instruments; `pixels` false reads no colour back.
    pub(crate) fn arm_probe(&mut self, pixels: bool) {
        self.probe = Some(Probe::new(pixels));
    }
    /// Once every draw group and quad of the frame is prepared (after
    /// `prepare_cull`): its occlusion queries, one for each draw the forward
    /// and translucent passes can issue.
    pub(crate) fn probe_size(&mut self) {
        if let Some(probe) = &mut self.probe {
            let draws =
                self.cull.groups.len() + self.quads.draws.len() + self.models.transparent.len();
            probe.size(&self.device, draws);
        }
    }
    pub(crate) fn probe_stage(&self, stage: Stage) {
        if let Some(probe) = &self.probe {
            probe.stage(stage);
        }
    }
    /// The query set a pass of `stage` is begun with, while armed.
    pub(crate) fn probe_queries(&self, stage: Stage) -> Option<&wgpu::QuerySet> {
        self.probe.as_ref().and_then(|p| p.set_for(stage))
    }
    /// Record the draw `drew` describes, beginning its query where its pass
    /// has one. `drew` runs only while armed.
    pub(crate) fn probe_begin(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        drew: impl FnOnce() -> Drew,
    ) -> bool {
        let Some(probe) = &self.probe else {
            return false;
        };
        self.probe_built.set(self.probe_built.get() + 1);
        probe.begin(pass, drew())
    }
    /// Record a draw without a query. `drew` runs only while armed.
    pub(crate) fn probe_note(&self, drew: impl FnOnce() -> Drew) {
        if let Some(probe) = &self.probe {
            self.probe_built.set(self.probe_built.get() + 1);
            let stage = probe.stage.get();
            probe.records.borrow_mut().push(Record {
                stage,
                drew: drew(),
                query: None,
            });
        }
    }
    /// The pipeline variant of the next group or local draw recorded.
    pub(crate) fn probe_variant(&self, variant: usize) {
        if let Some(probe) = &self.probe {
            probe.variant.set(Some(variant));
        }
    }
}
