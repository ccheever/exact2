//! Explicit preboot Apple content-region selection and native artifact lifetimes.
//! CoreText stays on Swift's serial worker. Rust owns exact request/publication
//! provenance and retains the native artifact that actually produced the metrics.
use exact_kernel::{
    ContentRegion, Kernel, NodeKey, PropId, RegionLayoutReceipt, RegionPublication,
    RegionSelection, RegionTextRequest,
};
use std::{
    any::Any,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
mod ffi;
mod wire;
pub use ffi::{NativeRegionOwner, RegionRelease};

/// Authored IDs, never test IDs; activation runs before the first layout.
#[derive(Clone, Copy, Debug)]
pub struct ContentRegionRegistration {
    /// Optional zero-argument authored action selecting the contained branch.
    pub activate: Option<&'static str>,
    /// Independently sized clipped owner.
    pub owner: &'static str,
    /// Direct content child.
    pub content: &'static str,
    /// Direct authored placeholder child.
    pub pending: &'static str,
}
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub(crate) fn serial() -> Result<u64, String> {
    SERIAL
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map(|n| n + 1)
        .map_err(|_| "region serial exhausted".into())
}
pub(crate) struct NativeArtifact {
    pub id: u64,
    pub _owner: Rc<dyn Any>,
}
pub(crate) struct Pending {
    pub id: u64,
    pub source: u64,
    pub request: RegionTextRequest,
}
pub(crate) struct RegionState {
    pub native: Option<NativeProjection>,
    pub binding: ContentRegion,
    pub incarnation: u64,
    pub pending: Option<Pending>,
    pub receipt: Option<RegionLayoutReceipt>,
    pub publication: Option<(u64, Rc<RegionPublication>)>,
    pub frames_json: String,
    sources: Vec<(exact_kernel::ParagraphStamp, u64)>,
    pub refused: Option<String>,
}
impl RegionState {
    pub fn new(
        kernel: &mut Kernel,
        registration: ContentRegionRegistration,
    ) -> Result<Self, String> {
        if kernel.roots().len() != 1 {
            return Err("content region requires one root".into());
        }
        let binding = ContentRegion {
            owner: unique(kernel, registration.owner)?,
            content: unique(kernel, registration.content)?,
            pending: unique(kernel, registration.pending)?,
        };
        kernel
            .set_content_region(Some(binding))
            .map_err(|e| format!("region registration: {e:?}"))?;
        Ok(Self {
            native: None,
            binding,
            incarnation: serial()?,
            pending: None,
            receipt: None,
            publication: None,
            frames_json: "[]".into(),
            sources: Vec::new(),
            refused: None,
        })
    }
    /// A current kernel receipt alone does not mean the native mirror selected
    /// that receipt. Failed staging may already have taken/dropped candidate B.
    pub(crate) fn selected_native_current(&self) -> bool {
        let (Some(native), Some(receipt)) = (&self.native, &self.receipt) else {
            return false;
        };
        let (Some(selected), RegionSelection::Accepted(publication)) =
            (&native.selected, &receipt.selection)
        else {
            return false;
        };
        self.refused.is_none()
            && receipt.current
            && !native.dirty
            && native.candidate.is_none()
            && selected.identity.incarnation == self.incarnation
            && selected.identity.content == self.binding.content
            && selected.identity.inputs == publication.inputs()
            && selected.identity.inputs.catalog == self.incarnation
            && selected.identity.inputs.consumer_revision == native.revision
            && Rc::ptr_eq(&selected.publication, publication)
            && selected.origin.bits_eq(receipt.origin)
            && selected
                .identity
                .ticket
                .as_ref()
                .is_none_or(|t| t == publication.ticket())
    }
    pub(crate) fn retire_native(&mut self) {
        self.pending = None;
        self.receipt = None;
        self.publication = None;
        self.frames_json.clear();
        self.sources.clear();
        self.refused = Some("native registered owner/content removed".into());
    }
    pub fn observe(&mut self, kernel: &Kernel, receipt: RegionLayoutReceipt) -> Result<(), String> {
        if let RegionSelection::Accepted(next) = &receipt.selection {
            if self
                .publication
                .as_ref()
                .is_none_or(|(_, old)| !Rc::ptr_eq(old, next))
            {
                if self.native.is_none() {
                    self.frames_json = wire::frames(next, kernel);
                }
                self.publication = Some((serial()?, next.clone()));
            }
        }
        if matches!(receipt.selection, RegionSelection::Pending(_)) {
            self.publication = None;
            self.frames_json = "[]".into();
        }
        self.receipt = Some(receipt);
        self.sources.retain(|(stamp, _)| {
            kernel
                .node_by_key(stamp.owner())
                .is_some_and(|n| n.paragraph_stamp().as_ref() == Some(stamp))
        });
        let Some(request) = kernel.region_text_request() else {
            self.pending = None;
            return Ok(());
        };
        if self
            .pending
            .as_ref()
            .is_some_and(|p| same(&p.request, request))
        {
            return Ok(());
        }
        let source = if let Some((_, id)) = self.sources.iter().find(|(s, _)| s == request.stamp())
        {
            *id
        } else {
            if self.sources.len() >= exact_kernel::region::REGION_OFFERS {
                return Err("region native source cap".into());
            }
            let id = serial()?;
            self.sources.push((request.stamp().clone(), id));
            id
        };
        self.pending = Some(Pending {
            id: serial()?,
            source,
            request: request.clone(),
        });
        Ok(())
    }
    pub fn request_json(
        &self,
        kernel: &Kernel,
        id: u64,
        known_source: u64,
    ) -> Result<String, String> {
        let p = self
            .pending
            .as_ref()
            .filter(|p| p.id == id)
            .ok_or("stale region request")?;
        wire::request(kernel, p, known_source)
    }
    pub fn json(&self, kernel: &Kernel) -> String {
        wire::state(self, kernel)
    }
}
pub(crate) fn same(a: &RegionTextRequest, b: &RegionTextRequest) -> bool {
    a.ticket() == b.ticket()
        && a.stamp() == b.stamp()
        && a.offer() == b.offer()
        && a.catalog() == b.catalog()
}
fn unique(kernel: &Kernel, name: &str) -> Result<NodeKey, String> {
    if name.is_empty() {
        return Err("empty region ID".into());
    }
    let mut found = None;
    for slot in kernel.arena().iter_live() {
        let key = kernel.arena().key(slot);
        if kernel
            .node_by_key(key)
            .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
            && found.replace(key).is_some()
        {
            return Err(format!("ambiguous region ID: {name}"));
        }
    }
    found.ok_or_else(|| format!("missing region ID: {name}"))
}

/// Structural/serialized projection admission, not source, CoreText or RSS limits.
/// Lower ceilings are useful for explicit refusal tests; raising these maxima is
/// deliberately not a way to bypass the first-tranche host contract.
#[derive(Clone, Copy, Debug)]
pub struct NativeProjectionLimits {
    /// Maximum selected or candidate native nodes.
    pub nodes: usize,
    /// Maximum direct child edges.
    pub child_edges: usize,
    /// Maximum mounted collections examined in this host.
    pub collections: usize,
    /// Maximum mounted rows examined in this host.
    pub collection_rows: usize,
    /// Maximum borrowed traversal entries, including row-root lookup scratch.
    pub traversal_entries: usize,
    /// Conservative bytes for one selected/candidate wire payload.
    pub packet_wire_bound: usize,
    /// Conservative bytes for a fully staged region diff.
    pub diff_wire_bound: usize,
}
impl Default for NativeProjectionLimits {
    fn default() -> Self {
        Self {
            nodes: 4096,
            child_edges: 4096,
            collections: 64,
            collection_rows: 4096,
            traversal_entries: 4096,
            packet_wire_bound: 8 * 1024 * 1024,
            diff_wire_bound: 16 * 1024 * 1024,
        }
    }
}
impl NativeProjectionLimits {
    pub(crate) fn validate(self) -> Result<Self, String> {
        let max = Self::default();
        if self.nodes == 0
            || self.nodes > max.nodes
            || self.child_edges > max.child_edges
            || self.collections > max.collections
            || self.collection_rows > max.collection_rows
            || self.traversal_entries > max.traversal_entries
            || self.traversal_entries == 0
            || self.packet_wire_bound > max.packet_wire_bound
            || self.packet_wire_bound == 0
            || self.diff_wire_bound > max.diff_wire_bound
            || self.diff_wire_bound == 0
        {
            return Err("native projection limits".into());
        }
        Ok(self)
    }
}

pub(crate) struct NativeHeader {
    pub key: NodeKey,
    pub id: exact_kernel::ViewId,
    pub kind: &'static str,
    pub inline_owner: Option<exact_kernel::ViewId>,
    // Listener names are NOT retained action authority. Runner dispatch remains live.
    pub handlers: Box<[&'static str]>,
}
pub(crate) struct NativeIdentity {
    pub incarnation: u64,
    pub content: NodeKey,
    pub inputs: exact_kernel::RegionInputs,
    pub ticket: Option<exact_kernel::RegionTicket>,
}
pub(crate) struct CandidateNativeNode {
    pub header: NativeHeader,
    pub mirror: crate::host::Mirror,
}
pub(crate) struct CandidateNative {
    pub identity: NativeIdentity,
    pub nodes: Vec<CandidateNativeNode>,
    pub collections: Vec<exact_runner::CollectionSnapshot>,
    pub charge: ProjectionSize,
}
pub(crate) struct SelectedNative {
    pub identity: NativeIdentity,
    pub serial: u64,
    pub publication: Rc<RegionPublication>,
    pub origin: exact_kernel::Frame,
    pub headers: Vec<NativeHeader>,
    pub collections: Vec<exact_runner::CollectionSnapshot>,
    pub charge: ProjectionSize,
}
#[derive(Default, Clone, Copy)]
pub(crate) struct ProjectionSize {
    pub nodes: usize,
    pub child_edges: usize,
    pub wire_upper: usize,
}
pub(crate) struct NativeProjection {
    pub limits: NativeProjectionLimits,
    pub selected: Option<SelectedNative>,
    pub candidate: Option<CandidateNative>,
    pub revision: u64,
    pub dirty: bool,
}
impl NativeProjection {
    pub fn new(limits: NativeProjectionLimits) -> Result<Self, String> {
        Ok(Self {
            limits: limits.validate()?,
            selected: None,
            candidate: None,
            revision: 0,
            dirty: true,
        })
    }
}
/// Borrowed ancestry only: no child Vec or copied source before admission.
pub(crate) fn within(kernel: &Kernel, key: NodeKey, root: NodeKey) -> bool {
    if kernel.node_by_key(key).is_none() {
        return false;
    }
    let arena = kernel.arena();
    let mut slot = Some(key.index);
    while let Some(s) = slot {
        if arena.key(s) == root {
            return true;
        }
        slot = arena.parent(s);
    }
    false
}
/// Count/price borrowed values before constructing owned props/style/topology.
/// Fixed per-row allowances cover numeric spelling, names, inherited/derived
/// border rows, listener names, op punctuation and duplicated diff keys.
pub(crate) fn projection_size(
    kernel: &Kernel,
    root: NodeKey,
    limits: NativeProjectionLimits,
    handler_count: usize,
) -> Result<ProjectionSize, String> {
    use exact_kernel::{NodeType, PropValue, RowValue, StyleMask};
    if handler_count > limits.traversal_entries {
        return Err("native handler cap".into());
    }
    let mut size = ProjectionSize::default();
    for slot in kernel.arena().iter_live() {
        let key = kernel.arena().key(slot);
        if !within(kernel, key, root) {
            continue;
        }
        let node = kernel.node_by_key(key).ok_or("native node removed")?;
        if matches!(node.node_type, NodeType::Canvas | NodeType::WebView) {
            return Err("native projection does not capture external surface state".into());
        }
        size.nodes = size.nodes.checked_add(1).ok_or("native node overflow")?;
        size.child_edges = size
            .child_edges
            .checked_add(kernel.arena().children(slot).len())
            .ok_or("native child overflow")?;
        if size.nodes > limits.nodes || size.child_edges > limits.child_edges {
            return Err("native structural capacity".into());
        }
        let mut bytes = 2048usize
            .checked_add(
                handler_count
                    .checked_mul(64)
                    .ok_or("native handler bytes overflow")?,
            )
            .ok_or("native bytes overflow")?;
        for (id, value) in node.props.iter() {
            let n = match value {
                PropValue::Str(s) => s.len(),
                _ => 96,
            };
            bytes = add_wire(
                bytes,
                n.checked_add(id.name().len())
                    .ok_or("native prop overflow")?,
                6,
            )?;
        }
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            bytes = add_wire(
                bytes,
                exact_kernel::generated::symbol(role).map_or(0, |s| s.0.len()),
                6,
            )?;
        }
        for id in node.style.mask.union(StyleMask::INHERITED).iter() {
            bytes = add_wire(bytes, id.name().len(), 6)?
                .checked_add(192)
                .ok_or("native style overflow")?;
            // computed_style clones authored non-inherited variable rows too;
            // price their storage before that clone, even if the wire skips them.
            match node.computed(id) {
                RowValue::ClipPath(p) => {
                    for (_, values) in p.commands() {
                        bytes = add_wire(bytes, values.len(), 96)?
                            .checked_add(64)
                            .ok_or("native clip overflow")?;
                    }
                }
                RowValue::Transitions(v) => bytes = add_wire(bytes, v.0.len(), 256)?,
                RowValue::Tracks(v) => bytes = add_wire(bytes, v.0.len(), 128)?,
                // Both appearances' stops, each up to eight after expansion.
                RowValue::BackgroundImage(g) => {
                    bytes = add_wire(bytes, g.gradient().map_or(0, |g| g.stops.len()), 512)?
                }
                _ => {}
            }
        }
        // style_json_for clones the whole StyleProps before masking; price
        // dormant variable storage as well, rather than assuming clear() freed it.
        for id in [
            exact_kernel::StyleId::ClipPath,
            exact_kernel::StyleId::BackgroundImage,
            exact_kernel::StyleId::Transition,
            exact_kernel::StyleId::GridTemplateColumns,
            exact_kernel::StyleId::GridTemplateRows,
        ] {
            if node.style.mask.has(id) {
                continue;
            }
            match node.style.get(id) {
                RowValue::ClipPath(p) => {
                    for (_, values) in p.commands() {
                        bytes = add_wire(bytes, values.len(), 96)?
                            .checked_add(64)
                            .ok_or("native clip overflow")?;
                    }
                }
                RowValue::Transitions(v) => bytes = add_wire(bytes, v.0.len(), 256)?,
                RowValue::Tracks(v) => bytes = add_wire(bytes, v.0.len(), 128)?,
                // Both appearances' stops, each up to eight after expansion.
                RowValue::BackgroundImage(g) => {
                    bytes = add_wire(bytes, g.gradient().map_or(0, |g| g.stops.len()), 512)?
                }
                _ => {}
            }
        }
        size.wire_upper = size
            .wire_upper
            .checked_add(bytes)
            .ok_or("native wire overflow")?;
        if size.wire_upper > limits.packet_wire_bound {
            return Err("native packet wire capacity".into());
        }
    }
    if size.nodes == 0 {
        return Err("native content missing".into());
    }
    size.wire_upper = add_wire(size.wire_upper, size.child_edges, 16)?;
    // Numeric collection bytes are admitted by collections_bounded's borrowed
    // count pass against the remaining wire allowance before any row copy.
    if size.wire_upper > limits.packet_wire_bound {
        return Err("native packet wire capacity".into());
    }
    Ok(size)
}
fn add_wire(total: usize, n: usize, multiplier: usize) -> Result<usize, String> {
    total
        .checked_add(n.checked_mul(multiplier).ok_or("native bytes overflow")?)
        .ok_or_else(|| "native bytes overflow".into())
}

pub(crate) fn native_collections_json(
    selected: &[exact_runner::CollectionSnapshot],
    outside: &[exact_runner::CollectionSnapshot],
    limits: NativeProjectionLimits,
) -> Result<String, String> {
    wire::native_collections(selected, outside, limits)
}
