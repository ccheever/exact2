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
fn serial() -> Result<u64, String> {
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
    pub fn observe(&mut self, kernel: &Kernel, receipt: RegionLayoutReceipt) -> Result<(), String> {
        if let RegionSelection::Accepted(next) = &receipt.selection {
            if self
                .publication
                .as_ref()
                .is_none_or(|(_, old)| !Rc::ptr_eq(old, next))
            {
                self.frames_json = wire::frames(next, kernel);
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
