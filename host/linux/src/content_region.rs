//! Explicit native content-region registration and selected publication.
//!
//! Registration happens before the first layout. Pending content is never
//! routed through the ordinary native text measurer or live-content painter.
use exact_kernel::{
    ContentRegion, Kernel, NodeKey, Offer, PropId, RegionInputs, RegionLayoutReceipt, ViewId,
};
mod controller;
mod source;
#[cfg(test)]
pub(crate) mod test_hooks;
mod worker;
#[cfg(test)]
static TEST_SERVICE: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
pub(crate) fn test_service() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_SERVICE.lock().unwrap_or_else(|p| p.into_inner());
    controller::test_wait_idle();
    guard
}
/// A dropped region's session retires in the background: a test that admits
/// a second region under one `test_service` guard waits for it first.
#[cfg(test)]
pub(crate) fn test_wait_idle() {
    controller::test_wait_idle();
}
use controller::Controller;
pub(crate) use controller::NativeText;
pub use source::TextSourceRun;

/// Borrowed paint/selection source from the selected native publication. This
/// proves source identity; platform selection controls are a separate consumer.
pub struct TextSnapshot<'a> {
    /// Immutable source, full stamp, catalog and exact final offer.
    pub request: &'a exact_kernel::RegionTextRequest,
    /// Actual accepted glyph/layout owner, never a current-source cold lookup.
    pub paragraph: &'a crate::text::Paragraph,
    /// Original byte/UTF-16/link metadata in canonical run order.
    pub runs: &'a [TextSourceRun],
    /// Accepted geometry at the current shell origin.
    pub frame: exact_kernel::Frame,
    /// Whether this publication may produce current collection measurements.
    pub current: bool,
}

/// Authored IDs supplied by the consumer's native launch entry, not test IDs.
#[derive(Clone, Copy, Debug)]
pub struct ContentRegionRegistration {
    /// Optional zero-argument authored launch action selecting the consumer's
    /// bounded branch. Runs once before any native layout or motion adoption.
    pub activate: Option<&'static str>,
    /// Independently sized and clipped owner.
    pub owner: &'static str,
    /// Direct content child.
    pub content: &'static str,
    /// Direct, real authored first-load placeholder child.
    pub pending: &'static str,
}

#[derive(PartialEq, Eq)]
struct CollectionIdentity {
    key: NodeKey,
    revision: u64,
    rows: Vec<(NodeKey, u64)>,
}

/// One registered region. Accepted native paint metadata must be attached before
/// an accepted kernel publication can be painted; no implicit live fallback.
pub struct ContentRegionState {
    binding: ContentRegion,
    inputs: RegionInputs,
    receipt: Option<RegionLayoutReceipt>,
    controller: Controller,
    dark: bool,
    paint: Option<crate::text::transfer::PaintContext>,
    incarnation: std::rc::Rc<()>,
    collections: Vec<CollectionIdentity>,
    painted: Option<(
        std::rc::Rc<exact_kernel::RegionPublication>,
        exact_kernel::Frame,
    )>,
}
impl ContentRegionState {
    pub(crate) fn register(
        kernel: &mut Kernel,
        roots: &[ViewId],
        ids: ContentRegionRegistration,
    ) -> Result<Self, String> {
        if roots.len() != 1 {
            return Err("content region requires one root".into());
        }
        let binding = ContentRegion {
            owner: unique(kernel, ids.owner)?,
            content: unique(kernel, ids.content)?,
            pending: unique(kernel, ids.pending)?,
        };
        // A busy retiring worker is an explicit registration refusal. There is
        // no unbounded waiter queue or Pending session without a wake source.
        let controller = Controller::admit()?;
        kernel
            .set_content_region(Some(binding))
            .map_err(|e| format!("content region registration: {e:?}"))?;
        Ok(Self {
            binding,
            inputs: RegionInputs {
                catalog: 0,
                consumer_revision: 0,
            },
            receipt: None,
            controller,
            dark: false,
            paint: None,
            incarnation: std::rc::Rc::new(()),
            collections: Vec::new(),
            painted: None,
        })
    }
    pub(crate) fn layout(
        &mut self,
        kernel: &mut Kernel,
        root: ViewId,
        offer: Offer,
    ) -> Result<bool, String> {
        self.validate_lifetime(kernel)?;
        if let Some(catalog) = self.controller.catalog() {
            self.inputs.catalog = catalog;
        }
        let receipt = kernel
            .compute_region_layout(root, offer, self.inputs)
            .map_err(|e| format!("content region layout: {e:?}"))?;
        let changed = !receipt.shell.changed.is_empty() || !receipt.shell.flow_changed.is_empty();
        self.receipt = Some(receipt);
        // Compute already published a coherent receipt. Work admission failure
        // is explicit status, NOT an Err after mutating published geometry.
        let _ = self.controller.reconcile(kernel, self.dark);
        Ok(changed)
    }
    pub(crate) fn validate_lifetime(&mut self, kernel: &Kernel) -> Result<(), String> {
        if [
            self.binding.owner,
            self.binding.content,
            self.binding.pending,
        ]
        .iter()
        .any(|key| kernel.node_by_key(*key).is_none())
        {
            return self
                .controller
                .record_refusal(Err("content region registration retired".into()));
        }
        Ok(())
    }
    /// Generation-bound branch keys resolved once at boot.
    pub fn binding(&self) -> ContentRegion {
        self.binding
    }
    pub(crate) fn incarnation(&self) -> &std::rc::Rc<()> {
        &self.incarnation
    }
    /// Last successful shell/selected-content publication; failures retain it.
    pub fn receipt(&self) -> Option<&RegionLayoutReceipt> {
        self.receipt.as_ref()
    }
    /// Exact source and geometry even while the live node has changed or gone.
    pub fn text_snapshot(&self, node: NodeKey) -> Option<TextSnapshot<'_>> {
        let receipt = self.receipt.as_ref()?;
        let (publication, origin) = self.painted.as_ref()?;
        let artifact = publication.paint_artifact(node)?;
        let native = artifact.payload::<NativeText>()?;
        Some(TextSnapshot {
            request: artifact.request(),
            paragraph: native.paragraph()?,
            runs: native.runs(),
            frame: publication.frame(node, *origin)?,
            current: receipt.current && self.publication_painted(),
        })
    }
    /// True only after the exact selected publication painted successfully.
    pub fn publication_painted(&self) -> bool {
        if self.refusal().is_some()
            || self
                .paint
                .is_some_and(|p| self.validate_scale(p.scale()).is_err())
        {
            return false;
        }
        match (self.receipt.as_ref(), self.painted.as_ref()) {
            (Some(receipt), Some((painted, origin))) => match &receipt.selection {
                exact_kernel::RegionSelection::Accepted(p) => {
                    std::rc::Rc::ptr_eq(p, painted) && receipt.origin == *origin
                }
                _ => false,
            },
            _ => false,
        }
    }
    /// The first CPU trial pins one exact DPR for the registration lifetime.
    /// Width/height changes remain supported; a new DPR explicitly retires work.
    pub(crate) fn configure_scale(&mut self, scale: f32) -> Result<(), String> {
        let paint = match crate::text::transfer::PaintContext::new(scale) {
            Ok(paint) => paint,
            Err(e) => {
                return self
                    .controller
                    .record_refusal(Err(format!("content raster context: {e:?}")))
            }
        };
        if self.paint == Some(paint) {
            return Ok(());
        }
        let revision = self
            .inputs
            .consumer_revision
            .checked_add(1)
            .ok_or("content consumer revision exhausted")?;
        self.paint = Some(paint);
        self.inputs.consumer_revision = revision;
        self.controller.configure_scale(scale)
    }
    pub(crate) fn validate_scale(&self, scale: f32) -> Result<(), String> {
        self.controller.validate_scale(scale)
    }
    pub(crate) fn appearance(&mut self, dark: bool) -> Result<bool, String> {
        if dark == self.dark {
            return Ok(false);
        }
        let revision = self
            .inputs
            .consumer_revision
            .checked_add(1)
            .ok_or("content consumer revision exhausted")?;
        self.inputs.consumer_revision = revision;
        self.dark = dark;
        Ok(true)
    }
    pub(crate) fn collection_context(
        &mut self,
        kernel: &Kernel,
        snapshots: &[exact_runner::CollectionSnapshot],
    ) -> Result<(), String> {
        let mut next = Vec::new();
        let mut rows = 0;
        for snapshot in snapshots {
            if !self.contains(kernel, snapshot.view) {
                continue;
            }
            let node = kernel
                .node(snapshot.view)
                .ok_or("removed region collection")?;
            let mut identities = Vec::new();
            for row in &snapshot.rows {
                rows += 1;
                if rows > exact_kernel::region::REGION_NODES {
                    return Err("region collection provenance limit".into());
                }
                let key = kernel.node(row.view).ok_or("removed region row")?.key;
                identities.push((key, row.epoch));
            }
            next.push(CollectionIdentity {
                key: node.key,
                revision: snapshot.revision,
                rows: identities,
            });
        }
        if next != self.collections {
            self.inputs.consumer_revision = self
                .inputs
                .consumer_revision
                .checked_add(1)
                .ok_or("content consumer revision exhausted")?;
            self.collections = next;
        }
        Ok(())
    }
    pub(crate) fn contains(&self, kernel: &Kernel, view: ViewId) -> bool {
        let mut at = kernel.node(view);
        while let Some(node) = at {
            if node.key == self.binding.content {
                return true;
            }
            at = node.parent.and_then(|id| kernel.node(id));
        }
        false
    }
    /// Refuse feedback from retained geometry or a different row generation.
    /// Collections outside this region continue their ordinary feedback path.
    pub fn collection_feedback_allowed(
        &self,
        kernel: &Kernel,
        snapshot: &exact_runner::CollectionSnapshot,
    ) -> bool {
        if !self.contains(kernel, snapshot.view) {
            return true;
        }
        let Some(receipt) = &self.receipt else {
            return false;
        };
        if !receipt.current || !self.publication_painted() {
            return false;
        }
        let exact_kernel::RegionSelection::Accepted(p) = &receipt.selection else {
            return false;
        };
        p.inputs() == self.inputs
            && self.collections.iter().any(
                |CollectionIdentity {
                     key,
                     revision,
                     rows,
                 }| {
                    kernel.node(snapshot.view).is_some_and(|n| n.key == *key)
                        && *revision == snapshot.revision
                        && rows.len() == snapshot.rows.len()
                        && rows.iter().zip(&snapshot.rows).all(|((key, epoch), row)| {
                            row.epoch == *epoch
                                && kernel.node(row.view).is_some_and(|n| n.key == *key)
                        })
                },
            )
    }
    /// Bounded transport occupancy; opaque font/native heaps are not byte caps.
    pub fn work_counts(&self) -> (usize, usize, usize) {
        let n = self.controller.counts();
        (n.running, n.pending, n.completed)
    }
    /// Explicit refusal. An unsuccessful registration returns HostError instead.
    pub fn refusal(&self) -> Option<&str> {
        match self.controller.phase() {
            controller::Phase::Refused(e) => Some(e),
            _ => None,
        }
    }
    #[cfg(unix)]
    pub(crate) fn completion_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.refusal().is_none().then(|| self.controller.fd())
    }
    pub(crate) fn poll(&mut self, kernel: &mut Kernel) -> Result<bool, String> {
        self.controller.poll(kernel)
    }
    pub(crate) fn first_painted(&mut self, painter: &crate::paint::Painter) -> Result<(), String> {
        self.configure_scale(painter.scale)?;
        self.painted = painter.published_region(self)?;
        self.controller.first_painted(&painter.text, painter.scale)
    }
}
fn unique(kernel: &Kernel, id: &str) -> Result<NodeKey, String> {
    if id.is_empty() {
        return Err("empty content-region authored ID".into());
    }
    let mut found = None;
    for slot in kernel.arena().iter_live() {
        let key = kernel.arena().key(slot);
        if kernel
            .node_by_key(key)
            .is_some_and(|n| n.props.str(PropId::Id) == Some(id))
            && found.replace(key).is_some()
        {
            return Err(format!("ambiguous content-region authored ID: {id}"));
        }
    }
    found.ok_or_else(|| format!("missing content-region authored ID: {id}"))
}
