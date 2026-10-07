//! Layout receipts survive silent list settling until the presenter sees them.
use super::*;

impl<D: DataSource> Host<D> {
    pub(super) fn withhold_layout(&mut self) {
        // These geometry rows will be withheld. Force a settled retry to
        // publish them even if the kernel's last guessed boxes compare equal.
        self.layout_withheld = true;
        for m in self.mirror.values_mut() {
            m.frame = None;
        }
        self.pending_layout.extend(
            self.mirror
                .keys()
                .filter_map(|id| self.runner.kernel().node(*id).map(|n| n.key)),
        );
    }

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
            // A miss has filled its cache on main before returning. Re-run
            // silently, collecting both receipts so a final unchanged frame
            // still replaces the provisional geometry the presenter never saw.
            for attempt in 0..3 {
                let before = self.runner.kernel().provisional_layouts();
                if self.content_region.is_some() {
                    self.region_layout(root, Offer::definite(w, h), batch)?;
                } else {
                    let receipt = self
                        .runner
                        .kernel_mut()
                        .compute_layout_presented(
                            root,
                            Offer::definite(w, h),
                            &self.height_presented,
                        )
                        .map_err(|e| format!("layout: {e:?}"))?;
                    self.runner.report_flow_skipped(&receipt.flow_skipped);
                    self.runner
                        .report_fragment_skipped(&receipt.fragment_skipped);
                    self.runner.moved(&receipt.changed);
                    self.record_layout(&receipt);
                }
                batch.layout_provisional = self.runner.kernel().provisional_layouts() != before;
                if !batch.layout_provisional {
                    break;
                }
                if attempt == 2 {
                    return Err("layout: field chrome remained provisional after three passes; batch refused before presentation".into());
                }
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
                if self.layout_withheld || self.stickies.get(&node.id) != Some(&c) {
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

    /// @ref LLP 1093 D7 — each box's fragments and each container's columns,
    /// when they changed: the presenter's copy of the kernel's record.
    fn emit_fragments(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let keys = kernel.fragmented();
        if keys.is_empty() && self.fragments.is_empty() {
            return;
        }
        let mut now = IdMap::default();
        for key in keys {
            let Some(node) = kernel.node_by_key(key) else {
                continue;
            };
            let record = crate::batch::fragments_json(kernel.fragments(key), kernel.columns(key));
            if self.layout_withheld || self.fragments.get(&node.id) != Some(&record) {
                batch.fragments(node.id, &record);
            }
            now.insert(node.id, record);
        }
        for id in self.fragments.keys() {
            if !now.contains_key(id) && kernel.node(*id).is_some() {
                batch.fragments(*id, "");
            }
        }
        self.fragments = now;
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
            if self.layout_withheld || m.flow != node.flow_shapes() {
                batch.flow(id, node.flow_shapes());
                m.flow = node.flow_shapes().to_vec();
            }
            if native_protected {
                continue;
            }
            let parent = node.parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let rel = relative(node.frame, parent);
            // A sheet sized to its route's content reads that extent too
            // (LLP 1075.003 §9.11). A route that scrolls itself keeps its
            // scroll extent, which the sheet then reads.
            let scrolls =
                style::effective_overflow(&node) != (Overflow::Visible, Overflow::Visible);
            let fits = node
                .props
                .str(PropId::NavigationDetent)
                .is_some_and(|d| d.split(' ').any(|w| w == "fit-content"));
            let content = if scrolls {
                Some(content_size(&node, kernel))
            } else {
                fits.then(|| fitted_size(&node, kernel))
            };
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
            let field_content = node.field_content_rect();
            if (self.layout_withheld && node.node_type == NodeType::TextInput)
                || m.field_content != field_content
            {
                m.field_content = field_content;
                batch.field_content(id, field_content);
            }
            if m.frame != Some(rel) {
                m.frame = Some(rel);
                batch.frame(id, rel.0, rel.1, rel.2, rel.3);
            }
            if let Some(c) = content {
                if self.layout_withheld || m.content != Some(c) {
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
        self.emit_fragments(batch);
        self.layout_withheld = false;
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

/// Natural scrollable overflow, including padding and descendants. The
/// presenter applies the CSS client-size minimum against its actual viewport;
/// flooring here loses the extent a native container needs under its own insets.
pub(super) fn content_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    extent(node, kernel, node.content)
}

/// A `fit-content` sheet's measure (LLP 1075.003 §9.11): the children's
/// extent and the authored end padding, at least the authored vertical
/// padding, without Taffy's height, which may count end padding, a native
/// container's bottom cover among it (the sheet's safe area, which UIKit adds
/// below the detent itself).
fn fitted_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    let env = kernel.env();
    let pads = padding(node.style.padding_top, node.frame.width, &env)
        + padding(node.style.padding_bottom, node.frame.width, &env);
    extent(node, kernel, (node.content.0, pads))
}

/// `from` floored by the direct children's extent plus the end padding.
fn extent(node: &NodeRef<'_>, kernel: &Kernel, from: (f32, f32)) -> (f32, f32) {
    // Taffy's block containers do not always count end-edge padding in
    // `content_size` (its flex containers do); CSS's `scrollHeight` does.
    // Floor with the direct children's extent plus the end padding.
    let env = kernel.env();
    let pad_right = padding(node.style.padding_right, node.frame.width, &env);
    let pad_bottom = padding(node.style.padding_bottom, node.frame.width, &env);
    let (mut w, mut h) = from;
    for child in node.children() {
        if let Some(c) = kernel.node(child) {
            w = w.max(c.frame.x - node.frame.x + c.frame.width + pad_right);
            h = h.max(c.frame.y - node.frame.y + c.frame.height + pad_bottom);
        }
    }
    (w, h)
}

/// A padding row in points; a percentage is of the containing width.
fn padding(d: exact_kernel::Dimension, against: f32, env: &exact_kernel::Env) -> f32 {
    match d.resolve(env) {
        exact_kernel::Dimension::Points(p) => p,
        exact_kernel::Dimension::Percent(p) => against * p / 100.0,
        exact_kernel::Dimension::Calc(p, x) => against * p / 100.0 + x,
        exact_kernel::Dimension::Auto
        | exact_kernel::Dimension::Env(..)
        | exact_kernel::Dimension::Segment(..)
        | exact_kernel::Dimension::Viewport(..) => 0.0,
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
