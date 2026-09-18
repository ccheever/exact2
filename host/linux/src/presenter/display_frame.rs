//! One acknowledged interaction picture and one submitted picture. Headless
//! and agent frames remain immediate; no glyph/command graph is copied.
use super::*;
#[cfg(any(target_os = "linux", test))]
use crate::paint::Presentation;
use crate::paint::ScrollBounds;
use exact_kernel::{Kernel, NodeKey};
#[cfg(any(target_os = "linux", test))]
use std::cell::RefCell;
use std::rc::Rc;
#[cfg(any(target_os = "linux", test))]
use std::sync::Arc;

struct Identity {
    #[cfg(any(target_os = "linux", test))]
    origin: Rc<()>,
    #[cfg(any(target_os = "linux", test))]
    succeeded: bool,
    #[cfg(any(target_os = "linux", test))]
    activatable: bool,
}
struct Witness {
    origin: Rc<()>,
    keys: BTreeMap<ViewId, NodeKey>,
    scroll: BTreeMap<ViewId, ScrollBounds>,
    parents: BTreeMap<ViewId, Option<ViewId>>,
    document: (f32, f32),
}
#[cfg(any(target_os = "linux", test))]
struct Picture {
    paint: Presentation,
    boxes: Vec<PaintedBox>,
    witness: Witness,
    scale: u32,
}

/// The pending pixel owner. ACK consumes its metadata even if a caller retains
/// the pixel Arc for VNC, so acknowledged paragraph history cannot accumulate.
#[cfg(any(target_os = "linux", test))]
pub(crate) struct SubmittedFrame {
    pub(crate) pixels: Arc<Pixmap>,
    identity: Rc<Identity>,
    picture: RefCell<Option<Picture>>,
}
#[cfg(all(test, target_os = "linux"))]
impl SubmittedFrame {
    pub(crate) fn fixture(value: u8) -> Self {
        let mut pixels = Pixmap::new(1, 1).unwrap();
        pixels.data_mut()[0] = value;
        Self {
            pixels: Arc::new(pixels),
            identity: Rc::new(Identity {
                origin: Rc::new(()),
                succeeded: true,
                activatable: true,
            }),
            picture: RefCell::new(None),
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    origin: Rc<()>,
    active: bool,
    rendering: bool,
    pending: Option<Rc<Identity>>,
    acknowledged: Option<Witness>,
}
impl State {
    pub(super) fn blocked(&self) -> bool {
        self.pending.is_some() || self.rendering
    }
    pub(super) fn attached(&self) -> bool {
        self.active
    }
    pub(super) fn submitting(&self) -> bool {
        self.rendering
    }
    pub(super) fn new_session(&mut self) -> bool {
        // Old DMA ownership survives reload; its event releases that buffer,
        // but cannot acknowledge or hit into the replacement runtime.
        self.origin = Rc::new(());
        self.active
    }
    fn witness(&self) -> Option<&Witness> {
        self.acknowledged
            .as_ref()
            .filter(|a| Rc::ptr_eq(&a.origin, &self.origin))
    }
    pub(super) fn allows(&self, kernel: &Kernel, id: ViewId) -> bool {
        if !self.active {
            return true;
        }
        let Some(a) = self.witness() else {
            return false;
        };
        let mut at = Some(id);
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else {
                return false;
            };
            if a.keys.get(&id) != Some(&node.key)
                || a.parents.get(&id).copied() != Some(node.parent)
                || node.style.display == exact_kernel::Display::None
            {
                return false;
            }
            at = node.parent;
        }
        true
    }
    pub(super) fn bounds(&self, kernel: &Kernel, id: ViewId) -> Option<ScrollBounds> {
        if !self.active {
            return None;
        }
        Some(
            self.witness()
                .filter(|_| self.allows(kernel, id))
                .and_then(|a| a.scroll.get(&id).copied())
                .unwrap_or(ScrollBounds {
                    axes: (Overflow::Hidden, Overflow::Hidden),
                    max: (0., 0.),
                }),
        )
    }
    pub(super) fn parent(&self, kernel: &Kernel, id: ViewId) -> Option<ViewId> {
        if !self.active {
            return kernel.node(id).and_then(|n| n.parent);
        }
        self.witness()
            .filter(|_| self.allows(kernel, id))
            .and_then(|a| a.parents.get(&id).copied().flatten())
            .filter(|parent| self.allows(kernel, *parent))
    }
    pub(super) fn document(&self) -> Option<(f32, f32)> {
        self.active
            .then(|| self.witness().map_or((0., 0.), |a| a.document))
    }
}

impl<D: DataSource> Presenter<D> {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn display_frame(&mut self) -> Option<SubmittedFrame> {
        if self.display.blocked() {
            return None;
        }
        // Keep A's owners alive for replay/failure during B's paint; swapping
        // afterward moves B into its receipt, without deep-copying any payload.
        let old_paint = self.brush.presentation();
        let old_boxes = std::mem::take(&mut self.boxes);
        self.display.active = true;
        self.display.rendering = true;
        let pixels = Arc::new(self.frame());
        self.display.rendering = false;
        let identity = Rc::new(Identity {
            origin: self.display.origin.clone(),
            succeeded: self.last_frame_succeeded,
            activatable: self.last_frame_succeeded && !self.dirty,
        });
        let limits = self.collection_scroll_limits();
        let kernel = self.host.kernel();
        let mut witness = Witness {
            origin: self.display.origin.clone(),
            keys: BTreeMap::new(),
            scroll: BTreeMap::new(),
            parents: BTreeMap::new(),
            document: self.live_document(),
        };
        if self.last_frame_succeeded {
            for b in &self.boxes {
                if let Some(n) = kernel.node(b.id) {
                    witness.keys.insert(b.id, n.key);
                    witness.parents.insert(b.id, n.parent);
                    witness.scroll.insert(
                        b.id,
                        self.brush.scroll_bounds(
                            kernel,
                            self.host.content_region(),
                            &n,
                            limits.get(&b.id).copied(),
                        ),
                    );
                }
            }
        }
        let picture = Picture {
            paint: self.brush.replace_presentation(old_paint),
            boxes: std::mem::replace(&mut self.boxes, old_boxes),
            witness,
            scale: self.brush.scale.to_bits(),
        };
        self.display.pending = Some(identity.clone());
        Some(SubmittedFrame {
            pixels,
            identity,
            picture: RefCell::new(Some(picture)),
        })
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn display_complete(&mut self, frame: &SubmittedFrame) -> bool {
        if !self
            .display
            .pending
            .as_ref()
            .is_some_and(|p| Rc::ptr_eq(p, &frame.identity))
        {
            return false;
        }
        // Consume once, even after reload. A retained pixel handle never pins
        // an old native paragraph/picture after this acknowledgement.
        let picture = frame.picture.borrow_mut().take();
        if Rc::ptr_eq(&self.display.origin, &frame.identity.origin) && frame.identity.succeeded {
            if let Some(picture) = picture {
                self.brush.replace_presentation(picture.paint);
                self.boxes = picture.boxes;
                self.display.acknowledged = Some(picture.witness);
                // Clamp the newest queued intent using B's numeric bounds, not
                // live C and not B's older scroll intent. Do not clear dirty.
                self.clamp_scroll();
                if self.host.content_region().is_some() {
                    let before = self.host.content_region().unwrap().publication_painted();
                    self.last_region_frame = Some((*frame.pixels).clone());
                    self.last_region_scale = Some(picture.scale);
                    if let Some(e) = self.host.content_region_painted(&self.brush) {
                        self.host.log(e);
                    }
                    if !before && self.host.content_region().unwrap().publication_painted() {
                        self.queue_collections();
                        self.dirty |= self.collection.pending();
                    }
                }
            }
        }
        self.display.pending = None;
        if Rc::ptr_eq(&self.display.origin, &frame.identity.origin)
            && frame.identity.activatable
            && !self.activation_failed
        {
            self.activate_first_pixel();
        }
        true
    }

    pub(super) fn activate_first_pixel(&mut self) {
        self.painted = true;
        match self.host.activate_data() {
            Ok(true) => {
                if let Some(error) = self.after_commit() {
                    self.activation_failed = true;
                    self.host.log(error);
                    return;
                }
            }
            Err(error) => {
                self.activation_failed = true;
                self.host.log(error);
                return;
            }
            Ok(false) if self.host.data_pending() => return,
            Ok(false) => {}
        }
        if let Some(u) = self.updates.as_mut() {
            u.boot_succeeded();
        }
        self.sync_delivery();
    }
}
#[cfg(test)]
mod tests;
