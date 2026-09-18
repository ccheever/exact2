//! One explicitly contained content publication, with UI-owned offer discovery.
//!
//! This is a kernel trial, not a native paint/selection or worker implementation.
//! Ordinary text callbacks remain final-only. Every ready answer must retain the
//! exact immutable source/shape artifact used for those metrics. Hosts must paint
//! the selected publication, never read candidate source through live NodeRefs
//! while displaying an older publication. Opaque artifact bytes are host-budgeted;
//! the kernel bounds request counts, captured source bytes, and geometry entries.
mod state;
mod tree;
use crate::text::{Paragraph, TextRun, TextStyle};
use crate::{
    Frame, LayoutReceipt, NodeKey, Offer, ParagraphStamp, TextMeasureRequest, TextMetrics,
};
pub(crate) use state::RegionState;
use std::{any::Any, rc::Rc, sync::Arc};

/// Maximum mounted nodes in either region branch (not logical document rows).
pub const REGION_NODES: usize = 4096;
/// Maximum distinct exact offers pinned by one candidate/publication.
pub const REGION_OFFERS: usize = 64;
/// Captured UTF-8 bytes per candidate/publication, excluding host artifacts.
pub const REGION_SOURCE_BYTES: usize = 16 * 1024 * 1024;

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
/// A frame in region-local coordinates, kept even after a key is destroyed.
#[derive(Clone, Debug)]
pub struct RegionFrame {
    /// Original generation; never resurrects arena membership or action routing.
    pub node: NodeKey,
    /// Local border box, before applying the current shell origin.
    pub frame: Frame,
    /// Local scrollable content extent from the same pass.
    pub content: (f32, f32),
}
/// Immutable accepted geometry plus the exact pinned artifacts that produced it.
#[derive(Clone)]
pub struct RegionPublication {
    ticket: RegionTicket,
    inputs: RegionInputs,
    frames: Vec<RegionFrame>,
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
    /// Local frames in paint order.
    pub fn frames(&self) -> &[RegionFrame] {
        &self.frames
    }
    /// Retained exact-offer paragraph artifacts.
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
    /// Project the accepted local frame without squeezing its original width.
    pub fn frame(&self, node: NodeKey, origin: Frame) -> Option<Frame> {
        self.frames.iter().find(|f| f.node == node).map(|f| Frame {
            x: origin.x + f.frame.x,
            y: origin.y + f.frame.y,
            ..f.frame
        })
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
    /// Accepted exact-offer artifact count; payload sizes are host-budgeted.
    pub accepted_offers: usize,
    /// Ready answers plus the one pending request.
    pub candidate_offers: usize,
}
