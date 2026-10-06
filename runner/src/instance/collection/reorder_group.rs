//! A reorder across the lists that share a `reorderGroup` (LLP 1094 D4):
//! the target's `Incoming` gap, the source's `Outgoing` close, the dragged
//! row hidden while a ghost stands for it, what a hold watches, and the
//! keyboard's steps (D9). The session itself is the runner's
//! (`runner/reorder_group.rs`); a collection holds only its own half.
use super::reorder::Offsets;
use super::*;
use exact_kernel::{Kernel, NodeKey};

/// The gap a foreign list opens for the dragged row.
#[derive(Debug)]
pub(super) struct Incoming {
    /// The dragged row's identity: once this list holds it, it has landed
    /// and fills the gap itself.
    item: String,
    /// The dragged row's height, which every row at or after the gap moves.
    extent: f64,
    /// The row the gap is before (`None` at the end); `None` until a sample
    /// or a step places it.
    before: Option<Option<String>>,
    certified: Option<ReorderGeometry>,
    stepped: bool,
    holding: bool,
    /// Closing: its rows go back to 0, then it is gone.
    closed: bool,
}

impl Collection {
    /// Whether the list holds the row whose identity is `ident`.
    pub(crate) fn holds(&self, ident: &str) -> bool {
        self.index.position(ident).is_some()
    }

    /// The preview's dragged row: its identity and its string key.
    pub(crate) fn preview_item(&self) -> Option<(String, String)> {
        let p = self.preview.as_ref()?;
        let at = self.index.position(&p.source)?;
        Some((p.source.clone(), self.keys[at].as_str()?.to_owned()))
    }

    /// The dragged row's height, which a foreign gap opens by.
    pub(crate) fn preview_extent(&self) -> Option<f64> {
        self.preview.as_ref().map(|p| p.height)
    }

    /// A grouped session's source: data that moves its rows moves the
    /// offsets with them, at once (LLP 1094 D8).
    pub(crate) fn set_grouped(&mut self) {
        if let Some(p) = &mut self.preview {
            p.grouped = true;
        }
    }

    /// The target is another list (`Outgoing`) or this one again (today's
    /// preview); either way the source's gap must be placed again.
    pub(crate) fn set_outgoing(
        &mut self,
        u: &mut Update<'_>,
        outgoing: bool,
    ) -> Result<(), InstanceError> {
        if let Some(p) = &mut self.preview {
            if p.outgoing != outgoing {
                p.outgoing = outgoing;
                p.certified = None;
                p.stepped = false;
            }
        }
        self.emit_preview(u)
    }

    /// A list the session now targets: no gap until a sample or a step.
    pub(crate) fn open_incoming(&mut self, item: String, extent: f64) {
        self.incoming = Some(Incoming {
            item,
            extent,
            before: None,
            certified: None,
            stepped: false,
            holding: false,
            closed: false,
        });
    }

    /// The gap closes: its rows go back to 0, `instant` when the rows moved
    /// in the same commit.
    pub(crate) fn close_incoming(
        &mut self,
        u: &mut Update<'_>,
        instant: bool,
    ) -> Result<(), InstanceError> {
        let Some(incoming) = &mut self.incoming else {
            return Ok(());
        };
        incoming.closed = true;
        let was = std::mem::replace(&mut self.instant, instant);
        let result = self.emit_preview(u);
        self.instant = was;
        self.incoming = None;
        result
    }

    /// Forget a target opened for a sample that could not be certified;
    /// nothing was emitted for it.
    pub(crate) fn abandon_incoming(&mut self) {
        self.incoming = None;
    }

    /// No gap here stays eligible for a drop until it is placed again.
    pub(crate) fn uncertify(&mut self) {
        if let Some(p) = &mut self.preview {
            p.certified = None;
            p.stepped = false;
        }
        if let Some(incoming) = &mut self.incoming {
            incoming.certified = None;
            incoming.stepped = false;
        }
    }

    /// The source becomes the target again (LLP 1094 D4): today's preview,
    /// placed by this sample, or left as it was when the sample is unproved.
    pub(crate) fn retarget_home(
        &mut self,
        u: &mut Update<'_>,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<bool, InstanceError> {
        let Some(p) = &mut self.preview else {
            return Ok(false);
        };
        let was = std::mem::replace(&mut p.outgoing, false);
        let accepted = self.move_preview(u, geometry, y)?;
        if !accepted {
            if let Some(p) = &mut self.preview {
                p.outgoing = was;
            }
        }
        Ok(accepted)
    }

    /// The measured gap at content offset `y`, nothing excluded; an empty
    /// list with a port certifies gap 0, the end (LLP 1094 D4). A gap
    /// outside the port, or beside a non-string key, is not certified.
    pub(crate) fn move_incoming(
        &mut self,
        u: &mut Update<'_>,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<bool, InstanceError> {
        let Some(incoming) = &mut self.incoming else {
            return Ok(false);
        };
        incoming.certified = None;
        incoming.stepped = false;
        let gap = if self.index.len() == 0 {
            (geometry.port_height > 0.0).then_some(0)
        } else {
            self.index.certified_gap(y).map_err(index_error)?
        };
        let Some(gap) = gap else {
            return Ok(false);
        };
        let top = self.index.prefix(gap).unwrap();
        if top < geometry.scroll_top || top > geometry.scroll_top + geometry.port_height {
            return Ok(false);
        }
        if gap < self.keys.len() && self.keys[gap].as_str().is_none() {
            return Ok(false);
        }
        let before = self.index.key(gap).map(str::to_owned);
        let incoming = self.incoming.as_mut().unwrap();
        incoming.before = Some(before);
        incoming.certified = Some(geometry);
        self.emit_preview(u)?;
        Ok(true)
    }

    /// A data change moved this target's rows: its gap must be certified
    /// again before a drop.
    pub(super) fn unsettle_incoming(&mut self) {
        if let Some(incoming) = self.incoming.as_mut().filter(|i| !i.holding) {
            incoming.certified = None;
            incoming.stepped = false;
        }
    }

    /// Consume the target's eligibility once: the key the row lands before
    /// (`Some(None)` at the end). The gap stays while the hold lasts.
    pub(crate) fn incoming_drop(&mut self, g: &ReorderGeometry) -> Option<Option<String>> {
        let incoming = self.incoming.as_ref()?;
        if incoming.holding
            || incoming.closed
            || !(incoming.stepped || incoming.certified.as_ref() == Some(g))
        {
            return None;
        }
        let before = match incoming.before.as_ref()? {
            Some(ident) => {
                let at = self.index.position(ident)?;
                Some(self.keys[at].as_str()?.to_owned())
            }
            None => None,
        };
        let incoming = self.incoming.as_mut().unwrap();
        incoming.holding = true;
        incoming.certified = None;
        Some(before)
    }

    pub(super) fn incoming_offsets(&self) -> Option<Offsets> {
        let incoming = self.incoming.as_ref().filter(|i| !i.closed)?;
        if self.index.position(&incoming.item).is_some() {
            // Landed: the row fills its own gap.
            return None;
        }
        let gap = match incoming.before.as_ref()? {
            Some(ident) => self.index.position(ident).unwrap_or(self.index.len()),
            None => self.index.len(),
        };
        // Every row at or after the gap moves by the row's height.
        Some(Offsets {
            source: usize::MAX,
            before: gap,
            height: incoming.extent,
            source_offset: 0.0,
        })
    }

    /// The source preview's drop when the session targets its own list:
    /// the keys, the hold kept on (LLP 1094 D8).
    pub(crate) fn hold_drop(
        &mut self,
        g: &ReorderGeometry,
    ) -> Result<Option<(String, Option<String>)>, InstanceError> {
        let Some(p) = &self.preview else {
            return Ok(None);
        };
        if p.terminal || p.holding || !(p.stepped || p.certified.as_ref() == Some(g)) {
            return Ok(None);
        }
        let Some(source) = self.index.position(&p.source) else {
            return Ok(None);
        };
        let before = match &p.before {
            Some(key) => {
                let Some(i) = self.index.position(key) else {
                    return Ok(None);
                };
                Some(self.keys[i].as_str().unwrap().to_owned())
            }
            None => None,
        };
        let item = self.keys[source].as_str().unwrap().to_owned();
        let p = self.preview.as_mut().unwrap();
        p.holding = true;
        p.certified = None;
        Ok(Some((item, before)))
    }

    /// The source enters its hold for a drop into another list.
    pub(crate) fn hold_outgoing(&mut self) {
        if let Some(p) = &mut self.preview {
            p.holding = true;
            p.certified = None;
        }
    }

    /// The hold is over (LLP 1094 D8): the previews close, at once when the
    /// rows moved in this commit, and the pin waits for `finish`.
    pub(crate) fn end_hold(
        &mut self,
        u: &mut Update<'_>,
        instant: bool,
    ) -> Result<(), InstanceError> {
        if let Some(p) = &mut self.preview {
            p.holding = false;
            p.terminal = true;
            p.certified = None;
        }
        let was = std::mem::replace(&mut self.instant, instant);
        let result = self.emit_preview(u);
        self.instant = was;
        result
    }

    /// Hide the row whose identity is `ident` (a ghost stands for it), or
    /// show every row again.
    pub(crate) fn set_hidden(
        &mut self,
        u: &mut Update<'_>,
        ident: Option<String>,
    ) -> Result<(), InstanceError> {
        self.hidden = ident;
        self.emit_preview(u)
    }

    /// The row's neighbours: the identities before and after it.
    pub(crate) fn place_of(&self, ident: &str) -> Option<(Option<String>, Option<String>)> {
        let at = self.index.position(ident)?;
        let key = |i: usize| self.index.key(i).map(str::to_owned);
        Some((at.checked_sub(1).and_then(key), key(at + 1)))
    }

    /// The mounted wrapper holding the row, if it is mounted.
    pub(crate) fn wrapper_of(&self, kernel: &Kernel, ident: &str) -> Option<NodeKey> {
        let at = self.index.position(ident)?;
        let row = self.mounted.iter().find(|r| r.position == at)?;
        Some(kernel.node(row.wrapper)?.key)
    }

    /// The slots a gap can take in this list, the dragged row left out:
    /// `(slot now, slots)`. The source's own slot is where its gap is.
    pub(crate) fn step_slot(&self) -> Option<(usize, usize)> {
        if let Some(p) = self.preview.as_ref().filter(|p| !p.outgoing) {
            let source = self.index.position(&p.source)?;
            let len = self.index.len() - 1;
            let slot = match &p.before {
                Some(key) => {
                    let b = self.index.position(key)?;
                    if b > source {
                        b - 1
                    } else {
                        b
                    }
                }
                None => len,
            };
            return Some((slot, len));
        }
        let incoming = self.incoming.as_ref()?;
        let len = self.index.len();
        let slot = match incoming.before.as_ref() {
            Some(Some(key)) => self.index.position(key).unwrap_or(len),
            _ => len,
        };
        Some((slot, len))
    }

    /// Place the gap at `slot` (clamped), as a key or a custom action does
    /// (LLP 1094 D9): no measured sample is needed for its drop.
    pub(crate) fn step_to(
        &mut self,
        u: &mut Update<'_>,
        slot: usize,
    ) -> Result<bool, InstanceError> {
        let Some((_, len)) = self.step_slot() else {
            return Ok(false);
        };
        let slot = slot.min(len);
        if let Some(p) = self.preview.as_ref().filter(|p| !p.outgoing) {
            let source = self.index.position(&p.source).unwrap();
            let at = if slot < source { slot } else { slot + 1 };
            let before = self.index.key(at).map(str::to_owned);
            let p = self.preview.as_mut().unwrap();
            p.before = before;
            p.certified = None;
            p.stepped = true;
        } else {
            let before = self.index.key(slot).map(str::to_owned);
            let Some(incoming) = &mut self.incoming else {
                return Ok(false);
            };
            incoming.before = Some(before);
            incoming.certified = None;
            incoming.stepped = true;
        }
        self.emit_preview(u)?;
        Ok(true)
    }

    /// The identity the gap is before here (`None` at the end or with no
    /// gap): `state.reorder`'s `before`.
    pub(crate) fn gap_before(&self, source: bool) -> Option<String> {
        if source {
            return self
                .preview
                .as_ref()
                .filter(|p| !p.outgoing)
                .and_then(|p| p.before.clone());
        }
        self.incoming.as_ref()?.before.clone()?
    }

    /// The key of the row a stepped gap sits before, or of the last row
    /// when it is at the end: what a step keeps in view.
    pub(crate) fn step_neighbour_key(&self) -> Option<String> {
        let before = match self.preview.as_ref().filter(|p| !p.outgoing) {
            Some(p) => p.before.clone(),
            None => self.incoming.as_ref()?.before.clone()?,
        };
        let ident = before.or_else(|| self.last_key())?;
        let at = self.index.position(&ident)?;
        self.keys[at].as_str().map(str::to_owned)
    }
}

impl ReorderStep {
    /// The step's move along a list, in slots.
    pub(crate) fn along(self) -> Option<isize> {
        match self {
            ReorderStep::Earlier => Some(-1),
            ReorderStep::Later => Some(1),
            _ => None,
        }
    }
}
