use super::*;

impl<D: DataSource> Presenter<D> {
    pub(in crate::presenter) fn model_page(&self) -> (f32, f32) {
        self.pending_model_scroll(PAGE)
            .map_or(self.page, |p| (self.page.0, p.top))
    }

    pub(in crate::presenter) fn arm_scroll_anchors(&mut self) {
        self.collection.cursors.retain(|&view, cursor| {
            !(cursor.anchored || cursor.staging)
                || view == PAGE
                || self
                    .host
                    .kernel()
                    .node(view)
                    .is_some_and(|n| cursor.key == Some(n.key))
        });
        let collections: BTreeSet<_> = self
            .host
            .collections_shallow()
            .iter()
            .map(|s| s.view)
            .collect();
        let model = self.collection_paint_scroll();
        let mut offsets: Vec<_> = model
            .as_ref()
            .unwrap_or(&self.scroll)
            .iter()
            .filter(|(id, _)| **id != PAGE && !collections.contains(id))
            .map(|(&id, &offset)| (id, offset))
            .collect();
        offsets.push((PAGE, self.model_page()));
        self.host.arm_scroll_anchors(&offsets, self.viewport);
    }

    pub(in crate::presenter) fn renew_scroll_anchors(&mut self, renewed: &[ViewId]) {
        for view in renewed {
            if let Some(cursor) = self.collection.cursors.get_mut(view).filter(|c| c.anchored) {
                // Keep the sequence beyond any picture submitted for the previous item.
                *cursor = Cursor {
                    key: cursor.key,
                    sequence: cursor.sequence.saturating_add(1),
                    ordinary: true,
                    anchored: true,
                    ..Cursor::default()
                };
            }
        }
    }

    pub(in crate::presenter) fn cancel_anchor_correction(&mut self, view: ViewId) {
        if let Some(cursor) = self
            .collection
            .cursors
            .get_mut(&view)
            .filter(|c| c.anchored)
        {
            self.dirty |= cursor.model_scroll.is_some();
            cursor.advance();
        }
    }

    pub(in crate::presenter) fn live_scroll_max(&self, view: ViewId) -> f32 {
        if view == PAGE {
            return (self.live_document().1 - self.viewport.1).max(0.);
        }
        self.host.kernel().node(view).map_or(0., |node| {
            self.brush
                .scroll_bounds(self.host.kernel(), self.host.content_region(), &node, None)
                .max
                .1
        })
    }

    pub(in crate::presenter) fn anchor_input_position(
        &self,
        view: ViewId,
        acknowledged: f32,
    ) -> Option<(f32, f32)> {
        let cursor = self.collection.cursors.get(&view).filter(|c| c.anchored)?;
        let shift = cursor
            .model_scroll
            .map_or(0., |p| p.rebase - cursor.rebased);
        Some((acknowledged + shift, self.live_scroll_max(view)))
    }

    /// Carry only the unapplied anchor shift across input. The full delta belongs
    /// in the live range's clamp, before the acknowledged picture clamps its copy.
    pub(in crate::presenter) fn anchor_input(
        &mut self,
        view: ViewId,
        from: (f32, f32),
        dy: f32,
        next: (f32, f32),
    ) -> Option<f32> {
        let (corrected, max) = self.anchor_input_position(view, from.1)?;
        let top = (corrected + dy).clamp(0., max);
        let cursor = self.collection.cursors.get_mut(&view)?;
        if next == from && (dy == 0. || top == corrected) {
            return Some(0.);
        }
        cursor.advance();
        if self.display.attached() && top != next.1 {
            let mut acknowledged = next;
            cursor.model_offset(
                cursor.key?,
                (next.0, top),
                true,
                Origin::Anchor,
                &mut acknowledged,
            );
            if let Some(pending) = &mut cursor.model_scroll {
                pending.rebase = top - next.1;
            }
        }
        self.dirty = true;
        Some(top - corrected)
    }

    pub(in crate::presenter) fn rebase_clamped_anchor(
        &mut self,
        view: ViewId,
        before: (f32, f32),
        after: (f32, f32),
    ) {
        // A clamp is the list's own (its range shrank under it): the reader stands where
        // it put it, and its revision stands too.
        self.reader.rebased(view, before, after);
        // It moved the input offset under any pending position too: that position's change is
        // from the clamped offset now, or a reader's next move would carry it again.
        self.advance_pending_base(view, (after.0 - before.0, after.1 - before.1));
        let Some(pending) = self
            .collection
            .cursors
            .get_mut(&view)
            .filter(|c| c.anchored)
            .and_then(|c| c.model_scroll.as_mut())
        else {
            return;
        };
        // A stale picture can narrow the input range while a newer intent waits.
        // Keep that intent's destination and move its remaining correction's base.
        pending.rebase += before.1 - after.1;
    }

    pub(in crate::presenter) fn restore_scroll_anchors(&mut self) {
        let mut corrections = self.host.scroll_anchors.take();
        for (&view, cursor) in &self.collection.cursors {
            if cursor.anchored && cursor.model_scroll.is_some() {
                corrections.entry(view).or_default();
            }
        }
        for (view, shift) in corrections {
            let (key, max) = if view == PAGE {
                (
                    NodeKey {
                        index: u32::MAX,
                        generation: 0,
                    },
                    (self.live_document().1 - self.viewport.1).max(0.),
                )
            } else {
                let Some(node) = self.host.kernel().node(view) else {
                    continue;
                };
                let bounds = self.brush.scroll_bounds(
                    self.host.kernel(),
                    self.host.content_region(),
                    &node,
                    None,
                );
                (node.key, bounds.max.1)
            };
            let cursor = self.collection.cursors.entry(view).or_default();
            cursor.bind(key, 0);
            cursor.ordinary = true;
            (cursor.anchored, cursor.staging) = (true, false);
            let before_model = cursor.model_scroll;
            let offset = if view == PAGE {
                &mut self.page
            } else {
                self.scroll.entry(view).or_default()
            };
            let was = *offset;
            let current = cursor.model_scroll.map_or(offset.1, |p| p.top);
            let top = (current + shift).clamp(0., max);
            if top == current {
                continue;
            }
            let rebase = cursor.model_scroll.map_or(cursor.rebased, |p| p.rebase) + top - current;
            let left = cursor
                .model_scroll
                .map_or(offset.0, |p| p.left.unwrap_or(offset.0));
            cursor.model_offset(
                key,
                (left, top),
                self.display.attached(),
                Origin::Anchor,
                offset,
            );
            if let Some(pending) = &mut cursor.model_scroll {
                pending.rebase = rebase;
            }
            self.reader.rebased(view, was, *offset);
            if *offset != was && view != PAGE {
                self.collection.scroll_event(view, key);
                self.executor.notify();
            }
            self.scroll_end_retargeted(view, before_model);
            self.dirty = true;
        }
    }

    /// Tell the runner where every scroller and the page stand, for
    /// `frame()` (LLP 1051.000 D1): after a clamp, and wherever a scroll
    /// moves geometry (`refresh_transform_geometry`).
    pub(crate) fn publish_scroll(&mut self) {
        self.restore_scroll_anchors();
        self.arm_scroll_anchors();
        let runner = self.host.runner_mut();
        runner.scrolled(None, f64::from(self.page.0), f64::from(self.page.1));
        for (id, (left, top)) in &self.scroll {
            runner.scrolled(Some(*id), f64::from(*left), f64::from(*top));
        }
        self.place_split();
    }
}

#[test]
fn an_unbound_scrollers_anchor_cursor_leaves_with_its_node() {
    let source = r#"component App
  state header = 48
  state visible = true
  action fold
    header = 0
  action remove
    visible = false
  view
    column width=400 height=400
      if visible
        scroll testId="port" width=400 height=400
          box height=header
          box testId="row" height=1000 press=fold dblclick=remove
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let named = |p: &Presenter<()>, name| {
        let kernel = p.host.kernel();
        kernel
            .node_by_key(kernel.find_by_test_id(name)[0])
            .unwrap()
            .id
    };
    let port = named(&p, "port");
    let row = named(&p, "row");
    p.scroll_to(port, (0., 400.)).unwrap();
    assert_eq!(p.host.dispatch_at(row, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    assert!(p.collection.cursors.contains_key(&port));
    assert_eq!(p.host.dispatch_at(row, Event::Dblclick, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
    assert!(!p.collection.cursors.contains_key(&port));
}
