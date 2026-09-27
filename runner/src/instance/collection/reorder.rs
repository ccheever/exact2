//! One descriptor, absolute O(W) targets, existing interaction pin lifetime.
use super::*;
use exact_kernel::{Kernel, NodeKey};

#[derive(Debug)]
pub(super) struct Preview {
    pub(super) binding: ReorderBinding,
    pub(super) token: ReorderToken,
    pub(super) source: String,
    before: Option<String>,
    height: f64,
    pub(super) terminal: bool,
    pub(super) pin_owned: bool,
    pub(super) handle: ViewId,
    certified: Option<ReorderGeometry>,
}
impl Collection {
    pub(crate) fn reorder_geometry(&self, list: NodeKey) -> Option<ReorderGeometry> {
        let g = self.geometry.as_ref()?;
        Some(ReorderGeometry {
            list,
            revision: self.revision,
            scroll_sequence: g.scroll_sequence,
            scroll_top: g.offset,
            port_width: g.port_cross,
            port_height: g.port_main,
            row_width: g.cross,
            total_extent: self.index.total_height(),
        })
    }
    pub(crate) fn reorder_binding(
        &self,
        kernel: &Kernel,
        handle: NodeKey,
    ) -> Option<ReorderBinding> {
        if !self.string_keys {
            return None;
        }
        let id = kernel.node_by_key(handle)?.id;
        let source = self.pin(Some(id))?;
        let row = self
            .mounted
            .iter()
            .find(|r| self.index.key(r.position) == Some(source.as_str()))?;
        self.keys[row.position].as_str()?;
        if !self.index.is_measured(&source) || self.index.height(row.position)? <= 0.0 {
            return None;
        }
        Some(ReorderBinding {
            handle,
            list: kernel.node(self.view)?.key,
            wrapper: kernel.node(row.wrapper)?.key,
            root: kernel.node(roots_of(&row.row.roots)[0])?.key,
            row_epoch: row.epoch,
        })
    }
    pub(crate) fn preview_binding(&self) -> Option<ReorderBinding> {
        self.preview.as_ref().map(|p| p.binding)
    }
    pub(crate) fn preview_token(&self) -> Option<ReorderToken> {
        self.preview.as_ref().map(|p| p.token)
    }
    pub(crate) fn preview_active(&self, token: ReorderToken) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|p| p.token == token && !p.terminal && p.pin_owned)
    }
    pub(crate) fn begin_preview(
        &mut self,
        u: &mut Update<'_>,
        binding: ReorderBinding,
        token: ReorderToken,
        handle: ViewId,
    ) -> Result<bool, InstanceError> {
        if self.preview.is_some()
            || self.geometry.as_ref().is_none_or(|g| {
                g.interaction_view != Some(handle)
                    || g.port_cross <= 0.0
                    || g.port_main <= 0.0
                    || g.cross <= 0.0
            })
        {
            return Ok(false);
        }
        let Some(source) = self.pin(Some(handle)) else {
            return Ok(false);
        };
        let position = self.index.position(&source).unwrap();
        let before = self.index.key(position + 1).map(str::to_owned);
        let certified = None;
        self.preview = Some(Preview {
            binding,
            token,
            source,
            before,
            height: self.index.height(position).unwrap(),
            terminal: false,
            pin_owned: true,
            handle,
            certified,
        });
        self.emit_preview(u)?;
        Ok(true)
    }
    pub(crate) fn move_preview(
        &mut self,
        u: &mut Update<'_>,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<bool, InstanceError> {
        let p = self.preview.as_mut().unwrap();
        // A live unproved final sample preserves presentation, but cannot leave
        // an older destination eligible for terminal action.
        p.certified = None;
        let source = self.index.position(&p.source).unwrap();
        let Some(gap) = self
            .index
            .certified_gap_excluding(y, source)
            .map_err(index_error)?
        else {
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
        let p = self.preview.as_mut().unwrap();
        p.before = before;
        p.certified = Some(geometry);
        self.emit_preview(u)?;
        Ok(true)
    }
    pub(crate) fn preview_drop(
        &mut self,
        u: &mut Update<'_>,
        g: &ReorderGeometry,
    ) -> Result<Option<(String, Option<String>)>, InstanceError> {
        let Some(p) = &self.preview else {
            return Ok(None);
        };
        if p.terminal || p.certified.as_ref() != Some(g) {
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
        self.end_preview(u)?;
        Ok(Some((item, before)))
    }
    // Terminal does not release the pin. Own dispatch may rebuild the row index;
    // source-key pin resolution still occurs before old mounted rows are taken.
    pub(crate) fn end_preview(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        if let Some(p) = &mut self.preview {
            p.terminal = true;
            p.certified = None;
        }
        self.emit_preview(u)
    }
    pub(crate) fn finish_preview(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        token: ReorderToken,
    ) -> Result<bool, InstanceError> {
        let Some(p) = &self.preview else {
            return Ok(false);
        };
        if p.token != token || !p.terminal {
            return Ok(false);
        }
        let release = p.pin_owned
            && self
                .geometry
                .as_ref()
                .is_some_and(|g| g.interaction_view == Some(p.handle));
        self.preview = None;
        if release {
            self.release_pins(u, frames, [false, true])?;
        }
        Ok(true)
    }
    pub(super) fn check_preview_height(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        if self.preview.as_ref().is_some_and(|p| {
            !p.terminal
                && self
                    .index
                    .position(&p.source)
                    .and_then(|i| self.index.height(i))
                    != Some(p.height)
        }) {
            self.end_preview(u)?;
        }
        Ok(())
    }
    pub(crate) fn preview_frame(
        &self,
        kernel: &Kernel,
        token: ReorderToken,
    ) -> Option<ReorderFrame> {
        let p = self.preview.as_ref()?;
        if p.token != token {
            return None;
        }
        let offsets = self.preview_offsets();
        Some(ReorderFrame {
            terminal: p.terminal,
            wrappers: self
                .mounted
                .iter()
                .filter_map(|r| {
                    Some(ReorderWrapper {
                        wrapper: kernel.node(r.wrapper)?.key,
                        root: kernel.node(roots_of(&r.row.roots)[0])?.key,
                        top: self.index.prefix(r.position)?,
                        offset: offsets.as_ref().map_or(0.0, |o| o.at(r.position)),
                    })
                })
                .collect(),
        })
    }
    fn preview_offsets(&self) -> Option<Offsets> {
        let p = self.preview.as_ref().filter(|p| !p.terminal)?;
        let source = self.index.position(&p.source)?;
        let before = match &p.before {
            Some(key) => self.index.position(key)?,
            None => self.index.len(),
        };
        Some(Offsets {
            source,
            before,
            height: p.height,
            source_offset: self.index.prefix(before)?
                - if before > source { p.height } else { 0.0 }
                - self.index.prefix(source)?,
        })
    }
    pub(super) fn emit_preview(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        if self.preview.is_none() {
            return Ok(());
        }
        let offsets = self.preview_offsets();
        for i in 0..self.mounted.len() {
            let offset = offsets
                .as_ref()
                .map_or(0.0, |o| o.at(self.mounted[i].position));
            let row = &mut self.mounted[i];
            if row.preview_target == Some(offset) {
                continue;
            }
            views::style(
                u,
                row.wrapper,
                &[
                    (
                        "translate",
                        Value::str(&format!("0px {}px", exact_num::Shortest(offset))),
                    ),
                    ("transition", Value::str("translate spring(300,30,1)")),
                ],
            )?;
            row.preview_target = Some(offset);
        }
        Ok(())
    }
    pub(super) fn lose_preview_pin(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        if let Some(p) = &mut self.preview {
            p.pin_owned = false;
        }
        self.end_preview(u)
    }
}

// Resolve keyed boundaries/prefixes once, then emit O(W) arithmetic targets.
struct Offsets {
    source: usize,
    before: usize,
    height: f64,
    source_offset: f64,
}
impl Offsets {
    fn at(&self, row: usize) -> f64 {
        if row == self.source {
            self.source_offset
        } else if self.before <= row && row < self.source {
            self.height
        } else if self.source < row && row < self.before {
            -self.height
        } else {
            0.0
        }
    }
}
