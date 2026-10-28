//! A painted model position acknowledged, and a list's own carried across the reader's input
//! (theme2 2026-10-28 07:39; Excord's Android fling). Whose a position is decides both: an app's
//! lands where asked and moves the reader's revision on, as UIKit's `setContentOffset` stops a
//! glide; a list's own is no app scroll, so the reader is rebased by all of it, its revision
//! stands and its fling goes on, as UIKit's does across its own content-offset adjustment.
use super::*;
use crate::paint::ScrollBounds;

/// Whose a pending model position is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::presenter) enum Origin {
    /// An app's: a requested `scrollTop` or `scrollLeft`, a seek landing on its row, or the end
    /// a `scrollFollowEnd` list followed as its content grew.
    App,
    /// The list's own, relative to the content: an anchor kept as rows change above it (a page
    /// prepended, a row measured). Carried across a reader's move by its change.
    Anchor,
    /// The list's own, absolute: a measurement clamped to the list's range. Carried across a
    /// reader's move as a clamp of where the reader now is.
    Clamp,
}

impl<D: DataSource> Presenter<D> {
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(in crate::presenter) fn acknowledge_collection_scroll(
        &mut self,
        painted: BTreeMap<ViewId, ModelScroll>,
    ) {
        for (view, accepted) in painted {
            let Some(current) = self.pending_model_scroll(view) else {
                continue;
            };
            if current.key != accepted.key || current.sequence != accepted.sequence {
                continue;
            }
            // A newer model correction on the same input sequence survives B,
            // but the interaction base becomes precisely B's painted position.
            let offset = if view == PAGE {
                &mut self.page
            } else {
                self.scroll.entry(view).or_default()
            };
            let before = *offset;
            offset.1 = accepted.top;
            if let Some(left) = accepted.left {
                offset.0 = left;
            }
            let after = *offset;
            let moved = after != before;
            let cursor = self.collection.cursors.get_mut(&view).unwrap();
            let correction = accepted.rebase - cursor.rebased;
            cursor.rebased = accepted.rebase;
            let survives = !current.same_target(&accepted);
            // A staging cursor's work ends with the position it staged.
            let retire = !survives && cursor.staging;
            if survives {
                self.dirty = true;
            } else {
                cursor.model_scroll = None;
            }
            // An app's position keeps only the anchor's share, and moves the revision on.
            let rebased = match accepted.origin {
                Origin::App => (before.0, before.1 + correction),
                Origin::Anchor | Origin::Clamp => after,
            };
            self.reader.rebased(view, before, rebased);
            self.scroll_end_acknowledged(view, accepted, moved);
            if survives {
                // What this acknowledgement applied is no longer the newer position's to apply.
                self.advance_pending_base(view, (after.0 - before.0, after.1 - before.1));
            }
            if moved && view != PAGE {
                self.collection.scroll_event(view, accepted.key);
                self.executor.notify();
            }
            if retire {
                self.collection.cursors.remove(&view);
            }
        }
        self.finish_collection_scroll_ends();
    }

    /// The input offset moved by `by` under a pending position (an acknowledgement applied part
    /// of it, a clamp of the range): its change is from the offset now, and so is the copy
    /// `scrollend` keeps of it to recognize its acknowledgement.
    pub(in crate::presenter) fn advance_pending_base(&mut self, view: ViewId, by: (f32, f32)) {
        let Some(pending) = self
            .collection
            .cursors
            .get_mut(&view)
            .and_then(|c| c.model_scroll.as_mut())
        else {
            return;
        };
        let was = *pending;
        pending.base = (pending.base.0 + by.0, pending.base.1 + by.1);
        let now = *pending;
        self.scroll_end_model_moved(view, was, now);
    }

    /// A wheel accepted on `view` (theme2 10:25, 11:06): it retires the list's pending position
    /// at once, as any input does, before a deferred collection turn would, so the next frame's
    /// acknowledgement can't write that position over the wheel.
    pub(in crate::presenter) fn retire_for_wheel(&mut self, view: ViewId) {
        if let Some(cursor) = self
            .collection
            .cursors
            .get_mut(&view)
            .filter(|c| !c.anchored)
        {
            if cursor.model_scroll.is_some() {
                cursor.advance();
                self.dirty = true;
            }
        }
    }

    /// A reader's sample on `view`: the list's own position not yet
    /// acknowledged (a page prepended above, a measurement) is kept to carry onto the input
    /// sequence the move begins, where `collection_scroll_turn` would retire it
    /// ([`Self::carry_own_position`]).
    pub(in crate::presenter) fn keep_own_position(&mut self, view: ViewId) {
        let own = self
            .collection
            .cursors
            .get(&view)
            .filter(|c| !c.anchored)
            .and_then(|c| {
                c.model_scroll
                    .filter(|p| p.origin != Origin::App && p.sequence == c.sequence)
            });
        if let Some(own) = own {
            self.collection.carried.insert(view, own);
        }
    }

    /// The list's own position carried across a reader's move to `now`, onto the new input
    /// sequence, as `anchor_input` carries an anchored list's unapplied shift: an anchor by its
    /// change (the content it keeps moved by it), a clamp as a clamp of `now`. Without this the
    /// move retired it and the content showed moved by it (a page landing mid-fling jumped).
    pub(in crate::presenter) fn carry_own_position(&mut self, view: ViewId) {
        let own = self.collection.carried.remove(&view);
        let Some(own) = own.filter(|_| self.display.attached()) else {
            return;
        };
        let now = self.scroll_of(view);
        let target = match own.origin {
            Origin::Anchor => (
                own.left.map_or(now.0, |left| now.0 + left - own.base.0),
                now.1 + own.top - own.base.1,
            ),
            _ => {
                let clamp = |now: f32, to: f32, from: f32| {
                    if to < from {
                        now.min(to)
                    } else {
                        now.max(to)
                    }
                };
                (
                    own.left
                        .map_or(now.0, |left| clamp(now.0, left, own.base.0)),
                    clamp(now.1, own.top, own.base.1),
                )
            }
        };
        let Some(cursor) = self.collection.cursors.get_mut(&view) else {
            return;
        };
        if let (Some(key), true) = (cursor.key, target != now) {
            cursor.model_offset(key, target, true, own.origin, &mut now.clone());
            self.dirty = true;
        }
    }

    /// A list shown again before its first frame, its picture bounded by its layout's travel
    /// (`hidden_scrollers`): staged as the list's own clamp, so the first frame that shows it paints
    /// in range and its input offset stays until that frame is acknowledged. A position staged while
    /// it was hidden (an anchor kept as rows were removed above it) wins over the stale input: it is
    /// kept if in range, else clamped, and the input is clamped only when there is none. A scroller
    /// that is no collection keeps the cursor that stages it (`ordinary`). Without a display there is
    /// no frame to wait for.
    pub(in crate::presenter) fn stage_shown(
        &mut self,
        view: ViewId,
        key: NodeKey,
        bounds: ScrollBounds,
    ) {
        let input = self.scroll_of(view);
        let staged = self
            .pending_model_scroll(view)
            .filter(|p| p.key == key)
            .map(|p| (p.left.unwrap_or(input.0), p.top));
        let from = staged.unwrap_or(input);
        let to = bounds.clamp(from);
        if to == from {
            return;
        }
        if !self.display.attached() {
            self.scroll.insert(view, to);
            self.rebase_clamped_anchor(view, input, to);
            return;
        }
        let cursor = self
            .collection
            .cursors
            .entry(view)
            .or_insert_with(|| Cursor {
                ordinary: true,
                staging: true,
                ..Cursor::default()
            });
        cursor.bind(key, cursor.sequence);
        cursor.model_offset(key, to, true, Origin::Clamp, &mut input.clone());
        self.dirty = true;
    }
    /// Whether `view` has a cursor, and how many staging cursors there are.
    #[cfg(test)]
    pub(in crate::presenter) fn cursors_left(&self, view: ViewId) -> (bool, usize) {
        let cursors = &self.collection.cursors;
        (
            cursors.contains_key(&view),
            cursors.values().filter(|c| c.staging).count(),
        )
    }
}
