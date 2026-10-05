//! Layout receipts survive silent list settling until the presenter sees them.
use super::*;

impl<D: DataSource> Host<D> {
    pub(super) fn record_layout(&mut self, receipt: &exact_kernel::LayoutReceipt) {
        self.pending_layout
            .extend(receipt.updated.iter().chain(&receipt.flow_changed).copied());
    }

    pub(super) fn queue_layout(&mut self, id: ViewId) {
        if let Some(node) = self.runner.kernel().node(id) {
            if !node.is_inline_run() {
                self.pending_layout.insert(node.key);
            }
        }
    }

    // Logical ancestry can change an editor's inherited spelling hint without
    // moving it. Visit that affected subtree only on a topology/hint change.
    pub(super) fn queue_layout_subtree(&mut self, id: ViewId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if let Some(node) = self.runner.kernel().node(id) {
                if !node.is_inline_run() {
                    self.pending_layout.insert(node.key);
                }
                stack.extend(node.children());
            }
        }
    }
    /// Lay every root out under the viewport and emit the parent-relative
    /// frames and scroll content sizes that changed.
    pub(super) fn layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        #[cfg(test)]
        {
            self.layout_calls += 1;
        }
        self.sync_height_owner()?;
        self.sync_height_transitions()?;
        self.collect_height_samples()?;
        self.height_presented.clear();
        let epoch = self.runner.kernel().epoch();
        self.height_presented
            .extend(
                self.height_sampling
                    .iter()
                    .map(|(node, px)| exact_kernel::PresentedHeight {
                        node: *node,
                        px: *px,
                        epoch,
                    }),
            );
        let (w, h) = self.viewport;
        for root in self.runner.roots() {
            if self.content_region.is_some() {
                self.region_layout(root, Offer::definite(w, h), batch)?;
            } else {
                let receipt = self
                    .runner
                    .kernel_mut()
                    .compute_layout_presented(root, Offer::definite(w, h), &self.height_presented)
                    .map_err(|e| format!("layout: {e:?}"))?;
                // @ref LLP 1043.000 §3 D4 — geometry can move without a frame change.
                self.runner.report_flow_skipped(&receipt.flow_skipped);
                self.runner.moved(&receipt.changed);
                self.record_layout(&receipt);
            }
        }
        self.height_projection.clear();
        self.height_projection
            .extend_from_slice(&self.height_sampling);
        self.emit_layout(batch)
    }

    /// @ref LLP 1083 D3 — each sticky node's constraint, when it changed.
    /// It follows its parent's and its scroller's boxes as well as its own,
    /// so every sticky node is read again after a layout; there are few.
    fn emit_sticky(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let keys = kernel.sticky_nodes();
        if keys.is_empty() && self.stickies.is_empty() {
            return;
        }
        let mut now = IdMap::default();
        for key in keys {
            let Some(node) = kernel.node_by_key(key) else {
                continue;
            };
            if let Some(c) = kernel.sticky_constraint(key) {
                if self.stickies.get(&node.id) != Some(&c) {
                    batch.sticky(node.id, Some(&c));
                }
                now.insert(node.id, c);
            }
        }
        for id in self.stickies.keys() {
            if !now.contains_key(id) && kernel.node(*id).is_some() {
                batch.sticky(*id, None);
            }
        }
        self.stickies = now;
    }

    /// LLP 1083.000 D4: publish ranks independently of geometry, including zero.
    pub(super) fn emit_ranks(&mut self, batch: &mut Batch) {
        for (id, placed) in self.runner.kernel().paint_order() {
            if self.ranks.insert(id, placed.rank) != Some(placed.rank) {
                batch.rank(id, placed.rank);
            }
        }
    }

    /// The parent-relative frames and scroll content sizes that changed since
    /// the presenter last heard them.
    fn emit_layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        self.judge_list_moves();
        // Preserve publication order without a tree insertion for each touch.
        let mut pending: Vec<_> = std::mem::take(&mut self.pending_layout)
            .into_iter()
            .collect();
        pending.sort_unstable();
        for key in pending {
            let Some(node) = self.runner.kernel().node_by_key(key) else {
                continue;
            };
            let id = node.id;
            let native_protected = self.native_protected_id(id);
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if node.is_inline_run() {
                continue;
            }
            let m = self.mirror.entry(id).or_default();
            // Silent list settling already updated kernel flow. Compare final
            // shapes with what the presenter saw, just like frames; destroyed
            // views drop this state with their mirror, and [] clears old ink.
            // Region-owned views still receive flow invalidation even when
            // their frames come from the selected native artifact below.
            if m.flow != node.flow_shapes() {
                batch.flow(id, node.flow_shapes());
                m.flow = node.flow_shapes().to_vec();
            }
            if native_protected {
                continue;
            }
            let parent = node.parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let rel = relative(node.frame, parent);
            let content = (style::effective_overflow(&node)
                != (Overflow::Visible, Overflow::Visible))
                .then(|| content_size(&node, kernel));
            // An ancestor hint may change without touching the editor. Pass
            // its effective value through native containment, or clear it to
            // restore the platform default when the last declaration disappears.
            if node.node_type == NodeType::TextInput {
                let spelling = node.spellcheck().map(|value| value.to_string());
                if m.props.get("spellcheck") != spelling.as_ref() {
                    if let Some(value) = spelling {
                        batch.props(id, &[("spellcheck", value.clone())], &[]);
                        m.props.insert("spellcheck".into(), value);
                    } else {
                        batch.props(id, &[], &["spellcheck"]);
                        m.props.remove("spellcheck");
                    }
                }
            }
            if m.frame != Some(rel) {
                m.frame = Some(rel);
                batch.frame(id, rel.0, rel.1, rel.2, rel.3);
            }
            if let Some(c) = content {
                if m.content != Some(c) {
                    m.content = Some(c);
                    batch.content(id, c.0, c.1);
                }
            }
            // @ref LLP 1063 — a moved box plays from where it was.
            if !self.presence.snap {
                self.observe_layout(key, batch);
            }
        }
        self.snap_layout(batch);
        self.emit_sticky(batch);
        self.emit_ranks(batch);
        // Layout/receipt work may change the live window. Motion-only ticks and
        // stale feedback never traverse the tree to collect this metadata.
        let collections = if self.native_mode() {
            self.native_collections_json()?
        } else {
            self.runner.collections_json()
        };
        if collections != self.collections_json {
            batch.collections(&collections);
            self.collections_json = collections;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{Op, PropId};
    use exact_runner::{DataError, Value};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }
    fn fixture() -> Host<NoData> {
        let plan = contract::compile(r#"component App
  view
    box testId="root"
      box testId="port" width=200 height=80 overflow-x="hidden" overflow-y="scroll" spellcheck="false"
        text "short" testId="text"
        input testId="field" value="abc"
      text "unrelated" testId="other"
"#).unwrap().encode();
        Host::boot(
            &plan,
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            900.,
            700.,
        )
        .unwrap()
        .0
    }
    fn id(host: &Host<NoData>, name: &str) -> ViewId {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    }
    #[test]
    fn ranks_follow_paint_facts_without_geometry_and_clear_to_zero() {
        use exact_kernel::{StyleId, StyleProps, StyleValue};
        fn opacity(id: ViewId, value: f64) -> Op {
            let mut patch = StyleProps::default();
            patch
                .set_dynamic(StyleId::Opacity, &StyleValue::Number(value))
                .unwrap();
            Op::SetStyle {
                id,
                patch: Box::new(patch),
            }
        }
        let mut host = fixture();
        let root = id(&host, "root");
        let other = id(&host, "other");
        assert_eq!(host.ranks[&other], 0);
        let frame = host.mirror[&other].frame;
        let mut apply = |ops: &[Op]| {
            let receipt = host.runner.kernel_mut().apply(0, 99, ops).unwrap();
            host.commit(&[Timed { at_ms: 0., receipt }], None)
        };
        let raised = apply(&[opacity(other, 0.5)]);
        assert!(
            raised.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":1}}")),
            "{raised}"
        );
        let cleared = apply(&[opacity(other, 1.)]);
        assert!(
            cleared.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":0}}")),
            "{cleared}"
        );
        assert_eq!(host.mirror[&other].frame, frame);
        let mut unchanged = Batch::new();
        host.layout(&mut unchanged).unwrap();
        assert!(!unchanged
            .finish(None, false, 0., None)
            .contains("\"op\":\"rank\""));
        assert_eq!(host.ranks[&root], 1);
        let receipt = host
            .runner
            .kernel_mut()
            .apply(0, 100, &[Op::DestroyView { id: other }])
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(!host.ranks.contains_key(&other));
    }

    #[test]
    fn ranks_follow_props_children_and_new_roots() {
        let mut host = fixture();
        let root = id(&host, "root");
        let port = id(&host, "port");
        let other = id(&host, "other");
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                99,
                &[Op::SetProp {
                    id: other,
                    prop: PropId::BackgroundMaterial,
                    value: "thin".into(),
                }],
            )
            .unwrap();
        let batch = host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(batch.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":1}}")));
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                100,
                &[
                    Op::SetChildren {
                        id: root,
                        children: vec![port],
                    },
                    Op::AttachRoot { id: other },
                ],
            )
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert_eq!(host.ranks[&other], 1);
        // A plain box moved out of the tree then made a root changes rank
        // even though it has no authored style or geometry change.
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                101,
                &[
                    Op::CreateView {
                        id: 999,
                        node_type: NodeType::View,
                    },
                    Op::SetChildren {
                        id: root,
                        children: vec![port, 999],
                    },
                ],
            )
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert_eq!(host.ranks[&999], 0);
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                102,
                &[
                    Op::SetChildren {
                        id: root,
                        children: vec![port],
                    },
                    Op::AttachRoot { id: 999 },
                ],
            )
            .unwrap();
        let batch = host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(batch.contains("{\"op\":\"rank\",\"id\":999,\"rank\":1}"));
    }

    #[test]
    fn ancestor_spelling_change_reaches_an_unmoved_editor() {
        let mut host = fixture();
        let port = id(&host, "port");
        let field = id(&host, "field");
        let frame = host.mirror[&field].frame;
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("false")
        );
        host.runner
            .kernel_mut()
            .apply(
                0,
                2,
                &[Op::SetProp {
                    id: port,
                    prop: PropId::Spellcheck,
                    value: "true".into(),
                }],
            )
            .unwrap();
        let mut batch = Batch::new();
        host.update(port, &mut batch);
        host.layout(&mut batch).unwrap();
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(host.mirror[&field].frame, frame);
    }

    #[test]
    fn a_new_sibling_does_not_requeue_retained_subtrees_but_a_move_does() {
        let mut host = fixture();
        let root = id(&host, "root");
        let port = id(&host, "port");
        let field = id(&host, "field");
        let field_key = host.runner.kernel().node(field).unwrap().key;
        let added = 999;
        let mut children = host.runner.kernel().node(root).unwrap().children();
        children.push(added);
        host.runner
            .kernel_mut()
            .apply(
                0,
                2,
                &[
                    Op::CreateView {
                        id: added,
                        node_type: NodeType::View,
                    },
                    Op::SetProp {
                        id: added,
                        prop: PropId::Spellcheck,
                        value: "true".into(),
                    },
                    Op::SetChildren { id: root, children },
                ],
            )
            .unwrap();
        let mut batch = Batch::new();
        host.create(added, &[], &mut batch);
        host.emit_children(root, &mut batch);
        assert!(!host.pending_layout.contains(&field_key));
        host.layout(&mut batch).unwrap();

        host.runner
            .kernel_mut()
            .apply(
                0,
                3,
                &[Op::SetChildren {
                    id: added,
                    children: vec![field],
                }],
            )
            .unwrap();
        host.emit_children(port, &mut batch);
        host.emit_children(added, &mut batch);
        assert!(host.pending_layout.contains(&field_key));
        host.layout(&mut batch).unwrap();
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("true")
        );
    }
}
