//! One explicitly contained content publication, with UI-owned offer discovery.
//!
//! This is a kernel trial, not a native paint/selection or worker implementation.
//! Ordinary callbacks and the default profile retain final-only artifact semantics.
//! The explicit split profile separates exact measurement facts from final owners.
//! Hosts must paint
//! the selected publication, never read candidate source through live NodeRefs
//! while displaying an older publication. Opaque artifact bytes are host-budgeted;
//! the kernel bounds request counts, captured source bytes, and geometry entries.
mod state;
mod tree;
use crate::text::{Paragraph, TextRun, TextStyle};
use crate::{
    Frame, LayoutError, LayoutReceipt, NodeKey, Offer, ParagraphStamp, TextMeasureRequest,
    TextMetrics,
};
pub(crate) use state::RegionState;
use std::{
    any::Any,
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Weak},
};

/// Maximum mounted nodes in either region branch (not logical document rows).
pub const REGION_NODES: usize = 4096;
/// Maximum distinct exact offers pinned by one candidate/publication.
pub const REGION_OFFERS: usize = 64;
/// Captured UTF-8 bytes per candidate/publication, excluding host artifacts.
pub const REGION_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Explicit count policy. SplitFacts is not native byte admission or activation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegionProfile {
    /// Existing64 retained exact-offer artifacts; no ownership-policy change.
    #[default]
    PinnedOffers,
    /// At most768 scalar facts and192 sources/final owners per generation.
    /// Two live reservations include externally held requests/artifacts/publications.
    /// Native adapters must separately admit bytes, external owners and paint context.
    SplitFacts,
}
/// What the current private request must deliver. Tuple equivalence alone is
/// insufficient: a final request is fresh even for an already measured offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionRequestPurpose {
    /// Default profile: keep the artifact for every measured offer.
    RetainedOffer,
    /// Split profile: validate scalar metrics, release the supplied payload.
    Measurement,
    /// Split profile: retain final paint ownership with bit-identical metrics.
    FinalPaint,
}
const SPLIT_FACTS: usize = 768;
const SPLIT_PAINTS: usize = 192;
// Runtime-local two-slot admission; never a worker, waiter list or wake source.
// It survives registration/reset in Kernel. Tokens themselves are Send and
// payload-free, so request transport cannot carry UI ownership to a worker.
#[derive(Clone, Default)]
pub(crate) struct RegionLeases(Rc<RefCell<[Weak<()>; 2]>>);
impl RegionLeases {
    pub fn has_live(&self) -> bool {
        self.0.borrow().iter().any(|w| w.strong_count() != 0)
    }
    fn reserve(&self) -> Option<Arc<()>> {
        let mut slots = self.0.borrow_mut();
        let slot = slots.iter_mut().find(|w| w.strong_count() == 0)?;
        let lease = Arc::new(());
        *slot = Arc::downgrade(&lease);
        Some(lease)
    }
}
// Stamp/source/catalog provenance is stored once in the set/source table.
// No native payload or per-fact Arc; two typed axes and optional baseline stay exact.
#[derive(Clone, Copy)]
struct ScalarFact {
    source: u16,
    offer: Offer,
    metrics: TextMetrics,
}
const _: () = assert!(std::mem::size_of::<ScalarFact>() <= 48);
#[derive(Default)]
struct FactSet {
    entries: Vec<ScalarFact>,
    sources: Vec<Arc<RegionTextSource>>,
}
impl FactSet {
    fn find(&self, stamp: &ParagraphStamp, offer: Offer) -> Option<usize> {
        self.entries.iter().position(|f| {
            self.sources[f.source as usize].stamp == *stamp && same_offer(f.offer, offer)
        })
    }
    fn push(&mut self, fact: ScalarFact) {
        // One exact requested allocation; no geometric growth beyond M768.
        if self.entries.capacity() == 0 {
            self.entries.reserve_exact(SPLIT_FACTS);
        }
        assert!(self.entries.len() < SPLIT_FACTS);
        self.entries.push(fact);
    }
}
fn same_offer(a: Offer, b: Offer) -> bool {
    use crate::AxisOffer::*;
    let same = |a, b| match (a, b) {
        (Definite(a), Definite(b)) => a.to_bits() == b.to_bits(),
        (MinContent, MinContent) | (MaxContent, MaxContent) => true,
        _ => false,
    };
    same(a.width, b.width) && same(a.height, b.height)
}
fn same_metrics(a: TextMetrics, b: TextMetrics) -> bool {
    a.width.to_bits() == b.width.to_bits()
        && a.height.to_bits() == b.height.to_bits()
        && a.first_baseline.map(f32::to_bits) == b.first_baseline.map(f32::to_bits)
}

/// Explicit direct children of an independently sized, clipped View.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentRegion {
    /// The fixed external box; candidate intrinsic sizes cannot size it.
    pub owner: NodeKey,
    /// The candidate content child.
    pub content: NodeKey,
    /// The real authored first-load placeholder child.
    pub pending: NodeKey,
}
/// External identities that are not the global authored/typing epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionInputs {
    /// Host-owned font/catalog identity. Advance before using a changed catalog.
    pub catalog: u64,
    /// Consumer source-map/collection snapshot identity, including row epochs.
    pub consumer_revision: u64,
}
/// Opaque lifetime identity for one source/offer/catalog candidate.
#[derive(Clone, Debug)]
pub struct RegionTicket(Arc<()>);
impl PartialEq for RegionTicket {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}
impl Eq for RegionTicket {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextKey {
    stamp: ParagraphStamp,
    offer: Offer,
}
/// One immutable first-missing request. Cloning shares its snapshot; no worker
/// sees the arena or computes geometry. Only the current request may complete.
#[derive(Clone)]
pub struct RegionTextRequest(Arc<RequestData>);
struct RequestData {
    ticket: RegionTicket,
    key: TextKey,
    catalog: u64,
    source: Arc<RegionTextSource>,
    purpose: RegionRequestPurpose,
    _lease: Option<Arc<()>>,
}
/// One immutable canonical source per full paragraph stamp, shared by all exact
/// offers and widths while the full stamp is unchanged. It retains no Kernel,
/// Arena or worker. This is metric input only: color, decoration, href and native
/// source/selection maps are NOT included. A future native adapter must capture
/// that metadata at this SAME full stamp before queuing work, and retain it with
/// the returned shape. Kernel marker payloads do not prove native painting.
pub struct RegionTextSource {
    stamp: ParagraphStamp,
    paragraph: Paragraph,
    runs: Vec<(Box<str>, TextStyle)>,
    bytes: usize,
}
impl RegionTextSource {
    /// Retained UTF-8 length. Host artifacts and vector/string spare capacity are
    /// separate; the core also bounds run/node/offer counts.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    /// Exact source/paint/metric revision of this owned snapshot.
    pub fn stamp(&self) -> &ParagraphStamp {
        &self.stamp
    }
}
impl RegionTextRequest {
    /// Retention/delivery policy of this exact private request.
    pub fn purpose(&self) -> RegionRequestPurpose {
        self.0.purpose
    }
    /// Candidate provenance, independent of sibling typing epochs.
    pub fn ticket(&self) -> &RegionTicket {
        &self.0.ticket
    }
    /// Exact canonical paragraph identity (including paint/source revision).
    pub fn stamp(&self) -> &ParagraphStamp {
        &self.0.key.stamp
    }
    /// Exact typed offer discovered on the UI executor.
    pub fn offer(&self) -> Offer {
        self.0.key.offer
    }
    /// Font/catalog identity that must select the worker's shaping context.
    pub fn catalog(&self) -> u64 {
        self.0.catalog
    }
    /// Shared canonical source; multiple offers do not duplicate UTF-8.
    pub fn source(&self) -> &Arc<RegionTextSource> {
        &self.0.source
    }
    /// Borrow the owned immutable input for manual/worker measurement.
    pub fn with_request<T>(&self, f: impl FnOnce(&TextMeasureRequest<'_>) -> T) -> T {
        let runs: Vec<_> = self
            .0
            .source
            .runs
            .iter()
            .map(|(text, style)| TextRun {
                text,
                style: *style,
            })
            .collect();
        f(&TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: self.0.source.paragraph,
            width: self.offer().width,
            height: self.offer().height,
        })
    }
}
/// Final metrics and an owned opaque source/shape artifact for one exact offer.
/// The payload must own everything a native painter/selection reader needs;
/// passing a metrics-only marker is suitable only for kernel tests.
#[derive(Clone)]
pub struct RegionArtifact {
    request: RegionTextRequest,
    metrics: TextMetrics,
    payload: Rc<dyn Any>,
}
impl RegionArtifact {
    /// Exact source/offer proof of this retained artifact.
    pub fn request(&self) -> &RegionTextRequest {
        &self.request
    }
    /// Final metrics associated with the same artifact.
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }
    /// The retained native artifact, borrowed without copying.
    pub fn payload<T: Any>(&self) -> Option<&T> {
        self.payload.downcast_ref()
    }
}
/// Immutable frame facts, kept even after a key is destroyed. `frames()` returns
/// origin-zero coordinates; `projected_frames(origin)` returns world coordinates
/// accumulated parent-first at that origin. Neither changes keys or extents.
#[derive(Clone, Debug)]
pub struct RegionFrame {
    /// Original generation; never resurrects arena membership or action routing.
    pub node: NodeKey,
    /// Border box in the coordinate space selected by the producing accessor.
    pub frame: Frame,
    /// Local scrollable content extent from the same pass.
    pub content: (f32, f32),
    // LLP 1043.000 D3: proof from the engine that produced this frame.
    pub(crate) height_measured: bool,
}

// A bounded coordinate witness, not retained layout-engine state. Parent is an
// earlier paint-order ordinal, or OWNER for a direct child of the omitted owner.
const OWNER: u32 = u32::MAX;
#[derive(Clone, Copy)]
pub(crate) struct RegionOffset {
    parent: u32,
    x: f32,
    y: f32,
    inline: bool,
}
impl RegionOffset {
    fn project(self, local: Frame, parent: Frame) -> Result<Frame, LayoutError> {
        let frame = if self.inline {
            Frame::default()
        } else {
            Frame {
                x: parent.x + self.x,
                y: parent.y + self.y,
                ..local
            }
        };
        if !frame.x.is_finite() || !frame.y.is_finite() {
            return Err(LayoutError::ContentRegion("projection overflow"));
        }
        Ok(frame)
    }
}

#[derive(Clone, Default)]
pub(crate) struct RegionGeometry {
    frames: Vec<RegionFrame>,
    offsets: Vec<RegionOffset>,
}
impl RegionGeometry {
    fn validate(&self, origin: Frame) -> Result<(), LayoutError> {
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(LayoutError::ContentRegion("projection overflow"));
        }
        if self.frames.len() > REGION_NODES || self.frames.len() != self.offsets.len() {
            return Err(LayoutError::ContentRegion("invalid projection geometry"));
        }
        Ok(())
    }

    fn project(&self, origin: Frame) -> Result<Vec<RegionFrame>, LayoutError> {
        self.validate(origin)?;
        let mut projected: Vec<RegionFrame> = Vec::with_capacity(self.frames.len());
        for (local, offset) in self.frames.iter().zip(&self.offsets) {
            let parent = if offset.parent == OWNER {
                origin
            } else {
                projected
                    .get(offset.parent as usize)
                    .ok_or(LayoutError::ContentRegion("invalid projection parent"))?
                    .frame
            };
            projected.push(RegionFrame {
                node: local.node,
                frame: offset.project(local.frame, parent)?,
                content: local.content,
                height_measured: local.height_measured,
            });
        }
        Ok(projected)
    }

    fn frame(&self, node: NodeKey, origin: Frame) -> Option<Frame> {
        self.validate(origin).ok()?;
        let mut index = self.frames.iter().position(|f| f.node == node)?;
        let mut chain = Vec::new();
        loop {
            chain.push(index);
            let parent = self.offsets[index].parent;
            if parent == OWNER {
                break;
            }
            // Strictly decreasing ordinals bound the walk and refuse cycles.
            if parent as usize >= index {
                return None;
            }
            index = parent as usize;
        }
        let mut frame = origin;
        for index in chain.into_iter().rev() {
            frame = self.offsets[index]
                .project(self.frames[index].frame, frame)
                .ok()?;
        }
        Some(frame)
    }
}
/// Immutable accepted geometry plus the exact pinned artifacts that produced it.
#[derive(Clone)]
pub struct RegionPublication {
    ticket: RegionTicket,
    inputs: RegionInputs,
    geometry: Rc<RegionGeometry>,
    facts: Arc<FactSet>,
    _lease: Option<Arc<()>>,
    artifacts: Vec<RegionArtifact>,
    paints: Vec<(NodeKey, usize)>,
}
impl RegionPublication {
    /// The accepted source/offer lifetime proof.
    pub fn ticket(&self) -> &RegionTicket {
        &self.ticket
    }
    /// Accepted catalog and consumer snapshot. Never replace these with current
    /// row epochs when displaying an older publication.
    pub fn inputs(&self) -> RegionInputs {
        self.inputs
    }
    /// Origin-zero flattened frames in paint order. Adding a nonzero origin to
    /// these flattened coordinates is not bit-equivalent to ordinary layout's
    /// parent-first f32 accumulation. Use `projected_frames` or `frame` for that
    /// projection; these local coordinates remain immutable source geometry.
    pub fn frames(&self) -> &[RegionFrame] {
        &self.geometry.frames
    }
    /// Default: all retained offers. Split profile: final paint owners only.
    pub fn artifacts(&self) -> &[RegionArtifact] {
        &self.artifacts
    }
    /// Artifact at this publication's final paragraph inner width. Native
    /// painters must consume this owner, never cold-shape current live source.
    pub fn paint_artifact(&self, node: NodeKey) -> Option<&RegionArtifact> {
        self.paints
            .iter()
            .find(|(k, _)| *k == node)
            .map(|(_, i)| &self.artifacts[*i])
    }
    /// Project all accepted frames in paint order with ordinary layout's exact
    /// parent-first f32 additions. Only origin.x/y are used; old widths/heights
    /// and content extents remain unchanged. Inline runs remain zero frames.
    ///
    /// One bounded O(W) pass, no arena reads or mutation. All results are checked
    /// before returning; an invalid/nonfinite projection returns an error, not
    /// a partial array. Native consumers must not translate a cached projection
    /// from a different origin and call it this projection.
    pub fn projected_frames(&self, origin: Frame) -> Result<Vec<RegionFrame>, LayoutError> {
        self.geometry.project(origin)
    }
    /// The same parent-first projection for one key, without squeezing its
    /// original dimensions. A bounded key scan plus ancestor walk; unknown keys
    /// or nonfinite/overflowing projections return None. No live arena lookup.
    pub fn frame(&self, node: NodeKey, origin: Frame) -> Option<Frame> {
        self.geometry.frame(node, origin)
    }
}
/// Explicit selection: no implicit live-content paint fallback while Pending.
#[derive(Clone)]
pub enum RegionSelection {
    /// Real authored placeholder; no accepted content yet.
    Pending(NodeKey),
    /// Owned accepted content, possibly from an older request/width/catalog.
    Accepted(Rc<RegionPublication>),
}
/// One shell publication plus selected content provenance.
#[derive(Clone)]
pub struct RegionLayoutReceipt {
    /// Shell/selected live-key frame changes for this turn.
    pub shell: LayoutReceipt,
    /// Current owner border box; also the clip (trial requires zero edges).
    pub origin: Frame,
    /// The branch a host must present.
    pub selection: RegionSelection,
    /// Whether accepted geometry matches the requested source/offer/inputs now.
    /// False means no current collection measurements, even for still-live keys.
    /// With no pending text request this can also mean both split reservations
    /// are occupied. The independent shell still publishes; a later compute may
    /// resume after an old lease drops. No timer or native wake is implied.
    pub current: bool,
}
impl RegionLayoutReceipt {
    /// A measurement-eligible frame, absent for retained older publications.
    /// Pair with `RegionPublication::inputs`, not a later collection snapshot.
    pub fn current_frame(&self, node: NodeKey) -> Option<Frame> {
        if !self.current {
            return None;
        }
        match &self.selection {
            RegionSelection::Accepted(p) => p.frame(node, self.origin),
            _ => None,
        }
    }
}

/// Retention owned by the current kernel region, excluding copies/handles held
/// by callers and opaque native payload bytes. UTF-8 is counted once per source
/// allocation within each publication/candidate, never once per offered width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionRetention {
    /// Accepted immutable source bytes (old content may still be displayed).
    pub accepted_source_bytes: usize,
    /// Candidate immutable source bytes across discovery turns.
    pub candidate_source_bytes: usize,
    /// Bytes shared by Arc between the two categories (not a second allocation).
    pub shared_source_bytes: usize,
    /// Deduplicated UTF-8 bytes owned across accepted and candidate sources.
    pub total_source_bytes: usize,
    /// Accepted artifacts: all exact offers by default, final owners in SplitFacts.
    /// Payload sizes and externally extracted native objects are host-budgeted.
    pub accepted_offers: usize,
    /// Default ready answers plus pending request; split final owners plus a
    /// pending final request. Scalar-only answers are counted separately below.
    pub candidate_offers: usize,
    /// Exact scalar facts (default: same count as accepted retained offers).
    pub accepted_facts: usize,
    /// Candidate scalar facts plus a pending measurement slot (default: offers).
    pub candidate_facts: usize,
}

#[cfg(test)]
mod split_storage_tests {
    use super::*;

    #[test]
    fn scalar_allocation_uses_exact_fixed_capacity_without_payload_owners() {
        let mut facts = FactSet::default();
        for _ in 0..SPLIT_FACTS {
            facts.push(ScalarFact {
                source: 0,
                offer: Offer::definite(1., 1.),
                metrics: TextMetrics::default(),
            });
        }
        assert_eq!(facts.entries.len(), 768);
        assert_eq!(facts.entries.capacity(), 768);
        assert_eq!(std::mem::size_of::<ScalarFact>(), 36);
        assert_eq!(
            facts.entries.capacity() * std::mem::size_of::<ScalarFact>(),
            27_648
        );
        assert_eq!(
            2 * facts.entries.capacity() * std::mem::size_of::<ScalarFact>(),
            55_296
        );
        assert!(facts.entries.capacity() * std::mem::size_of::<ScalarFact>() <= 36 * 1024);
        facts.sources.reserve_exact(SPLIT_PAINTS);
        assert_eq!(facts.sources.capacity(), 192);
        assert_eq!(std::mem::size_of::<RegionFrame>(), 36);
        assert_eq!(std::mem::size_of::<RegionOffset>(), 16);
        assert_eq!(
            REGION_NODES
                * (std::mem::size_of::<RegionFrame>() + std::mem::size_of::<RegionOffset>()),
            212_992
        );
        let mut final_owners: Vec<RegionArtifact> = Vec::new();
        final_owners.reserve_exact(SPLIT_PAINTS);
        assert_eq!(final_owners.capacity(), 192);
        let mut final_ordinals: Vec<(NodeKey, usize)> = Vec::new();
        final_ordinals.reserve_exact(SPLIT_PAINTS);
        assert_eq!(final_ordinals.capacity(), 192);
        eprintln!("scalar_fact_bytes={} fact_capacity={} source_slot_bytes={} source_capacity={} frame_bytes={} offset_bytes={}",
            std::mem::size_of::<ScalarFact>(), facts.entries.capacity(),
            std::mem::size_of::<Arc<RegionTextSource>>(), facts.sources.capacity(),
            std::mem::size_of::<RegionFrame>(), std::mem::size_of::<RegionOffset>());
    }
}
