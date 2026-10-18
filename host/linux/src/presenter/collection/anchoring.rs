use super::*;

impl<D: DataSource> Presenter<D> {
    pub(in crate::presenter) fn model_page(&self) -> (f32, f32) {
        self.pending_model_scroll(PAGE)
            .map_or(self.page, |p| (self.page.0, p.top))
    }

    pub(in crate::presenter) fn arm_scroll_anchors(&mut self) {
        self.collection.cursors.retain(|&view, cursor| {
            !cursor.anchored
                || view == PAGE
                || self
                    .host
                    .kernel()
                    .node(view)
                    .is_some_and(|n| cursor.key == Some(n.key))
        });
        // Page input has no collection turn to invalidate a submitted intent.
        if let Some(cursor) = self.collection.cursors.get_mut(&PAGE) {
            if cursor.page_at.is_some_and(|at| at != self.page) {
                let correction = cursor
                    .model_scroll
                    .map_or(0., |p| p.rebase - cursor.rebased);
                cursor.advance();
                if correction != 0. {
                    cursor.model_offset(
                        cursor.key.unwrap(),
                        (self.page.0, self.page.1 + correction),
                        true,
                        &mut self.page,
                    );
                    cursor.model_scroll.as_mut().unwrap().rebase = correction;
                }
                cursor.page_at = Some(self.page);
            }
        }
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

    pub(in crate::presenter) fn restore_scroll_anchors(&mut self) {
        for (view, shift) in self.host.scroll_anchors.take() {
            if shift == 0. {
                continue;
            }
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
            cursor.anchored = true;
            let before_model = cursor.model_scroll;
            let offset = if view == PAGE {
                &mut self.page
            } else {
                self.scroll.entry(view).or_default()
            };
            let was = *offset;
            let current = cursor.model_scroll.map_or(offset.1, |p| p.top);
            let top = (current + shift).clamp(0., max);
            let rebase = cursor.model_scroll.map_or(cursor.rebased, |p| p.rebase) + top - current;
            let left = cursor
                .model_scroll
                .map_or(offset.0, |p| p.left.unwrap_or(offset.0));
            cursor.model_offset(key, (left, top), self.display.attached(), offset);
            if let Some(pending) = &mut cursor.model_scroll {
                pending.rebase = rebase;
            }
            cursor.page_at = (view == PAGE).then_some(*offset);
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
