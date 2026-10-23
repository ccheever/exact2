//! The element resize event's rounds after layout (`exact_runner`'s
//! `resize`), run before a batch is finished, so the presenter hears the
//! resized handlers' commits in the batch whose layout raised them.
use super::*;

impl<D: DataSource> Host<D> {
    /// Observe the viewport at the host's time, laying out due timer commits
    /// before resize notifications choose their depth threshold.
    pub fn resize_at(&mut self, width: f32, height: f32, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        let (batch, error) = self.settle_resize_clock(Batch::new(), None);
        self.resize_inner(width, height, batch, error)
    }

    fn settle_resize_clock(
        &mut self,
        batch: Batch,
        error: Option<String>,
    ) -> (Batch, Option<String>) {
        let advanced = self.runner.advance_timed(self.now_ms);
        let error = error.or_else(|| advanced.error.map(|e| format!("{e:?}")));
        if advanced.receipts.is_empty() {
            return (batch, error);
        }
        let (mut batch, error) = self.commit_tree(&advanced.receipts, error, batch);
        let arrange = self.arrange_after_commit(&mut batch);
        self.group_after_commit(&mut batch);
        self.persist();
        self.emit_language(&mut batch);
        self.emit_transform_drags(&mut batch);
        self.present(&mut batch, false);
        (batch, error.or(arrange))
    }

    /// Deliver the element resize event as the browser's loop does (Resize
    /// Observer 1 §3.4.1): every `resize` node whose content box changed,
    /// each round deeper than the last one's shallowest, each round's
    /// commits laid out and presented into `batch` before the next looks.
    pub(super) fn resize_rounds(
        &mut self,
        mut batch: Batch,
        mut error: Option<String>,
    ) -> (Batch, Option<String>) {
        let mut depth = 0;
        loop {
            if self.runner.resize_due(depth).is_empty() {
                break;
            }
            // Advancing can change a shallower observed box. Its receipts
            // must be laid out before gathering this round, not after delivery.
            (batch, error) = self.settle_resize_clock(batch, error);
            let due = self.runner.resize_due(depth);
            let Some(&(_, _, shallowest)) = due.first() else {
                break;
            };
            depth = shallowest + 1;
            let mut receipts = Vec::new();
            for (view, rect, _) in due {
                self.runner.resize_delivered(view, rect);
                match self.runner.dispatch(view, Event::Resize(rect)) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        error.get_or_insert(format!("{e:?}"));
                    }
                }
            }
            if receipts.is_empty() {
                continue;
            }
            let (next, e) = self.commit_tree(&receipts, None, batch);
            batch = next;
            let arrange = self.arrange_after_commit(&mut batch);
            error = error.or(e).or(arrange);
            self.persist();
            self.emit_language(&mut batch);
            self.emit_transform_drags(&mut batch);
            self.present(&mut batch, false);
        }
        self.runner.resize_settled();
        // Layout-only batches also owe SVG scenes, even without motion or a commit.
        self.svg
            .emit(self.runner.kernel(), &self.engine, &mut batch);
        (batch, error)
    }
}

impl<D: DataSource> Host<D> {
    /// IntersectionObserver queues one sampled batch per turn, unlike resize's
    /// depth loop. Run at the next pump, after the presenter applied the last
    /// batch. A callback's changed geometry is sampled on the following wake.
    pub(super) fn intersect_round(
        &mut self,
        batch: Batch,
        mut error: Option<String>,
    ) -> (Batch, Option<String>) {
        let due = self.runner.intersect_due();
        let mut receipts = Vec::new();
        for entry in due {
            if !self.runner.intersect_delivered(&entry) {
                continue;
            }
            match self.runner.dispatch(
                entry.view,
                Event::Intersect {
                    is_intersecting: entry.is_intersecting,
                    intersection_ratio: entry.intersection_ratio,
                },
            ) {
                Ok(receipt) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Err(e) => {
                    error.get_or_insert(format!("{e:?}"));
                }
            }
        }
        if receipts.is_empty() {
            return (batch, error);
        }
        let (mut batch, error) = self.commit_tree(&receipts, error, batch);
        let arrange = self.arrange_after_commit(&mut batch);
        self.persist();
        self.emit_language(&mut batch);
        self.emit_transform_drags(&mut batch);
        self.present(&mut batch, false);
        self.resize_rounds(batch, error.or(arrange))
    }
}

#[cfg(test)]
mod intersect_tests {
    use super::*;
    struct Empty;
    impl DataSource for Empty {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn resize_observes_the_hosts_clock_and_arms_its_timer_there() {
        let plan = contract::compile(
            r#"component App
  state started = false
  state observed = -1
  state ended = -1
  task finish when started
    after(100, finish)
  action finish
    ended = performanceNow()
  action resized(w: number, h: number, r: DOMRectReadOnly)
    if w > 100
      observed = performanceNow()
      started = true
  view
    view width="100%" height=20 resize=resized
"#,
        )
        .unwrap()
        .encode();
        let (mut h, _) = Host::boot(
            &plan,
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            100.,
            100.,
        )
        .unwrap();
        h.tick(500.);
        h.resize(200., 100.);
        assert_eq!(
            h.runner.slot("observed"),
            Some(&exact_plan::Value::Number(500.))
        );
        assert_eq!(h.runner.timer_due_ms(), Some(600.));
        h.advance(600.);
        assert_eq!(
            h.runner.slot("ended"),
            Some(&exact_plan::Value::Number(600.))
        );
    }

    #[test]
    fn timers_layout_before_resize_gathers_its_shallowest_notification() {
        let plan = contract::compile(
            r#"component App
  state wide = false
  state parentWidth = 0
  state childWidth = 0
  task grow mount
    after(100, grow)
  action grow
    wide = true
  action parent(w: number, h: number, r: DOMRectReadOnly)
    parentWidth = w
  action child(w: number, h: number, r: DOMRectReadOnly)
    childWidth = w
  view
    view width=(wide ? 300 : 100) resize=parent
      view width="100%" height=20 resize=child
"#,
        )
        .unwrap()
        .encode();
        let (mut h, _) = Host::boot(
            &plan,
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            100.,
            100.,
        )
        .unwrap();
        h.now_ms = 200.;
        h.resize(200., 100.);
        assert_eq!(
            h.runner.slot("parentWidth"),
            Some(&exact_plan::Value::Number(300.))
        );
        assert_eq!(
            h.runner.slot("childWidth"),
            Some(&exact_plan::Value::Number(300.))
        );
    }

    #[test]
    fn native_intersect_mount_and_scroll_without_a_scroll_handler() {
        use exact_plan::Value;
        let source = include_str!("../../../scripts/fixtures/intersect.contract");
        let plan = contract::compile(source).unwrap().encode();
        let (mut h, batch) = Host::boot(
            &plan,
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.0,
            400.0,
        )
        .unwrap();
        let batch: serde_json::Value = serde_json::from_str(&batch).unwrap();
        h.fulfill_all(vec![], 0.0);
        assert!(batch["error"].is_null(), "{batch}");
        assert_eq!(
            h.runner().slot("count").and_then(Value::as_number),
            Some(1.0)
        );
        assert_eq!(h.runner().slot("inside"), Some(&Value::Bool(false)));
        let key = h.runner().kernel().find_by_test_id("port")[0];
        let root = h.runner().kernel().arena().local_id(key.index);
        for (top, count, inside) in [(60.0, 2.0, true), (100.0, 2.0, true), (211.0, 3.0, false)] {
            h.runner_mut().scrolled(Some(root), 0.0, top);
            let batch: serde_json::Value =
                serde_json::from_str(&h.fulfill_all(vec![], 0.0)).unwrap();
            assert!(batch["error"].is_null(), "{batch}");
            assert_eq!(
                h.runner().slot("count").and_then(Value::as_number),
                Some(count)
            );
            assert_eq!(h.runner().slot("inside"), Some(&Value::Bool(inside)));
        }
    }
    #[test]
    fn native_intersect_waits_for_presented_authored_scroll() {
        use exact_plan::Value;
        let source = include_str!("../../../scripts/fixtures/intersect.contract")
            .replace("testId=\"port\"", "testId=\"port\" scrollTop=150");
        let plan = contract::compile(&source).unwrap().encode();
        let (mut h, _) = Host::boot(
            &plan,
            Empty,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.0,
            400.0,
        )
        .unwrap();
        assert_eq!(
            h.runner().slot("count"),
            Some(&Value::Number(0.0)),
            "the presenter has not applied the batch yet"
        );
        let key = h.runner().kernel().find_by_test_id("port")[0];
        let root = h.runner().kernel().arena().local_id(key.index);
        h.runner_mut().scrolled(Some(root), 0.0, 150.0);
        h.fulfill_all(vec![], 0.0);
        assert_eq!(h.runner().slot("count"), Some(&Value::Number(1.0)));
        assert_eq!(h.runner().slot("inside"), Some(&Value::Bool(true)));
        assert_eq!(h.runner().slot("ratio"), Some(&Value::Number(1.0)));
    }
}
